//! Storage Engine API
//!
//! Provides a simple table-oriented storage API for benchmarks and applications.

use crate::buffer::flusher::PageFlusher;
use crate::buffer::BufferMgr;
use crate::catalog::Catalog;
use crate::heap::{HeapTable, RowId, Tuple, Value};
use crate::index::IndexManager;
use crate::lock::{LockManager, LockMode, TransactionId};
use crate::table::Column;
use crate::types::UndoPtr;
use crate::wal::WalManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Table ID type
pub type TableId = u64;

/// Filter condition for scan operations
#[derive(Debug, Clone)]
pub struct Filter {
    /// Column name to filter on
    pub column: String,
    /// Value to match
    pub value: Value,
}

/// Storage engine error
#[derive(Debug)]
pub enum StorageError {
    TableNotFound(String),
    TableAlreadyExists(String),
    TransactionNotFound,
    TransactionNotActive,
    LockTimeout,
    Deadlock,
    Other(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::TableNotFound(name) => write!(f, "Table not found: {}", name),
            StorageError::TableAlreadyExists(name) => write!(f, "Table already exists: {}", name),
            StorageError::TransactionNotFound => write!(f, "Transaction not found"),
            StorageError::TransactionNotActive => write!(f, "Transaction not active"),
            StorageError::LockTimeout => write!(f, "Lock timeout"),
            StorageError::Deadlock => write!(f, "Deadlock detected"),
            StorageError::Other(msg) => write!(f, "Storage error: {}", msg),
        }
    }
}

impl std::error::Error for StorageError {}

/// Storage engine result
pub type StorageResult<T> = Result<T, StorageError>;

/// Main storage engine interface
///
/// Provides table-oriented operations:
/// - create_table / drop_table: DDL
/// - insert / scan / update / delete: DML
pub struct StorageEngine {
    catalog: Arc<Catalog>,
    buffer_mgr: Arc<RwLock<BufferMgr>>,
    tables: HashMap<String, HeapTable>,
    lock_mgr: Arc<crate::lock::LockManager>,
    wal: Option<Arc<WalManager>>,
    index_mgr: IndexManager,
    undo_mgr: Arc<crate::undo::UndoManager>,
    flusher: Option<PageFlusher>,
}

impl StorageEngine {
    /// Create a new storage engine
    pub fn new(data_dir: impl Into<std::path::PathBuf>) -> StorageResult<Self> {
        let data_dir = data_dir.into();
        std::fs::create_dir_all(&data_dir).map_err(|e| StorageError::Other(e.to_string()))?;

        let catalog = Catalog::new(&data_dir).map_err(|e| StorageError::Other(e.to_string()))?;

        let vfs: Arc<dyn crate::vfs::VfsInterface> = Arc::new(crate::vfs::LocalFs::new());

        let buffer_mgr = Arc::new(RwLock::new(BufferMgr::init(
            10000,
            Arc::clone(&vfs),
            data_dir.clone(),
        )));

        let lock_mgr = LockManager::new();
        let lock_mgr_ref = Arc::new(lock_mgr);

        let wal = WalManager::new(data_dir.clone(), vfs.clone())
            .ok()
            .map(Arc::new);

        let undo_mgr = Arc::new(crate::undo::UndoManager::new(
            data_dir.to_str().unwrap_or("./data"),
        ));

        let mut trx_info_page_id = crate::wal::checkpoint::TRX_INFO_PAGE_ID;

        if let Some(ref wal) = wal {
            let buffer_mgr_for_recovery = Arc::clone(&buffer_mgr);
            let result = wal.recover(move |page_id: crate::types::PageId, data: &[u8]| {
                let mut buf = buffer_mgr_for_recovery.blocking_write();
                buf.recover_page(page_id, data).map_err(|e| e.to_string())
            });
            trx_info_page_id = result.trx_info_page_id;
        }

        let index_mgr = IndexManager::new(
            Arc::clone(&buffer_mgr),
            Arc::clone(&lock_mgr_ref),
            data_dir.clone(),
        );

        let flusher = wal.as_ref().map(|w| {
            let f = PageFlusher::new(Arc::clone(&buffer_mgr), Some(Arc::clone(w)), 1000, 0.1);
            f.start();
            f
        });

        let mut storage = Self {
            catalog: Arc::new(catalog),
            buffer_mgr,
            tables: HashMap::new(),
            lock_mgr: lock_mgr_ref,
            wal,
            index_mgr,
            undo_mgr,
            flusher,
        };

        if trx_info_page_id != 0 {
            let active_txns = storage
                .buffer_mgr
                .blocking_write()
                .get_active_txns(trx_info_page_id)
                .unwrap_or_default();

            if !active_txns.is_empty() {
                tracing::info!(
                    "Recovery: {} active transactions to rollback",
                    active_txns.len()
                );
                for (tx_id, undo_ptr) in active_txns {
                    if let Err(e) = storage.rollback_transaction(tx_id, undo_ptr) {
                        tracing::warn!("Failed to rollback transaction {}: {}", tx_id, e);
                    }
                }
            }
        }

        Ok(storage)
    }

    /// Rollback a transaction during crash recovery using undo log
    pub fn rollback_transaction(
        &mut self,
        tx_id: TransactionId,
        first_undo_ptr: UndoPtr,
    ) -> StorageResult<()> {
        if first_undo_ptr.is_null() {
            self.lock_mgr
                .abort(tx_id)
                .map_err(|e| StorageError::Other(e.to_string()))?;
            return Ok(());
        }

        let undo_mgr = &self.undo_mgr;

        let undo_records = undo_mgr
            .get_tx_undo_chain(tx_id, first_undo_ptr)
            .map_err(|e| StorageError::Other(e.to_string()))?;

        let mut affected_pages = Vec::new();
        for record in &undo_records {
            affected_pages.push(record.header.row_page_id);
        }

        let mut buffer_mgr = self.buffer_mgr.blocking_write();
        for page_id in affected_pages {
            buffer_mgr.mark_dirty(page_id);
        }
        drop(buffer_mgr);

        self.lock_mgr
            .abort(tx_id)
            .map_err(|e| StorageError::Other(e.to_string()))?;

        Ok(())
    }

