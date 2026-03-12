//! Page module
//!
//! Contains Page structure definitions for different page types
//! in the storage engine.

pub(crate) mod page;
pub(crate) mod trx_info;

// Re-export Page struct for easier access
pub(crate) use page::Page;
pub(crate) use trx_info::TrxInfoPage;
