//! Transaction Info Page - stores active transaction state for recovery

use crate::types::{PageId, TransactionId, UndoPtr, PAGE_SIZE};

/// Maximum number of transactions we can track
const MAX_TX_ENTRIES: usize = 1024;

/// Transaction info entry
#[derive(Debug, Clone, Copy)]
pub struct TxEntry {
    pub tx_id: TransactionId,
    pub last_undo_ptr: UndoPtr,
    pub is_active: bool,
}

impl TxEntry {
    pub fn null() -> Self {
        Self {
            tx_id: 0,
            last_undo_ptr: UndoPtr::null(),
            is_active: false,
        }
    }
}

/// Transaction Info Page - special system page for tracking transaction state
/// This page is managed by BufferMgr like regular data pages
#[derive(Debug, Clone)]
pub struct TrxInfoPage {
    pub page_id: PageId,
    pub entries: [TxEntry; MAX_TX_ENTRIES],
    pub count: usize,
}

impl TrxInfoPage {
    pub fn new(page_id: PageId) -> Self {
        Self {
            page_id,
            entries: [TxEntry::null(); MAX_TX_ENTRIES],
            count: 0,
        }
    }

    pub fn add_transaction(&mut self, tx_id: TransactionId, undo_ptr: UndoPtr) -> bool {
        if self.count >= MAX_TX_ENTRIES {
            return false;
        }

        for entry in &mut self.entries[..self.count] {
            if entry.tx_id == tx_id {
                entry.last_undo_ptr = undo_ptr;
                entry.is_active = true;
                return true;
            }
        }

        self.entries[self.count] = TxEntry {
            tx_id,
            last_undo_ptr: undo_ptr,
            is_active: true,
        };
        self.count += 1;
        true
    }

    pub fn remove_transaction(&mut self, tx_id: TransactionId) -> bool {
        for entry in &mut self.entries[..self.count] {
            if entry.tx_id == tx_id {
                entry.is_active = false;
                return true;
            }
        }
        false
    }

    pub fn get_undo_ptr(&self, tx_id: TransactionId) -> Option<UndoPtr> {
        for entry in &self.entries[..self.count] {
            if entry.tx_id == tx_id && entry.is_active {
                return Some(entry.last_undo_ptr);
            }
        }
        None
    }

    pub fn get_active_transactions(&self) -> Vec<(TransactionId, UndoPtr)> {
        let mut result = Vec::new();
        for entry in &self.entries[..self.count] {
            if entry.is_active {
                result.push((entry.tx_id, entry.last_undo_ptr));
            }
        }
        result
    }

    pub fn clear(&mut self) {
        self.count = 0;
        self.entries = [TxEntry::null(); MAX_TX_ENTRIES];
    }

    pub fn as_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut data = [0u8; PAGE_SIZE];

        let page_id_bytes = self.page_id.to_le_bytes();
        data[0..8].copy_from_slice(&page_id_bytes);

        let count_bytes = (self.count as u64).to_le_bytes();
        data[8..16].copy_from_slice(&count_bytes);

        let mut offset = 16;
        for entry in &self.entries[..self.count] {
            let tx_bytes = entry.tx_id.to_le_bytes();
            let ptr_page_bytes = entry.last_undo_ptr.page_id.to_le_bytes();
            let ptr_offset_bytes = entry.last_undo_ptr.offset.to_le_bytes();
            let ptr_lsn_bytes = entry.last_undo_ptr.lsn.to_le_bytes();
            let active_byte = if entry.is_active { 1u8 } else { 0u8 };

            data[offset..offset + 8].copy_from_slice(&tx_bytes);
            data[offset + 8..offset + 16].copy_from_slice(&ptr_page_bytes);
            data[offset + 16..offset + 18].copy_from_slice(&ptr_offset_bytes);
            data[offset + 18..offset + 26].copy_from_slice(&ptr_lsn_bytes);
            data[offset + 26] = active_byte;
            offset += 27;

            if offset >= PAGE_SIZE {
                break;
            }
        }

        data
    }

    pub fn from_bytes(data: &[u8]) -> Self {
        let page_id = u64::from_le_bytes(data[0..8].try_into().unwrap());
        let count = u64::from_le_bytes(data[8..16].try_into().unwrap()) as usize;

        let mut entries = [TxEntry::null(); MAX_TX_ENTRIES];
        let mut offset = 16;

        for i in 0..count.min(MAX_TX_ENTRIES) {
            if offset + 27 > data.len() {
                break;
            }
            let tx_id = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
            let ptr_page_id = u64::from_le_bytes(data[offset + 8..offset + 16].try_into().unwrap());
            let ptr_offset = u16::from_le_bytes(data[offset + 16..offset + 18].try_into().unwrap());
            let ptr_lsn = u64::from_le_bytes(data[offset + 18..offset + 26].try_into().unwrap());
            let is_active = data[offset + 26] != 0;

            entries[i] = TxEntry {
                tx_id,
                last_undo_ptr: UndoPtr {
                    page_id: ptr_page_id,
                    offset: ptr_offset,
                    lsn: ptr_lsn,
                },
                is_active,
            };
            offset += 27;
        }

        Self {
            page_id,
            entries,
            count: count.min(MAX_TX_ENTRIES),
        }
    }
}
