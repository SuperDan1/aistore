//! Transaction management

use crate::types::{PageId, LSN};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Transaction ID type
pub type TransactionId = u64;

/// Transaction status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxStatus {
    Active,
    Committed,
    Aborted,
}

/// Lock mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockMode {
    Shared,    // S lock - for read
    Exclusive, // X lock - for write
}

impl LockMode {
    pub fn compatible(&self, other: &LockMode) -> bool {
        match (self, other) {
            (LockMode::Shared, LockMode::Shared) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LockRequest {
    pub resource: Resource,
    pub mode: LockMode,
    pub granted: bool,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Resource {
    Table(String),
    Row(String, PageId, usize),
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub tx_id: TransactionId,
    pub status: TxStatus,
    pub start_time: Instant,
    pub locks: Vec<LockRequest>,
    pub start_lsn: LSN,
    pub last_undo_ptr: Option<crate::types::UndoPtr>,
}

impl Transaction {
    pub fn new(tx_id: TransactionId) -> Self {
        Self {
            tx_id,
            status: TxStatus::Active,
            start_time: Instant::now(),
            locks: Vec::new(),
            start_lsn: 0,
            last_undo_ptr: None,
        }
    }

    pub fn add_lock(&mut self, resource: Resource, mode: LockMode) {
        self.locks.push(LockRequest {
            resource,
            mode,
            granted: true,
        });
    }

    pub fn release_all_locks(&mut self) {
        self.locks.clear();
    }
}

/// Transaction state for MVCC
#[derive(Debug, Clone, Copy)]
pub struct TxState {
    pub tx_id: TransactionId,
    pub status: TxStatus,
    pub start_lsn: LSN,
    pub end_lsn: LSN,
}

pub struct TransactionManager {
    next_tx_id: AtomicU64,
    transactions: RwLock<HashMap<TransactionId, Transaction>>,
    lock_timeout: Duration,
    active_txns: RwLock<HashSet<TransactionId>>,
    max_committed_tx: AtomicU64,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            next_tx_id: AtomicU64::new(1),
            transactions: RwLock::new(HashMap::new()),
            lock_timeout: Duration::from_secs(30),
            active_txns: RwLock::new(HashSet::new()),
            max_committed_tx: AtomicU64::new(0),
        }
    }

    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self {
            next_tx_id: AtomicU64::new(1),
            transactions: RwLock::new(HashMap::new()),
            lock_timeout: Duration::from_secs(timeout_secs),
            active_txns: RwLock::new(HashSet::new()),
            max_committed_tx: AtomicU64::new(0),
        }
    }

    pub fn begin(&self) -> TransactionId {
        let tx_id = self.next_tx_id.fetch_add(1, Ordering::SeqCst);
        let tx = Transaction::new(tx_id);
        self.active_txns.blocking_write().insert(tx_id);
        self.transactions.blocking_write().insert(tx_id, tx);
        tx_id
    }

    pub fn get(&self, tx_id: TransactionId) -> Option<Transaction> {
        self.transactions.blocking_read().get(&tx_id).cloned()
    }

    pub fn get_mut(
        &self,
        tx_id: TransactionId,
    ) -> Option<parking_lot::RwLockWriteGuard<Transaction>> {
        self.transactions
            .blocking_write()
            .get_mut(&tx_id)
            .map(|_| unreachable!())
    }

    pub fn commit(&self, tx_id: TransactionId) -> Result<(), LockError> {
        let mut txns = self.transactions.blocking_write();
        if let Some(tx) = txns.get_mut(&tx_id) {
            if tx.status != TxStatus::Active {
                return Err(LockError::TransactionNotActive);
            }
            tx.status = TxStatus::Committed;
            tx.locks.clear();
        } else {
            return Err(LockError::TransactionNotFound);
        }

        self.active_txns.blocking_write().remove(&tx_id);

        let mut current_max = self.max_committed_tx.load(Ordering::SeqCst);
        while tx_id > current_max {
            if self
                .max_committed_tx
                .compare_exchange(current_max, tx_id, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                break;
            }
            current_max = self.max_committed_tx.load(Ordering::SeqCst);
        }

        Ok(())
    }

    pub fn abort(&self, tx_id: TransactionId) -> Result<(), LockError> {
        let mut txns = self.transactions.blocking_write();
        if let Some(tx) = txns.get_mut(&tx_id) {
            if tx.status != TxStatus::Active {
                return Err(LockError::TransactionNotActive);
            }
            tx.status = TxStatus::Aborted;
            tx.locks.clear();
        } else {
            return Err(LockError::TransactionNotFound);
        }

        self.active_txns.blocking_write().remove(&tx_id);

        Ok(())
    }

    pub fn set_timeout(&self, _duration: Duration) {}

    pub fn timeout(&self) -> Duration {
        self.lock_timeout
    }

    pub fn get_active_txns(&self) -> Vec<TransactionId> {
        self.active_txns.blocking_read().iter().copied().collect()
    }

    pub fn get_max_committed_tx(&self) -> TransactionId {
        self.max_committed_tx.load(Ordering::SeqCst)
    }

    pub fn is_active(&self, tx_id: TransactionId) -> bool {
        self.active_txns.blocking_read().contains(&tx_id)
    }

    pub fn set_start_lsn(&self, tx_id: TransactionId, lsn: LSN) {
        if let Some(tx) = self.transactions.blocking_write().get_mut(&tx_id) {
            tx.start_lsn = lsn;
        }
    }

    pub fn set_last_undo_ptr(&self, tx_id: TransactionId, ptr: crate::types::UndoPtr) {
        if let Some(tx) = self.transactions.blocking_write().get_mut(&tx_id) {
            tx.last_undo_ptr = Some(ptr);
        }
    }

    pub fn get_last_undo_ptr(&self, tx_id: TransactionId) -> crate::types::UndoPtr {
        self.transactions
            .blocking_read()
            .get(&tx_id)
            .and_then(|tx| tx.last_undo_ptr)
            .unwrap_or(crate::types::UndoPtr::null())
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub enum LockError {
    Timeout,
    Deadlock,
    TransactionNotFound,
    TransactionNotActive,
    ResourceNotFound,
    Conflict,
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockError::Timeout => write!(f, "Lock timeout"),
            LockError::Deadlock => write!(f, "Deadlock detected"),
            LockError::TransactionNotFound => write!(f, "Transaction not found"),
            LockError::TransactionNotActive => write!(f, "Transaction not active"),
            LockError::ResourceNotFound => write!(f, "Resource not found"),
            LockError::Conflict => write!(f, "Lock conflict"),
        }
    }
}

impl std::error::Error for LockError {}

pub type LockResult<T> = Result<T, LockError>;
