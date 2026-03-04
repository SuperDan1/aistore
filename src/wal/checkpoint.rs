//! WAL Checkpoint

use crate::types::PageId;
use crate::vfs::{VfsInterface, VfsResult};
use crate::wal::lsn::LSN;
use std::path::PathBuf;
use std::sync::Arc;

const CHECKPOINT_MAGIC: u32 = 0x434B5054; // "CKPT"
const CHECKPOINT_VERSION: u32 = 0x00000001;

/// Checkpoint record
#[derive(Debug, Clone)]
pub struct CheckpointRecord {
    pub checkpoint_id: u64,
    pub begin_lsn: LSN,
    pub end_lsn: LSN,
    pub dirty_pages: Vec<PageId>,
    pub active_transactions: Vec<u64>,
}

#[derive(Clone)]
pub struct CheckpointManager {
    checkpoint_dir: PathBuf,
    vfs: Arc<dyn VfsInterface>,
    last_checkpoint_lsn: LSN,
    checkpoint_id: u64,
}

impl CheckpointManager {
    pub fn new(checkpoint_dir: PathBuf, vfs: Arc<dyn VfsInterface>) -> Self {
        Self {
            checkpoint_dir,
            vfs,
            last_checkpoint_lsn: LSN::invalid(),
            checkpoint_id: 0,
        }
    }

    /// Create a checkpoint
    pub fn checkpoint(
        &mut self,
        begin_lsn: LSN,
        dirty_pages: Vec<PageId>,
        active_transactions: Vec<u64>,
    ) -> VfsResult<CheckpointRecord> {
        self.checkpoint_id += 1;

        let record = CheckpointRecord {
            checkpoint_id: self.checkpoint_id,
            begin_lsn: begin_lsn,
            end_lsn: LSN::invalid(),
            dirty_pages,
            active_transactions,
        };

        self.write_checkpoint(&record)?;
        self.last_checkpoint_lsn = begin_lsn;

        Ok(record)
    }

    /// Write checkpoint to disk
    fn write_checkpoint(&self, record: &CheckpointRecord) -> VfsResult<()> {
        self.vfs.create_dir(self.checkpoint_dir.to_str().unwrap())?;

        let path = self.checkpoint_dir.join("checkpoint.bin");

        let mut data = Vec::new();
        data.extend_from_slice(&CHECKPOINT_MAGIC.to_le_bytes());
        data.extend_from_slice(&CHECKPOINT_VERSION.to_le_bytes());
        data.extend_from_slice(&record.checkpoint_id.to_le_bytes());
        data.extend_from_slice(&record.begin_lsn.raw().to_le_bytes());
        data.extend_from_slice(&record.end_lsn.raw().to_le_bytes());

        let dirty_count = record.dirty_pages.len() as u32;
        data.extend_from_slice(&dirty_count.to_le_bytes());
        for page_id in &record.dirty_pages {
            data.extend_from_slice(&page_id.to_le_bytes());
        }

        let tx_count = record.active_transactions.len() as u32;
        data.extend_from_slice(&tx_count.to_le_bytes());
        for tx_id in &record.active_transactions {
            data.extend_from_slice(&tx_id.to_le_bytes());
        }

        self.vfs.pwrite(path.to_str().unwrap(), &data, 0)?;

        Ok(())
    }

    /// Load latest checkpoint
    pub fn load_latest(&self) -> Option<CheckpointRecord> {
        let path = self.checkpoint_dir.join("checkpoint.bin");

        let mut data = [0u8; 1024];
        let n = match self.vfs.pread(path.to_str().unwrap(), &mut data, 0) {
            Ok(n) => n,
            Err(_) => return None,
        };

        if n < 24 {
            return None;
        }

        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != CHECKPOINT_MAGIC {
            return None;
        }

        let checkpoint_id = u64::from_le_bytes([
            data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
        ]);
        let begin_lsn = LSN::from_raw(u64::from_le_bytes([
            data[16], data[17], data[18], data[19], data[20], data[21], data[22], data[23],
        ]));

        let dirty_count = u32::from_le_bytes([data[24], data[25], data[26], data[27]]) as usize;
        let mut dirty_pages = Vec::new();
        let mut offset = 28;
        for _ in 0..dirty_count {
            if offset + 8 > n {
                break;
            }
            let page_id = u64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            dirty_pages.push(page_id);
            offset += 8;
        }

        let tx_count = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        let mut active_transactions = Vec::new();
        offset += 4;
        for _ in 0..tx_count {
            if offset + 8 > n {
                break;
            }
            let tx_id = u64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            active_transactions.push(tx_id);
            offset += 8;
        }

        Some(CheckpointRecord {
            checkpoint_id,
            begin_lsn,
            end_lsn: LSN::invalid(),
            dirty_pages,
            active_transactions,
        })
    }

    /// Get last checkpoint LSN
    pub fn last_checkpoint_lsn(&self) -> LSN {
        self.last_checkpoint_lsn
    }
}
