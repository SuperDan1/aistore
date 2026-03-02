//! Aistore storage engine library

// Global type definitions
pub mod types;

// Import various modules
pub mod buffer;
pub mod catalog;
pub mod controlfile;
pub mod heap;
pub mod index;
pub mod infrastructure;
pub mod lock;
pub mod page;
pub mod segment;
pub mod sql;
pub mod storage; // Storage Engine API
pub mod table;
pub mod tablespace;
pub mod vfs;

// Re-export page items for easier access
pub use page::Page;

// Re-export vfs items for easier access
pub use vfs::VfsError;
pub use vfs::VfsInterface;

// Re-export heap items for easier access
pub use heap::{HeapTable, RowId, Tuple, Value};

// Re-export storage engine API
pub use storage::{StorageEngine, StorageError, StorageResult, TableId};

// Re-export catalog and table items
pub use catalog::Catalog;
pub use table::Column;

// Re-export buffer items
pub use buffer::BufferMgr;
