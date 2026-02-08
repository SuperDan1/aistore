//! Aistore storage engine library

// Global type definitions
pub mod types;

// Import various modules
pub mod buffer;
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
pub use page::PageType;
pub use page::Special;

// Re-export vfs items for easier access
pub use vfs::FileHandle;
pub use vfs::VfsError;
pub use vfs::VfsInterface;
pub use vfs::VfsResult;
