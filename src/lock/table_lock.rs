//! Table-level locking

use super::{LockError, LockMode, LockResult, TransactionId};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Table lock entry
#[derive(Debug, Clone)]
struct TableLockEntry {
    holders: Vec<TableLockHolder>,
    waiters: Vec<TableLockWaiter>,
}

impl TableLockEntry {
    fn new() -> Self {
        Self {
            holders: Vec::new(),
            waiters: Vec::new(),
        }
    }

    fn can_grant(&self, tx_id: TransactionId, mode: LockMode) -> bool {
        if self.holders.is_empty() {
            return true;
        }

        for holder in &self.holders {
            if holder.tx_id == tx_id {
                continue;
            }
            if !holder.mode.compatible(&mode) {
                return false;
            }
        }
        true
    }

    fn add_holder(&mut self, tx_id: TransactionId, mode: LockMode) {
        if !self.holders.iter().any(|h| h.tx_id == tx_id) {
            self.holders.push(TableLockHolder { tx_id, mode });
        }
    }

    fn remove_holder(&mut self, tx_id: TransactionId) {
        self.holders.retain(|h| h.tx_id != tx_id);
    }
}

#[derive(Debug, Clone)]
struct TableLockHolder {
    tx_id: TransactionId,
    mode: LockMode,
}

#[derive(Debug, Clone)]
struct TableLockWaiter {
    tx_id: TransactionId,
    mode: LockMode,
    enqueue_time: Instant,
}

/// Table lock manager
pub struct TableLockManager {
    locks: RwLock<HashMap<String, TableLockEntry>>,
    timeout: Duration,
}

impl TableLockManager {
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

    /// Acquire a table lock
    pub fn lock(&self, tx_id: TransactionId, table_name: &str, mode: LockMode) -> LockResult<()> {
        let start = Instant::now();

        loop {
            if start.elapsed() > self.timeout {
                return Err(LockError::Timeout);
            }

            let entry = {
                let locks = self.locks.read();
                locks.get(table_name).cloned()
            };

            match entry {
                Some(mut entry) => {
                    if entry.can_grant(tx_id, mode) {
                        entry.add_holder(tx_id, mode);
                        self.locks.write().insert(table_name.to_string(), entry);
                        return Ok(());
                    }

                    drop(entry);
                    let mut locks = self.locks.write();
                    let entry = locks
                        .entry(table_name.to_string())
                        .or_insert_with(TableLockEntry::new);

                    if entry.can_grant(tx_id, mode) {
                        entry.add_holder(tx_id, mode);
                        return Ok(());
                    }

                    entry.waiters.push(TableLockWaiter {
                        tx_id,
                        mode,
                        enqueue_time: Instant::now(),
                    });
                }
                None => {
                    let mut entry = TableLockEntry::new();
                    entry.add_holder(tx_id, mode);
                    self.locks.write().insert(table_name.to_string(), entry);
                    return Ok(());
                }
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Release a table lock
    pub fn unlock(&self, tx_id: TransactionId, table_name: &str) {
        let mut locks = self.locks.write();
        if let Some(entry) = locks.get_mut(table_name) {
            entry.remove_holder(tx_id);

            if entry.holders.is_empty() && !entry.waiters.is_empty() {
                let waiter = entry.waiters.remove(0);
                entry.holders.push(TableLockHolder {
                    tx_id: waiter.tx_id,
                    mode: waiter.mode,
                });
            }

            if entry.holders.is_empty() && entry.waiters.is_empty() {
                locks.remove(table_name);
            }
        }
    }

    /// Release all locks for a transaction
    pub fn release_all(&self, tx_id: TransactionId) {
        let tables: Vec<String> = {
            let locks = self.locks.read();
            locks
                .iter()
                .filter(|(_, entry)| entry.holders.iter().any(|h| h.tx_id == tx_id))
                .map(|(name, _)| name.clone())
                .collect()
        };

        for table in tables {
            self.unlock(tx_id, &table);
        }
    }
}

impl Default for TableLockManager {
    fn default() -> Self {
        Self::new()
    }
}
