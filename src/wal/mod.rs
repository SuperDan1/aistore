//! WAL Module - Write-Ahead Logging for Aistore
//!
//! This module provides WAL functionality for the storage engine.

pub mod checkpoint;
pub mod config;
pub mod log_buffer;
pub mod log_file;
pub mod log_record;
pub mod lsn;
pub mod recovery;

use checkpoint::CheckpointManager;
use config::WalConfig;
use log_buffer::LogBuffer;
use log_file::LogFileManager;
use log_record::LogRecord;
use lsn::LSN;
use recovery::{RecoveryManager, RecoveryResult};

use crate::lock::TransactionId;
use crate::types::PageId;
use crate::vfs::VfsInterface;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// WAL error
#[derive(Debug)]
pub enum WalError {
    IoError(String),
    NotFound(String),
    InvalidState(String),
}

impl std::fmt::Display for WalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WalError::IoError(e) => write!(f, "WAL I/O error: {}", e),
            WalError::NotFound(e) => write!(f, "WAL not found: {}", e),
            WalError::InvalidState(e) => write!(f, "WAL invalid state: {}", e),
        }
    }
}

impl std::error::Error for WalError {}

pub type WalResult<T> = Result<T, WalError>;

/// Transaction LSN tracking
#[allow(dead_code)]
struct TxLsn {
    prev_lsn: LSN,
    commit_lsn: LSN, // Kept for future use: tracking commit LSN for recovery
}

/// WAL Manager
pub struct WalManager {
    config: WalConfig,
    file_mgr: Arc<LogFileManager>,
    buffer: Arc<LogBuffer>,
    checkpoint_mgr: RwLock<CheckpointManager>,
    recovery_mgr: RwLock<Option<RecoveryManager>>,
    tx_lsns: RwLock<std::collections::HashMap<TransactionId, TxLsn>>,
    enabled: bool,
}

impl WalManager {
    /// Create a new WAL manager
    pub fn new(data_dir: PathBuf, vfs: Arc<dyn VfsInterface>) -> WalResult<Self> {
        let config = WalConfig::new()
            .with_log_dir(data_dir.join("wal"))
            .with_checkpoint_interval(60);

        let file_mgr = Arc::new(
            LogFileManager::new(config.clone(), Arc::clone(&vfs))
                .map_err(|e| WalError::IoError(e.to_string()))?,
        );

        let buffer = LogBuffer::new(config.clone(), Arc::clone(&file_mgr));

        let checkpoint_dir = data_dir.join("checkpoint");
        let checkpoint_mgr = CheckpointManager::new(checkpoint_dir, Arc::clone(&vfs));

        let recovery_mgr = RecoveryManager::new(
            config.clone(),
            Arc::clone(&file_mgr),
            checkpoint_mgr.clone(),
            vfs,
        );

        let manager = Self {
            config,
            file_mgr,
            buffer,
            checkpoint_mgr: RwLock::new(checkpoint_mgr),
            recovery_mgr: RwLock::new(Some(recovery_mgr)),
            tx_lsns: RwLock::new(std::collections::HashMap::new()),
            enabled: true,
        };

        manager.start_checkpoint_timer();

        Ok(manager)
    }

    /// Append a log record
    pub fn append(&self, tx_id: TransactionId, record: LogRecord) -> LSN {
        if !self.enabled {
            return LSN::invalid();
        }

        let prev_lsn = self
            .tx_lsns
            .read()
            .get(&tx_id)
            .map(|t| t.prev_lsn)
            .unwrap_or(LSN::invalid());

        let data = record.serialize();
        let lsn = match self.buffer.append(tx_id, data, None) {
            Ok(lsn) => lsn,
            Err(e) => {
                eprintln!("WAL: Failed to append record: {}", e);
                return LSN::invalid();
            }
        };

        self.tx_lsns.write().insert(
            tx_id,
            TxLsn {
                prev_lsn,
                commit_lsn: lsn,
            },
        );

        lsn
    }

    /// Begin a transaction
    pub fn tx_begin(&self, tx_id: TransactionId) -> LSN {
        let record = LogRecord::tx_begin(tx_id, LSN::invalid());
        self.append(tx_id, record)
    }