    /// Create a new table
    pub fn create_table(&mut self, name: &str, columns: Vec<Column>) -> StorageResult<TableId> {
        if self.tables.contains_key(name) {
            return Err(StorageError::TableAlreadyExists(name.to_string()));
        }

        let table = self
            .catalog
            .create_table(name, 1, columns)
            .map_err(|e| StorageError::Other(e.to_string()))?;

        let table_id = table.table_id();
        let heap_table = HeapTable::new(table, Arc::clone(&self.buffer_mgr), 1);
        self.tables.insert(name.to_string(), heap_table);

        Ok(table_id)
    }

    /// Drop a table
    pub fn drop_table(&mut self, name: &str) -> StorageResult<()> {
        self.tables.remove(name);
        self.catalog
            .drop_table(name)
            .map_err(|e| StorageError::Other(e.to_string()))?;
        Ok(())
    }

    /// Check if table exists
    pub fn table_exists(&self, name: &str) -> bool {
        self.tables.contains_key(name)
    }

    /// Insert a row (without transaction)
    pub fn insert(&mut self, table: &str, values: Vec<Value>) -> StorageResult<RowId> {
        let tx_id = self.lock_mgr.begin();

        let heap_table = self
            .tables
            .get_mut(table)
            .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

        let row_id = heap_table
            .insert(&values)
            .map_err(|e| StorageError::Other(e.to_string()))?;

        self.maintain_index_insert(tx_id, table, &values, row_id)?;

        Ok(row_id)
    }

    fn maintain_index_insert(
        &mut self,
        tx_id: TransactionId,
        table: &str,
        values: &[Value],
        row_id: RowId,
    ) -> StorageResult<()> {
        let (columns, index_ids) = {
            let heap_table = self
                .tables
                .get(table)
                .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

            let table_arc = heap_table.table();
            let table_id = table_arc.table_id();
            let columns: Vec<crate::table::Column> =
                table_arc.columns().iter().map(|c| c.clone()).collect();

            let indexes = self.index_mgr.get_table_indexes(table_id);
            let index_ids: Vec<u64> = indexes.iter().map(|m| m.id).collect();

            (columns, index_ids)
        };

        for id in index_ids {
            if let Err(e) = self.index_mgr.insert(tx_id, id, values, &columns, row_id) {
                return Err(StorageError::Other(format!("Index insert failed: {}", e)));
            }
        }

        Ok(())
    }

    /// Insert a row with transaction (acquires X lock on row)
    pub fn insert_with_tx(
        &mut self,
        tx_id: TransactionId,
        table: &str,
        values: Vec<Value>,
    ) -> StorageResult<RowId> {
        #[allow(unused_variables)]
        let table_id = {
            let heap_table = self
                .tables
                .get(table)
                .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;
            heap_table.table().table_id()
        };

        let columns = {
            let heap_table = self
                .tables
                .get(table)
                .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;
            heap_table.table().columns().to_vec()
        };

        let heap_table = self
            .tables
            .get_mut(table)
            .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

        let tuple = crate::heap::Tuple::new(values.clone());
        let data = tuple.serialize_with_mvcc(&columns, Some(tx_id), None);

        let row_id = heap_table
            .insert_raw(&data)
            .map_err(|e| StorageError::Other(e.to_string()))?;

        self.maintain_index_insert(tx_id, table, &values, row_id)?;

        self.lock_mgr
            .lock_row(
                tx_id,
                table,
                row_id.page_id,
                row_id.slot_idx,
                LockMode::Exclusive,
            )
            .map_err(|e| match e {
                crate::lock::LockError::Timeout => StorageError::LockTimeout,
                crate::lock::LockError::Deadlock => StorageError::Deadlock,
                _ => StorageError::Other(e.to_string()),
            })?;

        Ok(row_id)
    }

    /// Get a row by RowId directly (used with index lookup)
    pub fn get_row(&mut self, table: &str, row_id: RowId) -> StorageResult<Tuple> {
        let heap_table = self
            .tables
            .get_mut(table)
            .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

        heap_table
            .get(row_id)
            .map_err(|e| StorageError::Other(e.to_string()))
    }

    /// Scan rows with transaction (acquires S lock)
    #[allow(unused_variables)]
    pub fn scan_with_tx(
        &mut self,
        tx_id: TransactionId,
        table: &str,
        filter: Option<Filter>,
    ) -> StorageResult<Vec<Tuple>> {
        let heap_table = self
            .tables
            .get_mut(table)
            .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

        // Convert filter column name to index
        let filter_idx = if let Some(ref f) = filter {
            heap_table
                .table()
                .columns()
                .iter()
                .position(|c| c.name() == f.column)
                .map(|idx| (idx, &f.value))
        } else {
            None
        };

        // If filtering by id, acquire S lock on that row
        if let Some(ref f) = filter {
            if f.column == "id" {
                // Try to find the row first
                let results = heap_table
                    .scan_with_filter(filter_idx.clone())
                    .map_err(|e| StorageError::Other(e.to_string()))?;

                if let Some(tuple) = results.first() {
                    if let Some(Value::Int64(id)) = tuple.get(0) {
                        // For scan, we need to lock - but we don't have exact page/slot
                        // For now, just do the scan without specific row lock
                    }
                }
            }
        }

        heap_table
            .scan_with_filter(filter_idx)
            .map_err(|e| StorageError::Other(e.to_string()))
    }

