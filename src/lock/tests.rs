#![cfg(test)]

use crate::lock::{
    LockManager, LockMode, LockRowId, RowLockManager, TableLockManager, TransactionId,
    TransactionManager, TxStatus,
};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

mod transaction_tests {
    use super::*;

    #[test]
    fn test_transaction_begin() {
        let mgr = TransactionManager::new();
        let tx_id = mgr.begin();
        assert_eq!(tx_id, 1);
        assert!(mgr.is_active(tx_id));
    }

    #[test]
    fn test_transaction_begin_increments() {
        let mgr = TransactionManager::new();
        let tx1 = mgr.begin();
        let tx2 = mgr.begin();
        let tx3 = mgr.begin();
        assert_eq!(tx1, 1);
        assert_eq!(tx2, 2);
        assert_eq!(tx3, 3);
    }

    #[test]
    fn test_transaction_commit() {
        let mgr = TransactionManager::new();
        let tx_id = mgr.begin();

        let result = mgr.commit(tx_id);
        assert!(result.is_ok());

        assert!(!mgr.is_active(tx_id));
        let tx = mgr.get(tx_id).unwrap();
        assert_eq!(tx.status, TxStatus::Committed);
    }

    #[test]
    fn test_transaction_abort() {
        let mgr = TransactionManager::new();
        let tx_id = mgr.begin();

        let result = mgr.abort(tx_id);
        assert!(result.is_ok());

        assert!(!mgr.is_active(tx_id));
        let tx = mgr.get(tx_id).unwrap();
        assert_eq!(tx.status, TxStatus::Aborted);
    }

