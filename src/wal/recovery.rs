//! WAL Recovery

use crate::types::PageId;
use crate::vfs::VfsInterface;
use crate::wal::checkpoint::CheckpointManager;
use crate::wal::config::WalConfig;
use crate::wal::log_file::LogFileManager;
use crate::wal::log_record::{LogRecord, LogType, PageRedoPayload};
use crate::wal::lsn::LSN;
use std::sync::Arc;

/// Recovery result
#[derive(Debug)]
pub struct RecoveryResult {
    pub checkpoint_lsn: LSN,
    pub replayed_records: usize,
    pub rolled_back_transactions: Vec<u64>,
}

/// Recovery manager
pub struct RecoveryManager {
    config: WalConfig,
    file_mgr: Arc<LogFileManager>,
    checkpoint_mgr: CheckpointManager,
    vfs: Arc<dyn VfsInterface>,
}

impl RecoveryManager {
    pub fn new(
        config: WalConfig,
        file_mgr: Arc<LogFileManager>,
        checkpoint_mgr: CheckpointManager,
        vfs: Arc<dyn VfsInterface>,
    ) -> Self {
        Self {
            config,
            file_mgr,
            checkpoint_mgr,
            vfs,
        }
    }

    /// Perform recovery with page writer callback
    pub fn recover<F>(&self, write_page: F) -> RecoveryResult
    where
        F: Fn(PageId, &[u8]) -> Result<(), String>,
    {
        let checkpoint = self.checkpoint_mgr.load_latest();

        let checkpoint_lsn = checkpoint
            .as_ref()
            .map(|c| c.begin_lsn)
            .unwrap_or(LSN::invalid());

        let mut replayed_records = 0;
        let mut rolled_back = Vec::new();

        if let Some(cp) = checkpoint {
            replayed_records = self.replay_from_lsn(cp.begin_lsn, &write_page);
            rolled_back = cp.active_transactions;
        }

        RecoveryResult {
            checkpoint_lsn,
            replayed_records,
            rolled_back_transactions: rolled_back,
        }
    }

    /// Replay log records from a specific LSN
    fn replay_from_lsn<F>(&self, lsn: LSN, write_page: &F) -> usize
    where
        F: Fn(PageId, &[u8]) -> Result<(), String>,
    {
        let mut count = 0;
        let mut current_lsn = lsn;

        loop {
            let data = match self.file_mgr.read_from(current_lsn) {
                Ok(d) => d,
                Err(_) => break,
            };

            if data.is_empty() {
                break;
            }

            let record = match LogRecord::deserialize(&data) {
                Some(r) => r,
                None => break,
            };

            match record.header.log_type {
                LogType::PageRedo => {
                    if let Ok(payload) = serde_json::from_slice::<PageRedoPayload>(&record.payload)
                    {
                        if write_page(payload.page_id, &payload.data).is_ok() {
                            count += 1;
                        }
                    }
                }
                LogType::TxCommit => {
                    count += 1;
                }
                _ => {}
            }

            current_lsn = current_lsn + data.len() as u64;
        }

        count
    }
}
