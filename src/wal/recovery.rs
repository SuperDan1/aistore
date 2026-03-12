//! WAL Recovery

use crate::types::PageId;
use crate::vfs::VfsInterface;
use crate::wal::checkpoint::CheckpointManager;
use crate::wal::config::WalConfig;
use crate::wal::log_file::LogFileManager;
use crate::wal::log_record::{LogRecord, LogType, PageRedoPayload};
use crate::wal::lsn::LSN;
use std::collections::HashSet;
use std::sync::Arc;

/// Recovery result
#[derive(Debug)]
pub struct RecoveryResult {
    pub checkpoint_lsn: LSN,
    pub replayed_records: usize,
    pub trx_info_page_id: PageId,
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
        let trx_info_page_id = checkpoint.as_ref().map(|c| c.trx_info_page_id).unwrap_or(0);

        if let Some(cp) = checkpoint {
            let active_txs: HashSet<u64> = cp.active_transactions.iter().cloned().collect();
            replayed_records = self.replay_from_lsn(cp.begin_lsn, &write_page, Some(&active_txs));
        } else {
            replayed_records = self.replay_from_lsn(LSN::invalid(), &write_page, None);
        }

        RecoveryResult {
            checkpoint_lsn,
            replayed_records,
            trx_info_page_id,
        }
    }

    /// Replay log records from a specific LSN
    fn replay_from_lsn<F>(
        &self,
        lsn: LSN,
        write_page: &F,
        active_txs: Option<&HashSet<u64>>,
    ) -> usize
    where
        F: Fn(PageId, &[u8]) -> Result<(), String>,
    {
        let mut count = 0;
        let mut current_lsn = lsn;

        let mut committed_txs: HashSet<u64> = HashSet::new();
        let mut pending_txs: HashSet<u64> = HashSet::new();

        if let Some(active) = active_txs {
            pending_txs = active.clone();
        }

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

            let tx_id = record.header.tx_id;

            match record.header.log_type {
                LogType::TxBegin => {
                    pending_txs.insert(tx_id);
                }
                LogType::TxCommit => {
                    committed_txs.insert(tx_id);
                    pending_txs.remove(&tx_id);
                }
                LogType::TxAbort => {
                    pending_txs.remove(&tx_id);
                }
                LogType::PageRedo => {
                    if committed_txs.contains(&tx_id) {
                        if let Ok(payload) =
                            serde_json::from_slice::<PageRedoPayload>(&record.payload)
                        {
                            if write_page(payload.page_id, &payload.data).is_ok() {
                                count += 1;
                            }
                        }
                    }
                }
            }

            current_lsn = current_lsn + data.len() as u64;
        }

        count
    }
}
