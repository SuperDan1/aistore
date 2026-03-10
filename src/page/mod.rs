//! Page module
//!
//! Contains Page structure definitions for different page types
//! in the storage engine.

pub(crate) mod page;

// Re-export Page struct for easier access
pub(crate) use page::Page;