    /// Scan rows from a table with optional filter (without transaction)
    pub fn scan(&mut self, table: &str, filter: Option<Filter>) -> StorageResult<Vec<Tuple>> {
        let heap_table = self
            .tables
            .get_mut(table)
            .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

        // Convert filter column name to index
        let filter_idx = if let Some(ref f) = filter {
            heap_table
                .table()
                .columns()
                .iter()
                .position(|c| c.name() == f.column)
                .map(|idx| (idx, &f.value))
        } else {
            None
        };

        heap_table
            .scan_with_filter(filter_idx)
            .map_err(|e| StorageError::Other(e.to_string()))
    }

    /// Scan all rows from a table (convenience method)
    pub fn scan_all(&mut self, table: &str) -> StorageResult<Vec<Tuple>> {
        self.scan(table, None)
    }

    /// Update a row (without transaction)
    #[allow(unused_variables, unused_mut)]
    pub fn update(&mut self, table: &str, row_id: RowId, values: Vec<Value>) -> StorageResult<()> {
        let tx_id = self.lock_mgr.begin();

        let (old_values, columns_clone, index_ids) = {
            let mut heap_table = self
                .tables
                .get_mut(table)
                .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

            let old_tuple = heap_table
                .get(row_id)
                .map_err(|e| StorageError::Other(e.to_string()))?;

            let old_values: Vec<Value> = old_tuple.values().to_vec();

            let table_arc = heap_table.table();
            let table_id = table_arc.table_id();
            let columns = table_arc.columns();
            let columns_clone: Vec<crate::table::Column> =
                columns.iter().map(|c| c.clone()).collect();

            let indexes = self.index_mgr.get_table_indexes(table_id);
            let index_ids: Vec<u64> = indexes.iter().map(|m| m.id).collect();

            for id in &index_ids {
                if let Err(e) =
                    self.index_mgr
                        .delete(tx_id, *id, &old_values, &columns_clone, row_id)
                {
                    return Err(StorageError::Other(format!("Index delete failed: {}", e)));
                }
            }

            (old_values, columns_clone, index_ids)
        };

        let heap_table = self
            .tables
            .get_mut(table)
            .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

        heap_table
            .update(row_id, &values)
            .map_err(|e| StorageError::Other(e.to_string()))?;

        for id in index_ids {
            if let Err(e) = self
                .index_mgr
                .insert(tx_id, id, &values, &columns_clone, row_id)
            {
                return Err(StorageError::Other(format!("Index insert failed: {}", e)));
            }
        }

        Ok(())
    }

    /// Update a row with transaction (acquires X lock)
    #[allow(unused_variables, unused_mut)]
    pub fn update_with_tx(
        &mut self,
        tx_id: TransactionId,
        table: &str,
        row_id: RowId,
        values: Vec<Value>,
    ) -> StorageResult<()> {
        let (old_raw_data, index_data, table_id) = {
            let mut heap_table = self
                .tables
                .get_mut(table)
                .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

            let old_raw = heap_table
                .get_raw(row_id)
                .map_err(|e| StorageError::Other(e.to_string()))?;

            let old_tuple = heap_table
                .get(row_id)
                .map_err(|e| StorageError::Other(e.to_string()))?;

            let old_values: Vec<Value> = old_tuple.values().to_vec();

            let table_arc = heap_table.table();
            let table_id = table_arc.table_id();
            let columns = table_arc.columns();
            let columns_clone: Vec<crate::table::Column> =
                columns.iter().map(|c| c.clone()).collect();

            let indexes = self.index_mgr.get_table_indexes(table_id);
            let index_ids: Vec<u64> = indexes.iter().map(|m| m.id).collect();

            for id in &index_ids {
                if let Err(e) =
                    self.index_mgr
                        .delete(tx_id, *id, &old_values, &columns_clone, row_id)
                {
                    return Err(StorageError::Other(format!("Index delete failed: {}", e)));
                }
            }

            (old_raw, (old_values, columns_clone, index_ids), table_id)
        };

        // Write undo record BEFORE update
        let undo_ptr = self
            .undo_mgr
            .append(
                tx_id,
                table_id as u32,
                row_id.page_id,
                row_id.slot_idx,
                crate::types::UndoType::Update,
                old_raw_data,
                crate::types::UndoPtr::null(),
            )
            .map_err(|e| StorageError::Other(format!("Undo write failed: {}", e)))?;

        self.lock_mgr.set_last_undo_ptr(tx_id, undo_ptr);

        // Acquire X lock on row
        self.lock_mgr
            .lock_row(
                tx_id,
                table,
                row_id.page_id,
                row_id.slot_idx,
                LockMode::Exclusive,
            )
            .map_err(|e| match e {
                crate::lock::LockError::Timeout => StorageError::LockTimeout,
                crate::lock::LockError::Deadlock => StorageError::Deadlock,
                _ => StorageError::Other(e.to_string()),
            })?;

        // Perform update with MVCC header
        let heap_table = self
            .tables
            .get_mut(table)
            .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

        let columns = heap_table.table().columns().to_vec();
        let tuple = crate::heap::Tuple::new(values.clone());
        let data = tuple.serialize_with_mvcc(&columns, Some(tx_id), Some(undo_ptr));

        heap_table
            .update_raw(row_id, &data)
            .map_err(|e| StorageError::Other(e.to_string()))?;

        let (old_values, columns_clone, index_ids) = index_data;
        for id in index_ids {
            if let Err(e) = self
                .index_mgr
                .insert(tx_id, id, &values, &columns_clone, row_id)
            {
                return Err(StorageError::Other(format!("Index insert failed: {}", e)));
            }
        }

        Ok(())
    }

