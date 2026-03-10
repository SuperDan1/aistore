//! Aistore storage engine library

// Re-export storage engine API only
pub use heap::{HeapError, RowId, Tuple, Value};
pub use storage::{Filter, StorageEngine, StorageError, StorageResult, TableId};

// Internal modules (pub(crate) for crate-internal visibility)
pub(crate) mod buffer;
pub(crate) mod catalog;
pub(crate) mod controlfile;
pub(crate) mod heap;
pub(crate) mod index;
pub(crate) mod infrastructure;
pub(crate) mod lock;
pub(crate) mod mvcc;
pub(crate) mod page;
pub(crate) mod segment;
pub(crate) mod sql;
pub(crate) mod storage;
pub(crate) mod table;
pub(crate) mod tablespace;
pub(crate) mod types;
pub(crate) mod undo;
pub(crate) mod vfs;
pub(crate) mod wal;
