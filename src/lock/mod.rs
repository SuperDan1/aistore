//! Lock module - Concurrency control
//!
//! Provides transaction management and locking for ACID compliance.

mod deadlock;
mod page_lock;
mod row_lock;
mod table_lock;
mod transaction;

#[cfg(test)]
mod tests;

pub(crate) use deadlock::DeadlockDetector;
pub(crate) use page_lock::PageLockManager;
pub(crate) use row_lock::{RowId as LockRowId, RowLockManager};
pub(crate) use table_lock::TableLockManager;
pub(crate) use transaction::{LockError, LockMode, LockResult, TransactionId, TransactionManager};

#[cfg(test)]
pub(crate) use transaction::TxStatus;

use crate::types::UndoPtr;
use std::time::Duration;

/// Unified Lock Manager
pub(crate) struct LockManager {
    tx_manager: TransactionManager,
    row_locks: RowLockManager,
    table_locks: TableLockManager,
    page_locks: PageLockManager,
    deadlock_detector: DeadlockDetector,
}

impl LockManager {
    pub fn new() -> Self {
        Self {
            tx_manager: TransactionManager::new(),
            row_locks: RowLockManager::new(),
            table_locks: TableLockManager::new(),
            page_locks: PageLockManager::new(),
            deadlock_detector: DeadlockDetector::new(),
        }
    }

    pub fn begin(&self) -> TransactionId {
        self.tx_manager.begin()
    }

    pub fn commit(&self, tx_id: TransactionId) -> Result<(), LockError> {
        self.row_locks.release_all(tx_id);
        self.table_locks.release_all(tx_id);
        self.page_locks.release_all(tx_id);
        self.tx_manager.commit(tx_id)
    }

    pub fn abort(&self, tx_id: TransactionId) -> Result<(), LockError> {
        self.row_locks.release_all(tx_id);
        self.table_locks.release_all(tx_id);
        self.page_locks.release_all(tx_id);
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

    pub fn lock_page(
        &self,
        tx_id: TransactionId,
        index_id: u64,
        page_id: u64,
        mode: LockMode,
    ) -> LockResult<()> {
        self.page_locks.lock(tx_id, index_id, page_id, mode)
    }

    pub fn unlock_page(&self, tx_id: TransactionId, index_id: u64, page_id: u64) {
        self.page_locks.unlock(tx_id, index_id, page_id);
    }

    pub fn set_timeout(&self, _duration: Duration) {}

    pub fn get_max_committed_tx(&self) -> TransactionId {
        self.tx_manager.get_max_committed_tx()
    }

    pub fn get_active_txns(&self) -> Vec<TransactionId> {
        self.tx_manager.get_active_txns()
    }

    pub fn set_last_undo_ptr(&self, tx_id: TransactionId, ptr: UndoPtr) {
        self.tx_manager.set_last_undo_ptr(tx_id, ptr);
    }

    pub fn get_last_undo_ptr(&self, tx_id: TransactionId) -> UndoPtr {
        self.tx_manager.get_last_undo_ptr(tx_id)
    }
}

impl Default for LockManager {
    fn default() -> Self {
        Self::new()
    }
}
