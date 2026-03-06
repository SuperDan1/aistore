pub mod error;

use crate::heap::{HeapTable, RowId, Tuple, Value};
use crate::lock::{LockManager, LockMode, TransactionId};
use crate::types::{
    IsolationLevel, MvccError, MvccResult, ReadSnapshot, RowMVCCHeader, RowVersion,
    TransactionId as TxId, UndoPtr, UndoRecord, UndoType, LSN,
};
use crate::undo::UndoManager;
use std::sync::Arc;

pub struct MvccManager {
    lock_mgr: Arc<LockManager>,
    undo_mgr: Arc<UndoManager>,
}

impl MvccManager {
    pub fn new(lock_mgr: Arc<LockManager>, undo_mgr: Arc<UndoManager>) -> Self {
        Self { lock_mgr, undo_mgr }
    }

    pub fn create_snapshot(&self, tx_id: TransactionId) -> ReadSnapshot {
        let max_committed = self.lock_mgr.get_max_committed_tx();
        let active = self.lock_mgr.get_active_txns();

        ReadSnapshot {
            tx_id,
            snapshot_lsn: 0,
            max_committed_tx: max_committed,
            active_txns: active,
            isolation: IsolationLevel::ReadCommitted,
        }
    }

    pub fn is_visible(&self, snapshot: &ReadSnapshot, version: &RowVersion) -> bool {
        snapshot.is_visible_rc(version)
    }

    pub fn insert(
        &self,
        tx_id: TransactionId,
        table_id: u32,
        heap_table: &mut HeapTable,
        values: &[Value],
        columns: &[crate::table::Column],
    ) -> MvccResult<RowId> {
        self.lock_mgr
            .lock_row(tx_id, "", 0, 0, LockMode::Exclusive)
            .map_err(|_| MvccError::Other("Lock failed".to_string()))?;

        let tuple = Tuple::new(values.to_vec());
        let data = tuple.serialize_with_mvcc(columns, Some(tx_id), None);

        let row_id = heap_table
            .insert_raw(&data)
            .map_err(|e| MvccError::Other(e.to_string()))?;

        let undo_ptr = self
            .undo_mgr
            .append(
                tx_id,
                table_id,
                row_id.page_id,
                row_id.slot_idx,
                UndoType::Insert,
                vec![],
                UndoPtr::null(),
            )
            .map_err(|e| MvccError::Other(e.to_string()))?;

        self.lock_mgr.set_last_undo_ptr(tx_id, undo_ptr);

        Ok(row_id)
    }

    pub fn update(
        &self,
        tx_id: TransactionId,
        table_id: u32,
        heap_table: &mut HeapTable,
        row_id: &RowId,
        values: &[Value],
        columns: &[crate::table::Column],
    ) -> MvccResult<()> {
        self.lock_mgr
            .lock_row(
                tx_id,
                "",
                row_id.page_id,
                row_id.slot_idx,
                LockMode::Exclusive,
            )
            .map_err(|_| MvccError::Other("Lock failed".to_string()))?;

        let old_data = heap_table
            .get_raw(*row_id)
            .map_err(|e| MvccError::Other(e.to_string()))?;

        let undo_ptr = self
            .undo_mgr
            .append(
                tx_id,
                table_id,
                row_id.page_id,
                row_id.slot_idx,
                UndoType::Update,
                old_data.clone(),
                UndoPtr::null(),
            )
            .map_err(|e| MvccError::Other(e.to_string()))?;

        self.lock_mgr.set_last_undo_ptr(tx_id, undo_ptr);

        let tuple = Tuple::new(values.to_vec());
        let new_data = tuple.serialize_with_mvcc(columns, Some(tx_id), Some(undo_ptr));

        heap_table
            .update_raw(*row_id, &new_data)
            .map_err(|e| MvccError::Other(e.to_string()))?;

        Ok(())
    }

    pub fn delete(
        &self,
        tx_id: TransactionId,
        table_id: u32,
        heap_table: &mut HeapTable,
        row_id: &RowId,
        columns: &[crate::table::Column],
    ) -> MvccResult<()> {
        self.lock_mgr
            .lock_row(
                tx_id,
                "",
                row_id.page_id,
                row_id.slot_idx,
                LockMode::Exclusive,
            )
            .map_err(|_| MvccError::Other("Lock failed".to_string()))?;

        let old_data = heap_table
            .get_raw(*row_id)
            .map_err(|e| MvccError::Other(e.to_string()))?;

        let undo_ptr = self
            .undo_mgr
            .append(
                tx_id,
                table_id,
                row_id.page_id,
                row_id.slot_idx,
                UndoType::Delete,
                old_data,
                UndoPtr::null(),
            )
            .map_err(|e| MvccError::Other(e.to_string()))?;

        self.lock_mgr.set_last_undo_ptr(tx_id, undo_ptr);

        heap_table
            .delete(*row_id)
            .map_err(|e| MvccError::Other(e.to_string()))?;

        Ok(())
    }

    pub fn rollback(&self, tx_id: TransactionId) -> MvccResult<()> {
        let undo_ptr = self.lock_mgr.get_last_undo_ptr(tx_id);

        if undo_ptr.is_null() {
            return Ok(());
        }

        let records = self
            .undo_mgr
            .get_tx_undo_chain(tx_id, undo_ptr)
            .map_err(|e| MvccError::Other(e.to_string()))?;

        for record in records {
            let row_id = RowId::new(record.header.row_page_id, record.header.row_slot_idx);

            match record.header.undo_type {
                UndoType::Insert => {
                    // Rollback insert = delete the row
                    // Need heap table reference - this is simplified
                }
                UndoType::Update => {
                    // Rollback update = restore before_image
                    // Need heap table reference - this is simplified
                }
                UndoType::Delete => {
                    // Rollback delete = undelete (restore row)
                    // Need heap table reference - this is simplified
                }
            }
        }

        Ok(())
    }
}