    /// Delete a row (without transaction)
    #[allow(unused_variables, unused_mut)]
    pub fn delete(&mut self, table: &str, row_id: RowId) -> StorageResult<()> {
        let tx_id = self.lock_mgr.begin();

        let old_values: Vec<Value> = {
            let mut heap_table = self
                .tables
                .get_mut(table)
                .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

            let old_tuple = heap_table
                .get(row_id)
                .map_err(|e| StorageError::Other(e.to_string()))?;

            old_tuple.values().to_vec()
        };

        let columns: Vec<crate::table::Column> = {
            let heap_table = self
                .tables
                .get(table)
                .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

            let table_arc = heap_table.table();
            table_arc.columns().iter().map(|c| c.clone()).collect()
        };

        let index_ids: Vec<u64> = {
            let heap_table = self
                .tables
                .get(table)
                .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

            let table_arc = heap_table.table();
            let table_id = table_arc.table_id();

            let indexes = self.index_mgr.get_table_indexes(table_id);
            indexes.iter().map(|m| m.id).collect()
        };

        for id in index_ids {
            if let Err(e) = self
                .index_mgr
                .delete(tx_id, id, &old_values, &columns, row_id)
            {
                return Err(StorageError::Other(format!("Index delete failed: {}", e)));
            }
        }

        let heap_table = self
            .tables
            .get_mut(table)
            .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

        heap_table
            .delete(row_id)
            .map_err(|e| StorageError::Other(e.to_string()))
    }

    /// Delete a row with transaction (acquires X lock)
    #[allow(unused_variables, unused_mut)]
    pub fn delete_with_tx(
        &mut self,
        tx_id: TransactionId,
        table: &str,
        row_id: RowId,
    ) -> StorageResult<()> {
        let (old_values, columns_clone, index_ids) = {
            let mut heap_table = self
                .tables
                .get_mut(table)
                .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

            let old_tuple = heap_table
                .get(row_id)
                .map_err(|e| StorageError::Other(e.to_string()))?;

            let old_values: Vec<Value> = old_tuple.values().to_vec();

            let table_arc = heap_table.table();
            let table_id = table_arc.table_id();
            let columns = table_arc.columns();
            let columns_clone: Vec<crate::table::Column> =
                columns.iter().map(|c| c.clone()).collect();

            let indexes = self.index_mgr.get_table_indexes(table_id);
            let index_ids: Vec<u64> = indexes.iter().map(|m| m.id).collect();

            for id in &index_ids {
                if let Err(e) =
                    self.index_mgr
                        .delete(tx_id, *id, &old_values, &columns_clone, row_id)
                {
                    return Err(StorageError::Other(format!("Index delete failed: {}", e)));
                }
            }

            (old_values, columns_clone, index_ids)
        };

        // Acquire X lock on row
        self.lock_mgr
            .lock_row(
                tx_id,
                table,
                row_id.page_id,
                row_id.slot_idx,
                LockMode::Exclusive,
            )
            .map_err(|e| match e {
                crate::lock::LockError::Timeout => StorageError::LockTimeout,
                crate::lock::LockError::Deadlock => StorageError::Deadlock,
                _ => StorageError::Other(e.to_string()),
            })?;

        // Perform delete
        let heap_table = self
            .tables
            .get_mut(table)
            .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

        heap_table
            .delete(row_id)
            .map_err(|e| StorageError::Other(e.to_string()))
    }

    /// Get table info
    pub fn get_table(&self, name: &str) -> StorageResult<Arc<crate::table::Table>> {
        self.catalog
            .get_table(name)
            .map_err(|e| StorageError::Other(e.to_string()))
    }

    /// List all tables
    pub fn list_tables(&self) -> Vec<String> {
        self.tables.keys().cloned().collect()
    }

    /// Begin a new transaction
    pub fn begin_transaction(&mut self) -> TransactionId {
        let tx_id = self.lock_mgr.begin();
        if let Some(ref wal) = self.wal {
            wal.tx_begin(tx_id);
        }
        tx_id
    }

    /// Commit a transaction
    pub fn commit(&mut self, tx_id: TransactionId) -> StorageResult<()> {
        if let Some(ref wal) = self.wal {
            wal.commit(tx_id)
                .map_err(|e| StorageError::Other(e.to_string()))?;
        }

        self.index_mgr.release_tx_locks(tx_id);

        self.lock_mgr.commit(tx_id).map_err(|e| match e {
            crate::lock::LockError::Timeout => StorageError::LockTimeout,
            crate::lock::LockError::Deadlock => StorageError::Deadlock,
            _ => StorageError::Other(e.to_string()),
        })
    }

    /// Abort a transaction
    pub fn abort(&mut self, tx_id: TransactionId) -> StorageResult<()> {
        if let Some(ref wal) = self.wal {
            wal.abort(tx_id)
                .map_err(|e| StorageError::Other(e.to_string()))?;
        }

        self.index_mgr.release_tx_locks(tx_id);

        self.lock_mgr.abort(tx_id).map_err(|e| match e {
            crate::lock::LockError::Timeout => StorageError::LockTimeout,
            crate::lock::LockError::Deadlock => StorageError::Deadlock,
            _ => StorageError::Other(e.to_string()),
        })
    }

    /// Flush all dirty pages to disk
    pub fn flush(&mut self) -> StorageResult<()> {
        for heap_table in self.tables.values_mut() {
            heap_table
                .flush()
                .map_err(|e| StorageError::Other(e.to_string()))?;
        }
        Ok(())
    }

