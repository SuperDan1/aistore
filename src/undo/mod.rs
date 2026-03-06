//! Undo Tablespace for MVCC

mod error;
mod segment;

pub use error::{UndoError, UndoResult};
pub use segment::UndoSegment;

use crate::types::{PageId, TransactionId, UndoPtr, UndoRecord, UndoRecordHeader, UndoType, LSN};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

const UNDO_SEGMENT_SIZE: usize = 64 * 1024 * 1024;
const UNDO_PAGE_SIZE: usize = 8192;
const UNDO_FILE_PREFIX: &str = "undo_";
const UNDO_FILE_EXT: &str = "seg";

pub struct UndoManager {
    tablespace_path: PathBuf,
    segments: RwLock<Vec<UndoSegment>>,
    current_segment_id: RwLock<u32>,
    write_lock: RwLock<()>,
}

impl UndoManager {
    pub fn new(data_dir: &str) -> Self {
        let tablespace_path = PathBuf::from(data_dir).join("undo_tablespace");
        std::fs::create_dir_all(&tablespace_path).ok();

        Self {
            tablespace_path,
            segments: RwLock::new(Vec::new()),
            current_segment_id: RwLock::new(0),
            write_lock: RwLock::new(()),
        }
    }

    pub fn append(
        &self,
        tx_id: TransactionId,
        table_id: u32,
        row_page_id: PageId,
        row_slot_idx: usize,
        undo_type: UndoType,
        before_image: Vec<u8>,
        prev_undo_ptr: UndoPtr,
    ) -> UndoResult<UndoPtr> {
        let _guard = self.write_lock.write().unwrap();

        let header = UndoRecordHeader {
            length: (56 + before_image.len()) as u32,
            tx_id,
            table_id,
            row_page_id,
            row_slot_idx,
            prev_undo_ptr,
            undo_type,
            checksum: 0,
        };

        let record = UndoRecord {
            header,
            before_image,
        };
        let serialized = Self::serialize_record(&record);

        let (segment_id, offset) = self.write_to_segment(&serialized)?;

        Ok(UndoPtr {
            page_id: segment_id as PageId,
            offset,
            lsn: 0,
        })
    }

    pub fn get(&self, ptr: &UndoPtr) -> UndoResult<UndoRecord> {
        let segment_id = ptr.page_id as u32;
        let mut segments = self.segments.write().unwrap();

        let segment = segments
            .iter_mut()
            .find(|s| s.segment_id() == segment_id)
            .ok_or(UndoError::SegmentNotFound(segment_id))?;

        let data = segment.read_record(ptr.offset as usize)?;
        Self::deserialize_record(&data)
    }

    pub fn get_tx_undo_chain(
        &self,
        tx_id: TransactionId,
        first_ptr: UndoPtr,
    ) -> UndoResult<Vec<UndoRecord>> {
        let mut records = Vec::new();
        let mut ptr = first_ptr;

        while !ptr.is_null() {
            let record = self.get(&ptr)?;
            if record.header.tx_id != tx_id {
                break;
            }
            let prev_ptr = record.header.prev_undo_ptr;
            records.push(record);
            ptr = prev_ptr;
        }

        Ok(records)
    }

    pub fn purge_completed_tx(&self, oldest_active_tx: TransactionId) -> UndoResult<usize> {
        let mut purged = 0;
        let segments = self.segments.read().unwrap();

        for segment in segments.iter() {
            purged += segment.purge_old_records(oldest_active_tx)?;
        }

        Ok(purged)
    }

    fn write_to_segment(&self, data: &[u8]) -> UndoResult<(u32, u16)> {
        let mut segments = self.segments.write().unwrap();

        loop {
            let current_id = *self.current_segment_id.read().unwrap();

            if current_id as usize >= segments.len() {
                let new_segment =
                    UndoSegment::create(&self.tablespace_path, current_id, UNDO_SEGMENT_SIZE)?;
                segments.push(new_segment);
            }

            let segment = &mut segments[current_id as usize];

            match segment.append(data) {
                Ok(offset) => return Ok((current_id, offset)),
                Err(UndoError::SegmentFull) => {
                    *self.current_segment_id.write().unwrap() += 1;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn serialize_record(record: &UndoRecord) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(record.serialized_size());

        bytes.extend_from_slice(&record.header.length.to_le_bytes());
        bytes.extend_from_slice(&record.header.tx_id.to_le_bytes());
        bytes.extend_from_slice(&record.header.table_id.to_le_bytes());
        bytes.extend_from_slice(&record.header.row_page_id.to_le_bytes());
        bytes.extend_from_slice(&(record.header.row_slot_idx as u64).to_le_bytes());
        bytes.extend_from_slice(&record.header.prev_undo_ptr.page_id.to_le_bytes());
        bytes.extend_from_slice(&record.header.prev_undo_ptr.offset.to_le_bytes());
        bytes.extend_from_slice(&record.header.prev_undo_ptr.lsn.to_le_bytes());
        bytes.push(match record.header.undo_type {
            UndoType::Insert => 0,
            UndoType::Update => 1,
            UndoType::Delete => 2,
        });
        bytes.extend_from_slice(&0u32.to_le_bytes());

        let padding = 56 - bytes.len();
        bytes.extend(vec![0u8; padding]);

        bytes.extend_from_slice(&record.before_image);

        bytes
    }

    fn deserialize_record(data: &[u8]) -> UndoResult<UndoRecord> {
        if data.len() < 56 {
            return Err(UndoError::CorruptedRecord);
        }

        let length = u32::from_le_bytes(data[0..4].try_into().unwrap());
        let tx_id = TransactionId::from_le_bytes(data[4..12].try_into().unwrap());
        let table_id = u32::from_le_bytes(data[12..16].try_into().unwrap());
        let row_page_id = PageId::from_le_bytes(data[16..24].try_into().unwrap());
        let row_slot_idx = usize::from_le_bytes(data[24..32].try_into().unwrap());
        let prev_page_id = PageId::from_le_bytes(data[32..40].try_into().unwrap());
        let prev_offset = u16::from_le_bytes(data[40..42].try_into().unwrap());
        let prev_lsn = LSN::from_le_bytes(data[42..50].try_into().unwrap());
        let undo_type_val = data[50];

        let undo_type = match undo_type_val {
            0 => UndoType::Insert,
            1 => UndoType::Update,
            2 => UndoType::Delete,
            _ => return Err(UndoError::CorruptedRecord),
        };

        let before_image = data[56..].to_vec();

        let header = UndoRecordHeader {
            length,
            tx_id,
            table_id,
            row_page_id,
            row_slot_idx,
            prev_undo_ptr: UndoPtr {
                page_id: prev_page_id,
                offset: prev_offset,
                lsn: prev_lsn,
            },
            undo_type,
            checksum: 0,
        };

        Ok(UndoRecord {
            header,
            before_image,
        })
    }
}

impl Default for UndoManager {
    fn default() -> Self {
        Self::new("./data")
    }
}