    #[test]
    fn test_transaction_commit_not_active() {
        let mgr = TransactionManager::new();
        let tx_id = mgr.begin();
        mgr.commit(tx_id).unwrap();

        let result = mgr.commit(tx_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_transaction_abort_not_active() {
        let mgr = TransactionManager::new();
        let tx_id = mgr.begin();
        mgr.abort(tx_id).unwrap();

        let result = mgr.abort(tx_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_transaction_get_not_found() {
        let mgr = TransactionManager::new();
        let result = mgr.get(999);
        assert!(result.is_none());
    }

    #[test]
    fn test_transaction_max_committed_tx() {
        let mgr = TransactionManager::new();
        let tx1 = mgr.begin();
        let tx2 = mgr.begin();

        mgr.commit(tx1).unwrap();
        assert_eq!(mgr.get_max_committed_tx(), tx1);

        mgr.commit(tx2).unwrap();
        assert_eq!(mgr.get_max_committed_tx(), tx2);
    }

    #[test]
    fn test_get_active_txns() {
        let mgr = TransactionManager::new();
        let tx1 = mgr.begin();
        let tx2 = mgr.begin();

        let active = mgr.get_active_txns();
        assert!(active.contains(&tx1));
        assert!(active.contains(&tx2));

        mgr.commit(tx1).unwrap();
        let active = mgr.get_active_txns();
        assert!(!active.contains(&tx1));
        assert!(active.contains(&tx2));
    }

    #[test]
    fn test_set_start_lsn() {
        let mgr = TransactionManager::new();
        let tx_id = mgr.begin();

        mgr.set_start_lsn(tx_id, 100);

        let tx = mgr.get(tx_id).unwrap();
        assert_eq!(tx.start_lsn, 100);
    }

    #[test]
    fn test_undo_ptr_operations() {
        let mgr = TransactionManager::new();
        let tx_id = mgr.begin();

        let ptr = crate::types::UndoPtr {
            page_id: 1,
            offset: 100,
            lsn: 0,
        };
        mgr.set_last_undo_ptr(tx_id, ptr);

        let result = mgr.get_last_undo_ptr(tx_id);
        assert_eq!(result.page_id, 1);
        assert_eq!(result.offset, 100);
    }

    #[test]
    fn test_undo_ptr_null() {
        let mgr = TransactionManager::new();
        let tx_id = mgr.begin();

        let result = mgr.get_last_undo_ptr(tx_id);
        assert!(result.is_null());
    }
}

mod lock_mode_tests {
    use super::*;

    #[test]
    fn test_lock_mode_compatible_shared_shared() {
        assert!(LockMode::Shared.compatible(&LockMode::Shared));
    }

    #[test]
    fn test_lock_mode_compatible_shared_exclusive() {
        assert!(!LockMode::Shared.compatible(&LockMode::Exclusive));
    }

    #[test]
    fn test_lock_mode_compatible_exclusive_shared() {
        assert!(!LockMode::Exclusive.compatible(&LockMode::Shared));
    }

    #[test]
    fn test_lock_mode_compatible_exclusive_exclusive() {
        assert!(!LockMode::Exclusive.compatible(&LockMode::Exclusive));
    }
}

mod row_lock_tests {
    use super::*;

    #[test]
    fn test_row_lock_exclusive() {
        let mgr = RowLockManager::new();
        let tx_id: TransactionId = 1;
        let row_id = LockRowId::new("test".to_string(), 1, 0);

        let result = mgr.lock(tx_id, row_id.clone(), LockMode::Exclusive);
        assert!(result.is_ok());
    }

    #[test]
    fn test_row_lock_shared() {
        let mgr = RowLockManager::new();
        let tx_id: TransactionId = 1;
        let row_id = LockRowId::new("test".to_string(), 1, 0);

        let result = mgr.lock(tx_id, row_id.clone(), LockMode::Shared);
        assert!(result.is_ok());
    }

    #[test]
    fn test_row_lock_shared_multiple_readers() {
        let mgr = RowLockManager::new();
        let row_id = LockRowId::new("test".to_string(), 1, 0);

        let result1 = mgr.lock(1, row_id.clone(), LockMode::Shared);
        let result2 = mgr.lock(2, row_id.clone(), LockMode::Shared);

        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }

    #[test]
    fn test_row_lock_multiple_different_rows() {
        let mgr = RowLockManager::new();

        let row1 = LockRowId::new("test".to_string(), 1, 0);
        let row2 = LockRowId::new("test".to_string(), 1, 1);

        assert!(mgr.lock(1, row1.clone(), LockMode::Exclusive).is_ok());
        assert!(mgr.lock(1, row2.clone(), LockMode::Exclusive).is_ok());
    }

    #[test]
    fn test_row_lock_unlock() {
        let mgr = RowLockManager::new();
        let tx_id: TransactionId = 1;
        let row_id = LockRowId::new("test".to_string(), 1, 0);

        mgr.lock(tx_id, row_id.clone(), LockMode::Exclusive)
            .unwrap();
        mgr.unlock(tx_id, &row_id);

        let result = mgr.lock(tx_id, row_id.clone(), LockMode::Shared);
        assert!(result.is_ok());
    }

    #[test]
    fn test_row_lock_release_all() {
        let mgr = RowLockManager::new();
        let tx_id: TransactionId = 1;
        let row_id1 = LockRowId::new("test".to_string(), 1, 0);
        let row_id2 = LockRowId::new("test".to_string(), 1, 1);

        mgr.lock(tx_id, row_id1.clone(), LockMode::Exclusive)
            .unwrap();
        mgr.lock(tx_id, row_id2.clone(), LockMode::Exclusive)
            .unwrap();

        mgr.release_all(tx_id);

        let result1 = mgr.lock(tx_id, row_id1, LockMode::Exclusive);
        let result2 = mgr.lock(tx_id, row_id2, LockMode::Exclusive);

        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }
}

mod table_lock_tests {
    use super::*;

    #[test]
    fn test_table_lock_exclusive() {
        let mgr = TableLockManager::new();
        let tx_id: TransactionId = 1;

        let result = mgr.lock(tx_id, "test_table", LockMode::Exclusive);
        assert!(result.is_ok());
    }

    #[test]
    fn test_table_lock_shared() {
        let mgr = TableLockManager::new();
        let tx_id: TransactionId = 1;

        let result = mgr.lock(tx_id, "test_table", LockMode::Shared);
        assert!(result.is_ok());
    }

    #[test]
    fn test_table_lock_unlock() {
        let mgr = TableLockManager::new();
        let tx_id: TransactionId = 1;

        mgr.lock(tx_id, "test_table", LockMode::Exclusive).unwrap();
        mgr.unlock(tx_id, "test_table");

        let result = mgr.lock(tx_id, "test_table", LockMode::Shared);
        assert!(result.is_ok());
    }

    #[test]
    fn test_table_lock_release_all() {
        let mgr = TableLockManager::new();
        let tx_id: TransactionId = 1;

        mgr.lock(tx_id, "table1", LockMode::Exclusive).unwrap();
        mgr.lock(tx_id, "table2", LockMode::Exclusive).unwrap();

        mgr.release_all(tx_id);

        assert!(mgr.lock(tx_id, "table1", LockMode::Shared).is_ok());
        assert!(mgr.lock(tx_id, "table2", LockMode::Shared).is_ok());
    }

    #[test]
    fn test_table_lock_different_tables() {
        let mgr = TableLockManager::new();
        let tx_id: TransactionId = 1;

        assert!(mgr.lock(tx_id, "table1", LockMode::Exclusive).is_ok());
        assert!(mgr.lock(tx_id, "table2", LockMode::Exclusive).is_ok());
    }
}

mod lock_manager_tests {
    use super::*;

    #[test]
    fn test_lock_manager_new() {
        let mgr = LockManager::new();
        let tx_id = mgr.begin();
        assert_eq!(tx_id, 1);
    }

    #[test]
    fn test_lock_manager_begin_commit() {
        let mgr = LockManager::new();
        let tx_id = mgr.begin();

        let result = mgr.commit(tx_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lock_manager_begin_abort() {
        let mgr = LockManager::new();
        let tx_id = mgr.begin();

        let result = mgr.abort(tx_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lock_manager_lock_row() {
        let mgr = LockManager::new();
        let tx_id = mgr.begin();

        let result = mgr.lock_row(tx_id, "test", 1, 0, LockMode::Exclusive);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lock_manager_lock_table() {
        let mgr = LockManager::new();
        let tx_id = mgr.begin();

        let result = mgr.lock_table(tx_id, "test_table", LockMode::Exclusive);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lock_manager_unlock_row() {
        let mgr = LockManager::new();
        let tx_id = mgr.begin();

        mgr.lock_row(tx_id, "test", 1, 0, LockMode::Exclusive)
            .unwrap();
        mgr.unlock_row(tx_id, "test", 1, 0);
    }

    #[test]
    fn test_lock_manager_unlock_table() {
        let mgr = LockManager::new();
        let tx_id = mgr.begin();

        mgr.lock_table(tx_id, "test_table", LockMode::Exclusive)
            .unwrap();
        mgr.unlock_table(tx_id, "test_table");
    }

    #[test]
    fn test_lock_manager_get_max_committed_tx() {
        let mgr = LockManager::new();

        let tx1 = mgr.begin();
        let tx2 = mgr.begin();

        mgr.commit(tx1).unwrap();
        assert_eq!(mgr.get_max_committed_tx(), tx1);

        mgr.commit(tx2).unwrap();
        assert_eq!(mgr.get_max_committed_tx(), tx2);
    }

    #[test]
    fn test_lock_manager_get_active_txns() {
        let mgr = LockManager::new();

        let tx1 = mgr.begin();
        let tx2 = mgr.begin();

        let active = mgr.get_active_txns();
        assert!(active.contains(&tx1));
        assert!(active.contains(&tx2));

        mgr.commit(tx1).unwrap();
        let active = mgr.get_active_txns();
        assert!(!active.contains(&tx1));
    }

    #[test]
    fn test_lock_manager_undo_ptr() {
        let mgr = LockManager::new();
        let tx_id = mgr.begin();

        let ptr = crate::types::UndoPtr {
            page_id: 1,
            offset: 100,
            lsn: 0,
        };
        mgr.set_last_undo_ptr(tx_id, ptr);

        let result = mgr.get_last_undo_ptr(tx_id);
        assert!(!result.is_null());
    }
}

mod concurrency_tests {
    use super::*;

    #[test]
    fn test_concurrent_transaction_begin() {
        let mgr = Arc::new(TransactionManager::new());

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let mgr = mgr.clone();
                thread::spawn(move || mgr.begin())
            })
            .collect();

        let tx_ids: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        assert_eq!(tx_ids.len(), 10);
    }

    #[test]
    fn test_concurrent_row_locks() {
        let mgr = Arc::new(RowLockManager::new());

        let handles: Vec<_> = (0..5)
            .map(|i| {
                let mgr = mgr.clone();
                let row_id = LockRowId::new("test".to_string(), 1, i);
                thread::spawn(move || mgr.lock(i as TransactionId, row_id, LockMode::Shared))
            })
            .collect();

        for handle in handles {
            assert!(handle.join().unwrap().is_ok());
        }
    }

    #[test]
    fn test_concurrent_commit_order() {
        let mgr = TransactionManager::new();

        let tx1 = mgr.begin();
        let tx2 = mgr.begin();
        let tx3 = mgr.begin();

        mgr.commit(tx2).unwrap();
        mgr.commit(tx1).unwrap();
        mgr.commit(tx3).unwrap();

        assert_eq!(mgr.get_max_committed_tx(), tx3);
    }
}
