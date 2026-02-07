//! Page module
//!
//! Contains Page structure definitions for different page types
//! in the storage engine.

pub mod page;

// Re-export Page struct for easier access
pub use page::Page;
pub use page::PageType;
pub use page::Special;
