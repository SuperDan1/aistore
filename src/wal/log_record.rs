//! WAL Log Record Format

use crate::lock::TransactionId;
use crate::types::PageId;
use crate::wal::lsn::LSN;

/// Log type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogType {
    TxBegin,
    TxCommit,
    TxAbort,
    PageRedo,
}

/// Log record header (fixed 32 bytes)
#[derive(Debug, Clone)]
pub struct LogRecordHeader {
    pub lsn: LSN,
    pub tx_id: TransactionId,
    pub prev_lsn: LSN,
    pub log_type: LogType,
    pub payload_len: u32,
    pub checksum: u32,
}

/// Log record
#[derive(Debug, Clone)]
pub struct LogRecord {
    pub header: LogRecordHeader,
    pub payload: Vec<u8>,
}

/// Page redo payload
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PageRedoPayload {
    pub page_id: PageId,
    pub offset: u32,
    pub data: Vec<u8>,
}

impl LogRecord {
    /// Create a transaction begin log
    pub fn tx_begin(tx_id: TransactionId, prev_lsn: LSN) -> Self {
        Self {
            header: LogRecordHeader {
                lsn: LSN::invalid(),
                tx_id,
                prev_lsn,
                log_type: LogType::TxBegin,
                payload_len: 0,
                checksum: 0,
            },
            payload: Vec::new(),
        }
    }

    /// Create a transaction commit log
    pub fn tx_commit(tx_id: TransactionId, prev_lsn: LSN) -> Self {
        Self {
            header: LogRecordHeader {
                lsn: LSN::invalid(),
                tx_id,
                prev_lsn,
                log_type: LogType::TxCommit,
                payload_len: 0,
                checksum: 0,
            },
            payload: Vec::new(),
        }
    }

    /// Create a transaction abort log
    pub fn tx_abort(tx_id: TransactionId, prev_lsn: LSN) -> Self {
        Self {
            header: LogRecordHeader {
                lsn: LSN::invalid(),
                tx_id,
                prev_lsn,
                log_type: LogType::TxAbort,
                payload_len: 0,
                checksum: 0,
            },
            payload: Vec::new(),
        }
    }

    /// Create a page redo log
    pub fn page_redo(
        tx_id: TransactionId,
        prev_lsn: LSN,
        page_id: PageId,
        offset: u32,
        data: Vec<u8>,
    ) -> Self {
        let payload = PageRedoPayload {
            page_id,
            offset,
            data,
        };
        let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();

        Self {
            header: LogRecordHeader {
                lsn: LSN::invalid(),
                tx_id,
                prev_lsn,
                log_type: LogType::PageRedo,
                payload_len: payload_bytes.len() as u32,
                checksum: 0,
            },
            payload: payload_bytes,
        }
    }

    /// Get the serialized size of this record
    pub fn serialized_size(&self) -> usize {
        std::mem::size_of::<LogRecordHeader>() + self.payload.len()
    }

    /// Serialize the record to bytes
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.serialized_size());

        // Serialize header
        bytes.extend_from_slice(&self.header.lsn.raw().to_le_bytes());
        bytes.extend_from_slice(&self.header.tx_id.to_le_bytes());
        bytes.extend_from_slice(&self.header.prev_lsn.raw().to_le_bytes());
        bytes.push(match self.header.log_type {
            LogType::TxBegin => 0,
            LogType::TxCommit => 1,
            LogType::TxAbort => 2,
            LogType::PageRedo => 3,
        });
        bytes.extend_from_slice(&self.header.payload_len.to_le_bytes());

        // Payload
        bytes.extend_from_slice(&self.payload);

        bytes
    }

    /// Deserialize from bytes
    pub fn deserialize(data: &[u8]) -> Option<Self> {
        if data.len() < 32 {
            return None;
        }

        let lsn = LSN::from_raw(u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]));
        let tx_id = u64::from_le_bytes([
            data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
        ]);
        let prev_lsn = LSN::from_raw(u64::from_le_bytes([
            data[16], data[17], data[18], data[19], data[20], data[21], data[22], data[23],
        ]));
        let log_type = match data[24] {
            0 => LogType::TxBegin,
            1 => LogType::TxCommit,
            2 => LogType::TxAbort,
            3 => LogType::PageRedo,
            _ => return None,
        };
        let payload_len = u32::from_le_bytes([data[25], data[26], data[27], data[28]]);

        if data.len() < 32 + payload_len as usize {
            return None;
        }

        let payload = data[32..32 + payload_len as usize].to_vec();

        Some(Self {
            header: LogRecordHeader {
                lsn,
                tx_id,
                prev_lsn,
                log_type,
                payload_len,
                checksum: 0,
            },
            payload,
        })
    }
}
