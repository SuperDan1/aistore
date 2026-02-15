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
pub mod table;
pub mod tablespace;
pub mod vfs;

// Re-export page items for easier access
pub use page::Page;

// Re-export vfs items for easier access
pub use vfs::VfsError;
pub use vfs::VfsInterface;
