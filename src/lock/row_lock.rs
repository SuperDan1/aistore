//! Row-level locking

use super::{LockError, LockMode, LockResult, TransactionId};
use crate::types::PageId;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Row identifier (table + page + slot)
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RowId {
    pub table_name: String,
    pub page_id: PageId,
    pub slot_idx: usize,
}

impl RowId {
    pub fn new(table_name: String, page_id: PageId, slot_idx: usize) -> Self {
        Self {
            table_name,
            page_id,
            slot_idx,
        }
    }
}

/// Lock holder
#[derive(Debug, Clone)]
struct LockHolder {
    tx_id: TransactionId,
    mode: LockMode,
}

/// Lock waiter
#[derive(Debug, Clone)]
struct LockWaiter {
    tx_id: TransactionId,
    mode: LockMode,
    enqueue_time: Instant,
}

/// Row lock entry
#[derive(Debug, Clone)]
struct RowLockEntry {
    holders: Vec<LockHolder>,
    waiters: Vec<LockWaiter>,
}

impl RowLockEntry {
    fn new() -> Self {
        Self {
            holders: Vec::new(),
            waiters: Vec::new(),
        }
    }

    /// Check if a lock can be granted
    fn can_grant(&self, tx_id: TransactionId, mode: LockMode) -> bool {
        // If no holders, can grant
        if self.holders.is_empty() {
            return true;
        }

        // Check compatibility
        for holder in &self.holders {
            if holder.tx_id == tx_id {
                // Same transaction - upgrade S to X if needed
                if mode == LockMode::Exclusive && holder.mode == LockMode::Shared {
                    return false; // Need to wait for upgrade
                }
                continue;
            }
            if !holder.mode.compatible(&mode) {
                return false;
            }
        }
        true
    }

    /// Add a holder
    fn add_holder(&mut self, tx_id: TransactionId, mode: LockMode) {
        // Check if already holder
        if !self.holders.iter().any(|h| h.tx_id == tx_id) {
            self.holders.push(LockHolder { tx_id, mode });
        }
    }

    /// Remove a holder
    fn remove_holder(&mut self, tx_id: TransactionId) {
        self.holders.retain(|h| h.tx_id != tx_id);
    }
}

/// Row lock manager
pub struct RowLockManager {
    locks: RwLock<HashMap<RowId, RowLockEntry>>,
    timeout: Duration,
}

impl RowLockManager {
    pub fn new() -> Self {
        Self {
            locks: RwLock::new(HashMap::new()),
            timeout: Duration::from_secs(30),
        }
    }

    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self {
            locks: RwLock::new(HashMap::new()),
            timeout: Duration::from_secs(timeout_secs),
        }
    }

    /// Acquire a row lock
    pub fn lock(&self, tx_id: TransactionId, row_id: RowId, mode: LockMode) -> LockResult<()> {
        let start = Instant::now();

        loop {
            // Check timeout
            if start.elapsed() > self.timeout {
                return Err(LockError::Timeout);
            }

            // Get or create lock entry
            let entry = {
                let locks = self.locks.read();
                locks.get(&row_id).cloned()
            };

            match entry {
                Some(mut entry) => {
                    // Check if can grant
                    if entry.can_grant(tx_id, mode) {
                        entry.add_holder(tx_id, mode);
                        self.locks.write().insert(row_id, entry);
                        return Ok(());
                    }

                    // Add to waiters
                    drop(entry);
                    let mut locks = self.locks.write();
                    let entry = locks
                        .entry(row_id.clone())
                        .or_insert_with(RowLockEntry::new);

                    // Double check after acquiring write lock
                    if entry.can_grant(tx_id, mode) {
                        entry.add_holder(tx_id, mode);
                        return Ok(());
                    }

                    // Add to waiters
                    entry.waiters.push(LockWaiter {
                        tx_id,
                        mode,
                        enqueue_time: Instant::now(),
                    });
                }
                None => {
                    // No lock exists, create new one
                    let mut entry = RowLockEntry::new();
                    entry.add_holder(tx_id, mode);
                    self.locks.write().insert(row_id, entry);
                    return Ok(());
                }
            }

            // Wait a bit before retrying
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Release a row lock
    pub fn unlock(&self, tx_id: TransactionId, row_id: &RowId) {
        let mut locks = self.locks.write();
        if let Some(entry) = locks.get_mut(row_id) {
            entry.remove_holder(tx_id);

            // If no more holders, promote waiters
            if entry.holders.is_empty() && !entry.waiters.is_empty() {
                // Grant to first waiter
                let waiter = entry.waiters.remove(0);
                entry.holders.push(LockHolder {
                    tx_id: waiter.tx_id,
                    mode: waiter.mode,
                });
            }

            // Remove entry if no holders and no waiters
            if entry.holders.is_empty() && entry.waiters.is_empty() {
                locks.remove(row_id);
            }
        }
    }

    /// Get all locks held by a transaction
    pub fn get_locks(&self, tx_id: TransactionId) -> Vec<RowId> {
        let locks = self.locks.read();
        let locks = self.locks.read();
        locks
            .iter()
            .filter(|(_, entry)| entry.holders.iter().any(|h| h.tx_id == tx_id))
            .map(|(row_id, _)| row_id.clone())
            .collect()
    }

    /// Release all locks for a transaction
    pub fn release_all(&self, tx_id: TransactionId) {
        let locks_to_release = self.get_locks(tx_id);
        for row_id in locks_to_release {
            self.unlock(tx_id, &row_id);
        }
    }

    /// Check for deadlock (simple version)
    pub fn check_deadlock(&self, tx_id: TransactionId) -> bool {
        // Simplified: check if tx is waiting for a lock held by another tx that's waiting
        let locks = self.locks.read();

        // Find all transactions this tx is waiting on
        let mut waiting_on: Vec<TransactionId> = Vec::new();
        for (_, entry) in locks.iter() {
            for waiter in &entry.waiters {
                if waiter.tx_id == tx_id {
                    // This tx is waiting - find who it's waiting for
                    for holder in &entry.holders {
                        if holder.tx_id != tx_id {
                            waiting_on.push(holder.tx_id);
                        }
                    }
                }
            }
        }

        // Check if any of those transactions are waiting on this tx
        for wait_tx in waiting_on {
            for (_, entry) in locks.iter() {
                for waiter in &entry.waiters {
                    if waiter.tx_id == wait_tx {
                        for holder in &entry.holders {
                            if holder.tx_id == tx_id {
                                return true; // Deadlock detected
                            }
                        }
                    }
                }
            }
        }

        false
    }
}

impl Default for RowLockManager {
    fn default() -> Self {
        Self::new()
    }
}
