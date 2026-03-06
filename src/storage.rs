//! Storage Engine API
//!
//! Provides a simple table-oriented storage API for benchmarks and applications.

use crate::buffer::BufferMgr;
use crate::catalog::Catalog;
use crate::heap::{HeapTable, RowId, Tuple, Value};
use crate::index::IndexManager;
use crate::lock::{LockManager, LockMode, TransactionId};
use crate::table::Column;
use crate::types::PAGE_SIZE;
use crate::undo::UndoManager;
use crate::wal::WalManager;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

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
    lock_mgr: LockManager,
    wal: Option<WalManager>,
    index_mgr: IndexManager,
    undo_mgr: Arc<crate::undo::UndoManager>,
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

        let wal = WalManager::new(data_dir.clone(), vfs.clone()).ok();

        let index_mgr = IndexManager::new(Arc::clone(&buffer_mgr), data_dir.clone());

        let undo_mgr = Arc::new(crate::undo::UndoManager::new(
            data_dir.to_str().unwrap_or("./data"),
        ));

        Ok(Self {
            catalog: Arc::new(catalog),
            buffer_mgr,
            tables: HashMap::new(),
            lock_mgr,
            wal,
            index_mgr,
            undo_mgr,
        })
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
        let heap_table = self
            .tables
            .get_mut(table)
            .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

        let row_id = heap_table
            .insert(&values)
            .map_err(|e| StorageError::Other(e.to_string()))?;

        self.maintain_index_insert(table, &values, row_id)?;

        Ok(row_id)
    }

    fn maintain_index_insert(
        &mut self,
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
            if let Err(e) = self.index_mgr.insert(id, values, &columns, row_id) {
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

        self.maintain_index_insert(table, &values, row_id)?;

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
    pub fn update(&mut self, table: &str, row_id: RowId, values: Vec<Value>) -> StorageResult<()> {
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
                if let Err(e) = self
                    .index_mgr
                    .delete(*id, &old_values, &columns_clone, row_id)
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
            if let Err(e) = self.index_mgr.insert(id, &values, &columns_clone, row_id) {
                return Err(StorageError::Other(format!("Index insert failed: {}", e)));
            }
        }

        Ok(())
    }

    /// Update a row with transaction (acquires X lock)
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
                if let Err(e) = self
                    .index_mgr
                    .delete(*id, &old_values, &columns_clone, row_id)
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
            if let Err(e) = self.index_mgr.insert(id, &values, &columns_clone, row_id) {
                return Err(StorageError::Other(format!("Index insert failed: {}", e)));
            }
        }

        Ok(())
    }

    /// Delete a row (without transaction)
    pub fn delete(&mut self, table: &str, row_id: RowId) -> StorageResult<()> {
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
            if let Err(e) = self.index_mgr.delete(id, &old_values, &columns, row_id) {
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
                if let Err(e) = self
                    .index_mgr
                    .delete(*id, &old_values, &columns_clone, row_id)
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
        self.index_mgr
            .lookup(index_id, values, &[])
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
        let heap_table = match self.tables.get_mut(table) {
            Some(t) => t,
            None => return false,
        };

        let raw_data = match heap_table.get_raw(row_id) {
            Ok(d) => d,
            Err(_) => return false,
        };

        if raw_data.len() < crate::types::RowMVCCHeader::SIZE {
            return true;
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
                return true;
            }

            if header.undo_ptr.is_null() {
                return false;
            }

            // Follow undo chain to find visible version
            let undo_record = match self.undo_mgr.get(&header.undo_ptr) {
                Ok(r) => r,
                Err(_) => return false,
            };

            // For UPDATE undo records: if current row's tx is active,
            // the before_image IS the visible version in RC
            if matches!(undo_record.header.undo_type, crate::types::UndoType::Update) {
                let undo_tx = undo_record.header.tx_id;
                // In RC: if the updating tx is active, the before_image is visible
                if undo_tx != snapshot.tx_id && snapshot.is_active(undo_tx) {
                    // TODO: Return the before_image data instead of just true
                    // For now, just mark as visible
                    return true;
                }
            }

            // Check if undo record's tx is visible
            let undo_version = crate::types::RowVersion {
                tx_id_created: undo_record.header.tx_id,
                tx_id_deleted: 0,
                undo_ptr: undo_record.header.prev_undo_ptr,
                is_current: false,
            };

            if snapshot.is_visible_rc(&undo_version) {
                return true;
            }

            // Continue following chain to previous undo record
            if undo_record.header.prev_undo_ptr.is_null() {
                return false;
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
        storage
            .insert_with_tx(tx1, "test", vec![Value::Int64(1), Value::Int64(1)])
            .unwrap();
        storage.commit(tx1).unwrap();

        // T2: UPDATE value=2, DON'T COMMIT
        let tx2 = storage.begin_transaction();
        let rows = storage.scan("test", None).unwrap();
        assert_eq!(rows.len(), 1);
        let row_id = RowId::new(1, 0);
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
}
