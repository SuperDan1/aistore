//! Storage Engine API
//!
//! Provides a simple table-oriented storage API for benchmarks and applications.

use crate::buffer::BufferMgr;
use crate::catalog::Catalog;
use crate::heap::{HeapTable, RowId, Tuple, Value};
use crate::lock::{LockManager, LockMode, TransactionId};
use crate::table::Column;
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
    buffer_mgr: Arc<BufferMgr>,
    tables: HashMap<String, HeapTable>,
    lock_mgr: LockManager,
}

impl StorageEngine {
    /// Create a new storage engine
    pub fn new(data_dir: impl Into<std::path::PathBuf>) -> StorageResult<Self> {
        let data_dir = data_dir.into();
        std::fs::create_dir_all(&data_dir).map_err(|e| StorageError::Other(e.to_string()))?;

        let catalog = Catalog::new(&data_dir).map_err(|e| StorageError::Other(e.to_string()))?;

        let buffer_mgr = Arc::new(BufferMgr::init(
            10000,
            Arc::new(crate::vfs::LocalFs::new()),
            data_dir.clone(),
        ));

        let lock_mgr = LockManager::new();

        Ok(Self {
            catalog: Arc::new(catalog),
            buffer_mgr,
            tables: HashMap::new(),
            lock_mgr,
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

    /// Insert a row
    pub fn insert(&mut self, table: &str, values: Vec<Value>) -> StorageResult<RowId> {
        let heap_table = self
            .tables
            .get_mut(table)
            .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

        heap_table
            .insert(&values)
            .map_err(|e| StorageError::Other(e.to_string()))
    }

    /// Scan rows from a table with optional filter
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

    /// Update a row
    pub fn update(&mut self, table: &str, row_id: RowId, values: Vec<Value>) -> StorageResult<()> {
        let heap_table = self
            .tables
            .get_mut(table)
            .ok_or_else(|| StorageError::TableNotFound(table.to_string()))?;

        heap_table
            .update(row_id, &values)
            .map_err(|e| StorageError::Other(e.to_string()))
    }

    /// Delete a row
    pub fn delete(&mut self, table: &str, row_id: RowId) -> StorageResult<()> {
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
        self.lock_mgr.begin()
    }

    /// Commit a transaction
    pub fn commit(&mut self, tx_id: TransactionId) -> StorageResult<()> {
        self.lock_mgr.commit(tx_id).map_err(|e| match e {
            crate::lock::LockError::Timeout => StorageError::LockTimeout,
            crate::lock::LockError::Deadlock => StorageError::Deadlock,
            _ => StorageError::Other(e.to_string()),
        })
    }

    /// Abort a transaction
    pub fn abort(&mut self, tx_id: TransactionId) -> StorageResult<()> {
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
}
