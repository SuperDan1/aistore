//! Table module for managing table metadata
//!
//! This module implements table management with the following features:
//! - Table metadata storage (table_id, table_name, segment_id)
//! - System cache (syscache) for quick table lookups
//! - Table creation with automatic segment allocation

pub mod builder;
pub mod column;
pub mod syscache;
pub mod table;

pub use builder::TableBuilder;
pub use column::Column;
pub use syscache::SysCache;
pub use table::Table;
pub use table::TableType;

#[cfg(test)]
mod tests {
    include!("tests.rs");
}
