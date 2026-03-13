#![deny(unused_variables)]
#![deny(unused_mut)]
#![deny(unused_imports)]
#![deny(unused_assignments)]

//! Aistore storage engine library

// Re-export storage engine API
pub use heap::{HeapError, RowId, Tuple, Value};
pub use storage::{Filter, StorageEngine, StorageError, StorageResult, TableId};

// Public modules for external usage (bench, tools, etc.)
pub mod heap;
pub mod storage;
pub mod table;
pub mod types;

// Internal modules (pub(crate) for crate-internal visibility)
pub(crate) mod buffer;
pub(crate) mod catalog;
pub(crate) mod controlfile;
pub mod index;
pub mod infrastructure;
pub(crate) mod lock;
pub(crate) mod logger;
pub(crate) mod mvcc;
pub(crate) mod page;
pub(crate) mod segment;
pub(crate) mod sql;
pub(crate) mod tablespace;
pub(crate) mod undo;
pub(crate) mod vfs;
pub(crate) mod wal;