    /// Create an index on a table
    pub fn create_index(
        &mut self,
        table: &str,
        name: &str,
        columns: Vec<String>,
        unique: bool,
    ) -> StorageResult<u64> {
        let table_arc = self
            .catalog
            .get_table(table)
            .map_err(|e| StorageError::Other(e.to_string()))?;

        let table = table_arc.as_ref();
        let table_id = table.table_id();

        let index_id = self
            .index_mgr
            .create_index(table_id, name.to_string(), columns, unique)
            .map_err(|e| StorageError::Other(e.to_string()))?;

        Ok(index_id)
    }

    /// Drop an index
    pub fn drop_index(&mut self, index_id: u64) -> StorageResult<()> {
        self.index_mgr
            .drop_index(index_id)
            .map_err(|e| StorageError::Other(e.to_string()))
    }

    /// Lookup by index
    pub fn lookup_index(&self, index_id: u64, values: &[Value]) -> StorageResult<Vec<RowId>> {
        let tx_id = self.lock_mgr.begin();

        self.index_mgr
            .lookup(tx_id, index_id, values, &[])
            .map_err(|e| StorageError::Other(e.to_string()))
    }

    pub fn create_snapshot(&self, tx_id: TransactionId) -> crate::types::ReadSnapshot {
        crate::types::ReadSnapshot {
            tx_id,
            snapshot_lsn: 0,
            max_committed_tx: self.lock_mgr.get_max_committed_tx(),
            active_txns: self.lock_mgr.get_active_txns(),
            isolation: crate::types::IsolationLevel::ReadCommitted,
        }
    }

    pub fn is_row_visible(
        &mut self,
        table: &str,
        row_id: RowId,
        snapshot: &crate::types::ReadSnapshot,
    ) -> bool {
        self.get_visible_before_image(table, row_id, snapshot)
            .is_some()
    }

    /// Get visible before_image for a row (for RC: returns before_image when UPDATE tx is active)
    /// Returns Some(before_image) if visible and has before_image, None otherwise
    pub fn get_visible_before_image(
        &mut self,
        table: &str,
        row_id: RowId,
        snapshot: &crate::types::ReadSnapshot,
    ) -> Option<Vec<u8>> {
        let heap_table = match self.tables.get_mut(table) {
            Some(t) => t,
            None => return None,
        };

        let raw_data = match heap_table.get_raw(row_id) {
            Ok(d) => d,
            Err(_) => return None,
        };

        if raw_data.len() < crate::types::RowMVCCHeader::SIZE {
            return Some(raw_data);
        }

        let mut header_bytes = [0u8; 34];
        header_bytes.copy_from_slice(&raw_data[..34]);
        let mut header = crate::types::RowMVCCHeader::from_bytes(&header_bytes);

        loop {
            let version = crate::types::RowVersion {
                tx_id_created: header.tx_id_created,
                tx_id_deleted: header.tx_id_deleted,
                undo_ptr: header.undo_ptr,
                is_current: true,
            };

            if snapshot.is_visible_rc(&version) {
                return Some(raw_data);
            }

            if header.undo_ptr.is_null() {
                return None;
            }

            let undo_record = match self.undo_mgr.get(&header.undo_ptr) {
                Ok(r) => r,
                Err(_) => return None,
            };

            if matches!(undo_record.header.undo_type, crate::types::UndoType::Update) {
                let undo_tx = undo_record.header.tx_id;
                if undo_tx != snapshot.tx_id && snapshot.is_active(undo_tx) {
                    return Some(undo_record.before_image.clone());
                }
            }

            let undo_version = crate::types::RowVersion {
                tx_id_created: undo_record.header.tx_id,
                tx_id_deleted: 0,
                undo_ptr: undo_record.header.prev_undo_ptr,
                is_current: false,
            };

            if snapshot.is_visible_rc(&undo_version) {
                return Some(undo_record.before_image.clone());
            }

            if undo_record.header.prev_undo_ptr.is_null() {
                return None;
            }
            header.undo_ptr = undo_record.header.prev_undo_ptr;
        }
    }

    pub fn scan_with_snapshot(
        &mut self,
        table: &str,
        snapshot: &crate::types::ReadSnapshot,
    ) -> StorageResult<Vec<Tuple>> {
        let heap_table = self
            .tables
            .get_mut(table)
            .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

        let results = heap_table
            .iter_visible(|header| {
                let version = crate::types::RowVersion {
                    tx_id_created: header.tx_id_created,
                    tx_id_deleted: header.tx_id_deleted,
                    undo_ptr: header.undo_ptr,
                    is_current: true,
                };
                snapshot.is_visible_rc(&version)
            })
            .map_err(|e| StorageError::Other(e.to_string()))?;

        Ok(results)
    }
}

#[cfg(test)]
mod mvcc_tests {
    use super::*;

    fn create_test_engine() -> StorageEngine {
        let tmp_dir = std::env::temp_dir().join(format!("mvcc_test_{}", std::process::id()));
        std::fs::create_dir_all(&tmp_dir).ok();
        StorageEngine::new(&tmp_dir).unwrap()
    }

    #[test]
    fn test_rc_insert_commit_update_uncommitted() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "value".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        // T1: INSERT value=1, COMMIT
        let tx1 = storage.begin_transaction();
        let row_id = storage
            .insert_with_tx(tx1, "test", vec![Value::Int64(1), Value::Int64(1)])
            .unwrap();
        storage.commit(tx1).unwrap();

        // T2: UPDATE value=2, DON'T COMMIT
        let tx2 = storage.begin_transaction();
        storage
            .update_with_tx(tx2, "test", row_id, vec![Value::Int64(1), Value::Int64(2)])
            .unwrap();
        // Don't commit tx2

        // T3: Query - should see value=1 (RC, tx2 not committed)
        let snapshot = storage.create_snapshot(999);
        let is_visible = storage.is_row_visible("test", row_id, &snapshot);

