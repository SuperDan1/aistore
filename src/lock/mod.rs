//! Lock module - Concurrency control
//!
//! Provides transaction management and locking for ACID compliance.

pub mod deadlock;
pub mod row_lock;
pub mod table_lock;
pub mod transaction;

pub use deadlock::DeadlockDetector;
pub use row_lock::{RowId as LockRowId, RowLockManager};
pub use table_lock::TableLockManager;
pub use transaction::{
    LockError, LockMode, LockResult, Transaction, TransactionId, TransactionManager, TxStatus,
};

use std::time::Duration;

/// Unified Lock Manager
pub struct LockManager {
    tx_manager: TransactionManager,
    row_locks: RowLockManager,
    table_locks: TableLockManager,
    deadlock_detector: DeadlockDetector,
}

impl LockManager {
    pub fn new() -> Self {
        Self {
            tx_manager: TransactionManager::new(),
            row_locks: RowLockManager::new(),
            table_locks: TableLockManager::new(),
            deadlock_detector: DeadlockDetector::new(),
        }
    }

    pub fn begin(&self) -> TransactionId {
        self.tx_manager.begin()
    }

    pub fn commit(&self, tx_id: TransactionId) -> Result<(), LockError> {
        self.row_locks.release_all(tx_id);
        self.table_locks.release_all(tx_id);
        self.tx_manager.commit(tx_id)
    }

    pub fn abort(&self, tx_id: TransactionId) -> Result<(), LockError> {
        self.row_locks.release_all(tx_id);
        self.table_locks.release_all(tx_id);
        self.tx_manager.abort(tx_id)
    }

    pub fn lock_row(
        &self,
        tx_id: TransactionId,
        table: &str,
        page_id: u64,
        slot_idx: usize,
        mode: LockMode,
    ) -> Result<(), LockError> {
        let row_id = LockRowId::new(table.to_string(), page_id, slot_idx);
        self.row_locks.lock(tx_id, row_id, mode)
    }

    pub fn unlock_row(&self, tx_id: TransactionId, table: &str, page_id: u64, slot_idx: usize) {
        let row_id = LockRowId::new(table.to_string(), page_id, slot_idx);
        self.row_locks.unlock(tx_id, &row_id);
    }

    pub fn lock_table(
        &self,
        tx_id: TransactionId,
        table: &str,
        mode: LockMode,
    ) -> Result<(), LockError> {
        self.table_locks.lock(tx_id, table, mode)
    }

    pub fn unlock_table(&self, tx_id: TransactionId, table: &str) {
        self.table_locks.unlock(tx_id, table);
    }

    pub fn set_timeout(&self, _duration: Duration) {
        // Timeout would need interior mutability - simplified for now
    }
}

impl Default for LockManager {
    fn default() -> Self {
        Self::new()
    }
}