    /// Commit a transaction
    pub fn commit(&self, tx_id: TransactionId) -> WalResult<LSN> {
        if !self.enabled {
            return Ok(LSN::invalid());
        }

        let tx_lsn = self.tx_lsns.write().remove(&tx_id);
        let (prev_lsn, commit_lsn) = tx_lsn
            .as_ref()
            .map(|t| (t.prev_lsn, t.commit_lsn))
            .unwrap_or((LSN::invalid(), LSN::invalid()));

        let record = LogRecord::tx_commit(tx_id, prev_lsn);
        self.append(tx_id, record);

        self.buffer.flush().map_err(|e| WalError::IoError(e))?;

        Ok(commit_lsn)
    }

    /// Abort a transaction
    pub fn abort(&self, tx_id: TransactionId) -> WalResult<()> {
        if !self.enabled {
            return Ok(());
        }

        let tx_lsn = self.tx_lsns.write().remove(&tx_id);
        let prev_lsn = tx_lsn
            .as_ref()
            .map(|t| t.prev_lsn)
            .unwrap_or(LSN::invalid());

        let record = LogRecord::tx_abort(tx_id, prev_lsn);
        self.append(tx_id, record);

        self.buffer.flush().map_err(|e| WalError::IoError(e))?;

        Ok(())
    }

    /// Write page redo log
    pub fn write_page_redo(
        &self,
        tx_id: TransactionId,
        page_id: PageId,
        offset: u32,
        data: &[u8],
    ) -> LSN {
        if !self.enabled {
            return LSN::invalid();
        }

        let prev_lsn = self
            .tx_lsns
            .read()
            .get(&tx_id)
            .map(|t| t.prev_lsn)
            .unwrap_or(LSN::invalid());

        let record = LogRecord::page_redo(tx_id, prev_lsn, page_id, offset, data.to_vec());
        self.append(tx_id, record)
    }

    /// Flush dirty pages and write redo logs
    pub fn flush_dirty_pages(
        &self,
        get_dirty_pages: impl Fn() -> Vec<PageId>,
        get_page_data: impl Fn(PageId) -> Option<Vec<u8>>,
    ) -> WalResult<()> {
        if !self.enabled {
            return Ok(());
        }

        let dirty_pages = get_dirty_pages();

        for page_id in dirty_pages {
            if let Some(page_data) = get_page_data(page_id) {
                self.write_page_redo(0, page_id, 0, &page_data);
            }
        }

        self.buffer.flush().map_err(|e| WalError::IoError(e))?;
        Ok(())
    }

    /// Force flush
    pub fn flush(&self) -> WalResult<()> {
        self.buffer.flush().map_err(|e| WalError::IoError(e))
    }

    /// Get current LSN
    pub fn current_lsn(&self) -> LSN {
        self.file_mgr.current_lsn()
    }

    /// Perform checkpoint with provided dirty pages
    pub fn checkpoint(&self, dirty_pages: Vec<PageId>) -> WalResult<LSN> {
        let active_transactions: Vec<u64> = self.tx_lsns.read().keys().cloned().collect();

        let lsn = self.file_mgr.current_lsn();

        let mut mgr = self.checkpoint_mgr.write();
        mgr.checkpoint(lsn, dirty_pages, active_transactions)
            .map_err(|e| WalError::IoError(e.to_string()))?;

        let _ = self.file_mgr.cleanup_old_logs(lsn);

        Ok(lsn)
    }

    /// Recover from crash
    pub fn recover(&self) -> RecoveryResult {
        if let Some(ref mgr) = *self.recovery_mgr.read() {
            mgr.recover(|_, _| Ok(()))
        } else {
            RecoveryResult {
                checkpoint_lsn: LSN::invalid(),
                replayed_records: 0,
                rolled_back_transactions: Vec::new(),
            }
        }
    }

    /// Start checkpoint timer
    fn start_checkpoint_timer(&self) {
        let interval = self.config.checkpoint_interval_sec;
        if interval == 0 {
            return;
        }

        let buffer = Arc::clone(&self.buffer);
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(interval));
            let _ = buffer.flush();
        });
    }

    /// Enable/disable WAL
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if WAL is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Drop for WalManager {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}