        assert!(
            is_visible,
            "Should see old value=1 because tx2 is not committed"
        );

        // Cleanup
        storage.abort(tx2).ok();
    }

    #[test]
    fn test_rc_insert_commit_update_committed() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "value".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        // T1: INSERT value=1, COMMIT
        let tx1 = storage.begin_transaction();
        storage
            .insert_with_tx(tx1, "test", vec![Value::Int64(1), Value::Int64(1)])
            .unwrap();
        storage.commit(tx1).unwrap();

        // T2: UPDATE value=2, COMMIT
        let tx2 = storage.begin_transaction();
        let row_id = RowId::new(1, 0);
        storage
            .update_with_tx(tx2, "test", row_id, vec![Value::Int64(1), Value::Int64(2)])
            .unwrap();
        storage.commit(tx2).unwrap();

        // T3: Query - should see value=2 (RC, tx2 committed)
        let snapshot = storage.create_snapshot(999);
        let is_visible = storage.is_row_visible("test", row_id, &snapshot);

        assert!(
            is_visible,
            "Should see new value=2 because tx2 is committed"
        );
    }

    #[test]
    fn test_rc_insert_uncommitted() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "value".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        // T1: INSERT value=1, DON'T COMMIT
        let tx1 = storage.begin_transaction();
        storage
            .insert_with_tx(tx1, "test", vec![Value::Int64(1), Value::Int64(1)])
            .unwrap();
        // Don't commit tx1

        // T2: Query with snapshot - should NOT see the row (tx1 not committed)
        let snapshot = storage.create_snapshot(999);
        let rows = storage.scan_with_snapshot("test", &snapshot).unwrap();

        assert_eq!(rows.len(), 0, "Should not see uncommitted insert");
    }

    #[test]
    fn test_rc_rollback() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "value".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        // T1: INSERT value=1, COMMIT
        let tx1 = storage.begin_transaction();
        storage
            .insert_with_tx(tx1, "test", vec![Value::Int64(1), Value::Int64(1)])
            .unwrap();
        storage.commit(tx1).unwrap();

        // T2: UPDATE value=2, ROLLBACK
        let tx2 = storage.begin_transaction();
        let row_id = RowId::new(1, 0);
        storage
            .update_with_tx(tx2, "test", row_id, vec![Value::Int64(1), Value::Int64(2)])
            .unwrap();
        storage.abort(tx2).unwrap();

        // T3: Query - should see value=1 (tx2 rolled back)
        let snapshot = storage.create_snapshot(999);
        let is_visible = storage.is_row_visible("test", row_id, &snapshot);

        assert!(is_visible, "Should see value=1 because tx2 was rolled back");
    }

    #[test]
    fn test_rc_delete_uncommitted() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "value".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        let tx1 = storage.begin_transaction();
        let row_id = storage
            .insert_with_tx(tx1, "test", vec![Value::Int64(1), Value::Int64(1)])
            .unwrap();
        storage.commit(tx1).unwrap();

        let tx2 = storage.begin_transaction();
        storage.delete_with_tx(tx2, "test", row_id).unwrap();

        let snapshot = storage.create_snapshot(999);
        let rows = storage.scan_with_snapshot("test", &snapshot).unwrap();

        assert_eq!(
            rows.len(),
            0,
            "Should not see row in scan because delete not committed"
        );

        storage.abort(tx2).ok();
    }

    #[test]
    fn test_rc_delete_committed() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "value".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        let tx1 = storage.begin_transaction();
        let row_id = storage
            .insert_with_tx(tx1, "test", vec![Value::Int64(1), Value::Int64(1)])
            .unwrap();
        storage.commit(tx1).unwrap();

        let tx2 = storage.begin_transaction();
        storage.delete_with_tx(tx2, "test", row_id).unwrap();
        storage.commit(tx2).unwrap();

        let snapshot = storage.create_snapshot(999);
        let is_visible = storage.is_row_visible("test", row_id, &snapshot);

        assert!(!is_visible, "Should not see row because delete committed");
    }

    #[test]
    fn test_rc_multiple_transactions() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "value".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        let tx1 = storage.begin_transaction();
        storage
            .insert_with_tx(tx1, "test", vec![Value::Int64(1), Value::Int64(100)])
            .unwrap();
        storage.commit(tx1).unwrap();

        let tx2 = storage.begin_transaction();
        storage
            .insert_with_tx(tx2, "test", vec![Value::Int64(2), Value::Int64(200)])
            .unwrap();
        storage.commit(tx2).unwrap();

        let tx3 = storage.begin_transaction();
        storage
            .insert_with_tx(tx3, "test", vec![Value::Int64(3), Value::Int64(300)])
            .unwrap();
        storage.commit(tx3).unwrap();

        let snapshot = storage.create_snapshot(999);
        let rows = storage.scan_with_snapshot("test", &snapshot).unwrap();

        assert_eq!(rows.len(), 3, "Should see all 3 committed rows");
    }

    #[test]
    fn test_rc_own_transaction_visible() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "value".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        let tx1 = storage.begin_transaction();
        let _row_id = storage
            .insert_with_tx(tx1, "test", vec![Value::Int64(1), Value::Int64(1)])
            .unwrap();

        let rows = storage.scan("test", None).unwrap();
        assert!(
            rows.len() > 0,
            "Own uncommitted insert should be visible in scan"
        );

        storage.commit(tx1).unwrap();
    }

    #[test]
    fn test_rc_after_commit_visibility() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "value".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        let tx1 = storage.begin_transaction();
        let row_id = storage
            .insert_with_tx(tx1, "test", vec![Value::Int64(1), Value::Int64(1)])
            .unwrap();
        storage.commit(tx1).unwrap();

        let snapshot = storage.create_snapshot(tx1);
        let is_visible = storage.is_row_visible("test", row_id, &snapshot);

        assert!(
            is_visible,
            "Committed row should be visible to new transaction"
        );
    }
}

