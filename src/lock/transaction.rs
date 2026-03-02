//! Transaction management

use crate::heap::RowId;
use crate::types::PageId;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    /// Check if two lock modes are compatible
    pub fn compatible(&self, other: &LockMode) -> bool {
        match (self, other) {
            (LockMode::Shared, LockMode::Shared) => true,
            _ => false,
        }
    }
}

/// Lock request
#[derive(Debug, Clone)]
pub struct LockRequest {
    pub resource: Resource,
    pub mode: LockMode,
    pub granted: bool,
}

/// Resource type
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Resource {
    Table(String),
    Row(String, PageId, usize), // table_name, page_id, slot_idx
}

/// Transaction
#[derive(Debug, Clone)]
pub struct Transaction {
    pub tx_id: TransactionId,
    pub status: TxStatus,
    pub start_time: Instant,
    pub locks: Vec<LockRequest>,
}

impl Transaction {
    pub fn new(tx_id: TransactionId) -> Self {
        Self {
            tx_id,
            status: TxStatus::Active,
            start_time: Instant::now(),
            locks: Vec::new(),
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

/// Transaction manager
pub struct TransactionManager {
    next_tx_id: AtomicU64,
    transactions: RwLock<HashMap<TransactionId, Transaction>>,
    lock_timeout: Duration,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            next_tx_id: AtomicU64::new(1),
            transactions: RwLock::new(HashMap::new()),
            lock_timeout: Duration::from_secs(30),
        }
    }

    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self {
            next_tx_id: AtomicU64::new(1),
            transactions: RwLock::new(HashMap::new()),
            lock_timeout: Duration::from_secs(timeout_secs),
        }
    }

    /// Begin a new transaction
    pub fn begin(&self) -> TransactionId {
        let tx_id = self.next_tx_id.fetch_add(1, Ordering::SeqCst);
        let tx = Transaction::new(tx_id);
        self.transactions.write().insert(tx_id, tx);
        tx_id
    }

    /// Get transaction
    pub fn get(&self, tx_id: TransactionId) -> Option<Transaction> {
        self.transactions.read().get(&tx_id).map(|t| t.clone())
    }

    /// Get transaction mutable
    pub fn get_mut(
        &self,
        tx_id: TransactionId,
    ) -> Option<parking_lot::RwLockWriteGuard<Transaction>> {
        self.transactions.write().get_mut(&tx_id).map(|_| {
            // Return a guard - this is a workaround
            unreachable!()
        })
    }

    /// Commit transaction
    pub fn commit(&self, tx_id: TransactionId) -> Result<(), LockError> {
        let mut txns = self.transactions.write();
        if let Some(tx) = txns.get_mut(&tx_id) {
            if tx.status != TxStatus::Active {
                return Err(LockError::TransactionNotActive);
            }
            tx.status = TxStatus::Committed;
            tx.locks.clear();
            Ok(())
        } else {
            Err(LockError::TransactionNotFound)
        }
    }

    /// Abort transaction
    pub fn abort(&self, tx_id: TransactionId) -> Result<(), LockError> {
        let mut txns = self.transactions.write();
        if let Some(tx) = txns.get_mut(&tx_id) {
            if tx.status != TxStatus::Active {
                return Err(LockError::TransactionNotActive);
            }
            tx.status = TxStatus::Aborted;
            tx.locks.clear();
            Ok(())
        } else {
            Err(LockError::TransactionNotFound)
        }
    }

    /// Set lock timeout
    pub fn set_timeout(&self, _duration: Duration) {
        // This would need interior mutability, simplified for now
    }

    /// Get timeout
    pub fn timeout(&self) -> Duration {
        self.lock_timeout
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Lock error
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

//
