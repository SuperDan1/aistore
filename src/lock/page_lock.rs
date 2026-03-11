//! Page-level locking for index operations

use super::{LockError, LockMode, LockResult, TransactionId};
use crate::types::PageId;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Page identifier (index_id + page_id)
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct PageLockId {
    pub index_id: u64,
    pub page_id: PageId,
}

impl PageLockId {
    pub fn new(index_id: u64, page_id: PageId) -> Self {
        Self { index_id, page_id }
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

/// Page lock entry
#[derive(Debug, Clone)]
struct PageLockEntry {
    holders: Vec<LockHolder>,
    waiters: Vec<LockWaiter>,
}

impl PageLockEntry {
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
                if mode == LockMode::Exclusive && holder.mode == LockMode::Shared {
                    return false;
                }
                continue;
            }
            if !holder.mode.compatible(&mode) {
                return false;
            }
        }
        true
    }

    fn add_holder(&mut self, tx_id: TransactionId, mode: LockMode) {
        self.holders.push(LockHolder { tx_id, mode });
    }

    fn remove_holder(&mut self, tx_id: TransactionId) {
        self.holders.retain(|h| h.tx_id != tx_id);
    }

    fn add_waiter(&mut self, tx_id: TransactionId, mode: LockMode) {
        self.waiters.push(LockWaiter {
            tx_id,
            mode,
            enqueue_time: Instant::now(),
        });
    }

    fn remove_waiter(&mut self, tx_id: TransactionId) {
        self.waiters.retain(|w| w.tx_id != tx_id);
    }

    fn grant_next_waiter(&mut self) -> Option<(TransactionId, LockMode)> {
        if self.waiters.is_empty() {
            return None;
        }

        let idx = self
            .waiters
            .iter()
            .position(|w| self.can_grant(w.tx_id, w.mode));

        if let Some(idx) = idx {
            let waiter = self.waiters.remove(idx);
            return Some((waiter.tx_id, waiter.mode));
        }
        None
    }
}

/// Page lock manager - provides page-level locking for index operations
pub struct PageLockManager {
    locks: RwLock<HashMap<PageLockId, PageLockEntry>>,
    lock_timeout: Duration,
}

impl PageLockManager {
    pub fn new() -> Self {
        Self {
            locks: RwLock::new(HashMap::new()),
            lock_timeout: Duration::from_secs(10),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.lock_timeout = timeout;
        self
    }

    /// Acquire a lock on a page
    pub fn lock(
        &self,
        tx_id: TransactionId,
        index_id: u64,
        page_id: PageId,
        mode: LockMode,
    ) -> LockResult<()> {
        let start = Instant::now();

        loop {
            let page_lock_id = PageLockId::new(index_id, page_id);

            {
                let locks = self.locks.read();
                if let Some(entry) = locks.get(&page_lock_id) {
                    if entry.can_grant(tx_id, mode) {
                        drop(locks);
                        let mut locks = self.locks.write();
                        let entry = locks.entry(page_lock_id).or_insert_with(PageLockEntry::new);
                        entry.add_holder(tx_id, mode);
                        return Ok(());
                    }
                }
            }

            {
                let page_lock_id = PageLockId::new(index_id, page_id);
                let mut locks = self.locks.write();
                let entry = locks.entry(page_lock_id).or_insert_with(PageLockEntry::new);

                if entry.can_grant(tx_id, mode) {
                    entry.add_holder(tx_id, mode);
                    return Ok(());
                }

                if !entry.waiters.iter().any(|w| w.tx_id == tx_id) {
                    entry.add_waiter(tx_id, mode);
                }
            }

            if start.elapsed() > self.lock_timeout {
                let page_lock_id = PageLockId::new(index_id, page_id);
                let mut locks = self.locks.write();
                if let Some(entry) = locks.get_mut(&page_lock_id) {
                    entry.remove_waiter(tx_id);
                }
                return Err(LockError::Timeout);
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Release a lock on a page
    pub fn unlock(&self, tx_id: TransactionId, index_id: u64, page_id: PageId) {
        let page_lock_id = PageLockId::new(index_id, page_id);
        let mut locks = self.locks.write();

        if let Some(entry) = locks.get_mut(&page_lock_id) {
            entry.remove_holder(tx_id);

            while let Some((waiter_tx_id, mode)) = entry.grant_next_waiter() {
                entry.add_holder(waiter_tx_id, mode);
            }

            if entry.holders.is_empty() && entry.waiters.is_empty() {
                locks.remove(&page_lock_id);
            }
        }
    }

    /// Release all locks held by a transaction
    pub fn release_all(&self, tx_id: TransactionId) {
        let mut locks = self.locks.write();
        let keys: Vec<_> = locks.keys().cloned().collect();

        for key in keys {
            if let Some(entry) = locks.get_mut(&key) {
                entry.remove_holder(tx_id);
                entry.remove_waiter(tx_id);

                while let Some((waiter_tx_id, mode)) = entry.grant_next_waiter() {
                    if waiter_tx_id != tx_id {
                        entry.add_holder(waiter_tx_id, mode);
                    }
                }

                if entry.holders.is_empty() && entry.waiters.is_empty() {
                    locks.remove(&key);
                }
            }
        }
    }

    /// Check if a page is locked by transaction
    pub fn is_locked(&self, index_id: u64, page_id: PageId) -> bool {
        let page_lock_id = PageLockId::new(index_id, page_id);
        let locks = self.locks.read();
        locks.contains_key(&page_lock_id)
    }

    /// Get lock info for debugging
    pub fn lock_info(&self) -> Vec<(PageLockId, Vec<(TransactionId, LockMode)>)> {
        let locks = self.locks.read();
        locks
            .iter()
            .map(|(key, entry)| {
                let holders = entry.holders.iter().map(|h| (h.tx_id, h.mode)).collect();
                (key.clone(), holders)
            })
            .collect()
    }
}

impl Default for PageLockManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_lock_basic() {
        let lock_mgr = PageLockManager::new();
        let tx_id = 1;

        lock_mgr.lock(tx_id, 1, 100, LockMode::Shared).unwrap();
        assert!(lock_mgr.is_locked(1, 100));

        lock_mgr.unlock(tx_id, 1, 100);
        assert!(!lock_mgr.is_locked(1, 100));
    }

    #[test]
    fn test_page_lock_shared_compatible() {
        let lock_mgr = PageLockManager::new();

        lock_mgr.lock(1, 1, 100, LockMode::Shared).unwrap();
        lock_mgr.lock(2, 1, 100, LockMode::Shared).unwrap();

        lock_mgr.unlock(1, 1, 100);
        lock_mgr.unlock(2, 1, 100);
    }

    #[test]
    fn test_page_lock_exclusive_conflict() {
        let lock_mgr = PageLockManager::new();

        lock_mgr.lock(1, 1, 100, LockMode::Exclusive).unwrap();

        let result = lock_mgr.lock(2, 1, 100, LockMode::Shared);
        assert!(result.is_err());

        lock_mgr.unlock(1, 1, 100);
    }
}