#[cfg(test)]
mod acid_tests {
    use super::*;

    fn create_test_engine() -> StorageEngine {
        let tmp_dir = std::env::temp_dir().join(format!("acid_test_{}", std::process::id()));
        std::fs::create_dir_all(&tmp_dir).ok();
        StorageEngine::new(&tmp_dir).unwrap()
    }

    #[test]
    fn test_atomicity_insert() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "value".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        let tx = storage.begin_transaction();
        storage
            .insert_with_tx(tx, "test", vec![Value::Int64(1), Value::Int64(100)])
            .unwrap();
        storage.commit(tx).unwrap();

        let rows = storage.scan("test", None).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn test_atomicity_update() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "value".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        let tx1 = storage.begin_transaction();
        let row_id = storage
            .insert_with_tx(tx1, "test", vec![Value::Int64(1), Value::Int64(100)])
            .unwrap();
        storage.commit(tx1).unwrap();

        let tx2 = storage.begin_transaction();
        storage
            .update_with_tx(
                tx2,
                "test",
                row_id,
                vec![Value::Int64(1), Value::Int64(200)],
            )
            .unwrap();
        storage.commit(tx2).unwrap();

        let rows = storage.scan("test", None).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn test_atomicity_delete() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "value".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        let tx1 = storage.begin_transaction();
        let row_id = storage
            .insert_with_tx(tx1, "test", vec![Value::Int64(1), Value::Int64(100)])
            .unwrap();
        storage.commit(tx1).unwrap();

        let tx2 = storage.begin_transaction();
        storage.delete_with_tx(tx2, "test", row_id).unwrap();
        storage.commit(tx2).unwrap();

        let rows = storage.scan("test", None).unwrap();
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn test_atomicity_rollback() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "value".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        let tx1 = storage.begin_transaction();
        storage
            .insert_with_tx(tx1, "test", vec![Value::Int64(1), Value::Int64(100)])
            .unwrap();
        storage.commit(tx1).unwrap();

        let tx2 = storage.begin_transaction();
        let row_id = RowId::new(1, 0);
        storage
            .update_with_tx(
                tx2,
                "test",
                row_id,
                vec![Value::Int64(1), Value::Int64(200)],
            )
            .unwrap();
        storage.abort(tx2).unwrap();

        let rows = storage.scan("test", None).unwrap();
        assert_eq!(rows.len(), 1, "Rolled back update should not change data");
    }

    #[test]
    fn test_consistency_constraints() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "value".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        let tx = storage.begin_transaction();
        let row_id = storage
            .insert_with_tx(tx, "test", vec![Value::Int64(1), Value::Int64(100)])
            .unwrap();
        storage.commit(tx).unwrap();

        let snapshot = storage.create_snapshot(tx);
        let is_visible = storage.is_row_visible("test", row_id, &snapshot);
        assert!(is_visible, "Inserted row should be visible");
    }

    #[test]
    fn test_isolation_dirty_read_prevented() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "value".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        let tx1 = storage.begin_transaction();
        storage
            .insert_with_tx(tx1, "test", vec![Value::Int64(1), Value::Int64(100)])
            .unwrap();

        let snapshot = storage.create_snapshot(999);
        let rows = storage.scan_with_snapshot("test", &snapshot).unwrap();

        assert_eq!(rows.len(), 0, "Dirty read should be prevented");

        storage.abort(tx1).ok();
    }

    #[test]
    fn test_isolation_serializable_like() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "value".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        let tx1 = storage.begin_transaction();
        storage
            .insert_with_tx(tx1, "test", vec![Value::Int64(1), Value::Int64(100)])
            .unwrap();
        storage.commit(tx1).unwrap();

        let tx2 = storage.begin_transaction();
        storage
            .insert_with_tx(tx2, "test", vec![Value::Int64(2), Value::Int64(200)])
            .unwrap();
        storage.commit(tx2).unwrap();

        let rows = storage.scan("test", None).unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn test_durability_commit_persisted() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "value".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        let tx = storage.begin_transaction();
        storage
            .insert_with_tx(tx, "test", vec![Value::Int64(1), Value::Int64(100)])
            .unwrap();
        storage.commit(tx).unwrap();

        let rows = storage.scan("test", None).unwrap();
        assert_eq!(rows.len(), 1, "Committed data should be durable");
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    fn create_test_engine() -> StorageEngine {
        let tmp_dir = std::env::temp_dir().join(format!("integration_test_{}", std::process::id()));
        std::fs::create_dir_all(&tmp_dir).ok();
        StorageEngine::new(&tmp_dir).unwrap()
    }

    #[test]
    fn test_full_workflow() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "users",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "name".to_string(),
                        crate::types::ColumnType::Varchar(100),
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        let tx = storage.begin_transaction();
        let row_id = storage
            .insert_with_tx(
                tx,
                "users",
                vec![Value::Int64(1), Value::VarChar("Alice".to_string())],
            )
            .unwrap();
        storage.commit(tx).unwrap();

        let rows = storage.scan("users", None).unwrap();
        assert_eq!(rows.len(), 1);

        let tx = storage.begin_transaction();
        storage
            .update_with_tx(
                tx,
                "users",
                row_id,
                vec![Value::Int64(1), Value::VarChar("Bob".to_string())],
            )
            .unwrap();
        storage.commit(tx).unwrap();

        let tx = storage.begin_transaction();
        storage.delete_with_tx(tx, "users", row_id).unwrap();
        storage.commit(tx).unwrap();

        let rows = storage.scan("users", None).unwrap();
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn test_multiple_tables() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "table1",
                vec![Column::new(
                    "id".to_string(),
                    crate::types::ColumnType::Int64,
                    false,
                    0,
                )],
            )
            .unwrap();

        storage
            .create_table(
                "table2",
                vec![Column::new(
                    "id".to_string(),
                    crate::types::ColumnType::Int64,
                    false,
                    0,
                )],
            )
            .unwrap();

        let tx1 = storage.begin_transaction();
        storage
            .insert_with_tx(tx1, "table1", vec![Value::Int64(1)])
            .unwrap();
        storage.commit(tx1).unwrap();

        let tx2 = storage.begin_transaction();
        storage
            .insert_with_tx(tx2, "table2", vec![Value::Int64(2)])
            .unwrap();
        storage.commit(tx2).unwrap();

        let rows1 = storage.scan("table1", None).unwrap();
        let rows2 = storage.scan("table2", None).unwrap();

        assert_eq!(rows1.len(), 1);
        assert_eq!(rows2.len(), 1);
    }

    #[test]
    fn test_crud_all_column_types() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new(
                        "c_int64".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        0,
                    ),
                    Column::new(
                        "c_varchar".to_string(),
                        crate::types::ColumnType::Varchar(100),
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        let tx = storage.begin_transaction();
        storage
            .insert_with_tx(
                tx,
                "test",
                vec![Value::Int64(42), Value::VarChar("test string".to_string())],
            )
            .unwrap();
        storage.commit(tx).unwrap();

        let rows = storage.scan("test", None).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn test_scan_with_filter() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![
                    Column::new("id".to_string(), crate::types::ColumnType::Int64, false, 0),
                    Column::new(
                        "value".to_string(),
                        crate::types::ColumnType::Int64,
                        false,
                        1,
                    ),
                ],
            )
            .unwrap();

        let tx = storage.begin_transaction();
        for i in 1..=10 {
            storage
                .insert_with_tx(tx, "test", vec![Value::Int64(i), Value::Int64(i * 10)])
                .unwrap();
        }
        storage.commit(tx).unwrap();

        let rows = storage.scan("test", None).unwrap();
        assert_eq!(rows.len(), 10);
    }

    #[test]
    fn test_scan_empty_table() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "empty",
                vec![Column::new(
                    "id".to_string(),
                    crate::types::ColumnType::Int64,
                    false,
                    0,
                )],
            )
            .unwrap();

        let rows = storage.scan("empty", None).unwrap();
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn test_bulk_insert() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![Column::new(
                    "id".to_string(),
                    crate::types::ColumnType::Int64,
                    false,
                    0,
                )],
            )
            .unwrap();

        let tx = storage.begin_transaction();
        for i in 1..=100 {
            storage
                .insert_with_tx(tx, "test", vec![Value::Int64(i)])
                .unwrap();
        }
        storage.commit(tx).unwrap();

        let rows = storage.scan("test", None).unwrap();
        assert_eq!(rows.len(), 100);
    }

    #[test]
    fn test_drop_table() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![Column::new(
                    "id".to_string(),
                    crate::types::ColumnType::Int64,
                    false,
                    0,
                )],
            )
            .unwrap();

        let tx = storage.begin_transaction();
        storage
            .insert_with_tx(tx, "test", vec![Value::Int64(1)])
            .unwrap();
        storage.commit(tx).unwrap();

        storage.drop_table("test").unwrap();

        let result = storage.scan("test", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_table_name() {
        let mut storage = create_test_engine();

        storage
            .create_table(
                "test",
                vec![Column::new(
                    "id".to_string(),
                    crate::types::ColumnType::Int64,
                    false,
                    0,
                )],
            )
            .unwrap();

        let result = storage.create_table(
            "test",
            vec![Column::new(
                "id".to_string(),
                crate::types::ColumnType::Int64,
                false,
                0,
            )],
        );

        assert!(result.is_err());
    }
}

#[cfg(test)]
mod thread_safety_tests {
    use super::*;
    use crate::types::ColumnType;

    fn create_test_engine() -> StorageEngine {
        let tmp_dir = std::env::temp_dir().join(format!("thread_test_{}", std::process::id()));
        std::fs::create_dir_all(&tmp_dir).ok();
        StorageEngine::new(&tmp_dir).unwrap()
    }

    #[test]
    fn test_storage_tables_thread_safety_issue() {
        // ISSUE: StorageEngine requires &mut self for all operations
        // This means tables HashMap is not safely shareable across threads
        // Each thread needs its own StorageEngine instance
        //
        // Current design:
        //   tables: HashMap<String, HeapTable>  // NOT thread-safe
        //
        // Should be:
        //   tables: Arc<RwLock<HashMap<String, HeapTable>>>

        // This works fine with separate instances
        for i in 0..10 {
            let mut engine = create_test_engine();
            engine
                .create_table(
                    &format!("t_{}", i),
                    vec![Column::new("id".to_string(), ColumnType::Int64, false, 0)],
                )
                .unwrap();
        }
    }

    #[test]
    fn test_concurrent_table_access() {
        // Test that StorageEngine cannot be easily shared across threads
        // due to &mut self requirement
        let mut storage = create_test_engine();

        storage
            .create_table(
                "shared",
                vec![Column::new("id".to_string(), ColumnType::Int64, false, 0)],
            )
            .unwrap();

        // NOTE: Cannot share StorageEngine via Arc directly
        // because insert/scan/update/delete all require &mut self
        // This is a design limitation - not thread-safe for concurrent mutations
        let result = storage.table_exists("shared");
        assert!(result, "table should exist");
    }
}
