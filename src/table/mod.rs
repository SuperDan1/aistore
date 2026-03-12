//! Table module for managing table metadata
//!
//! This module implements table management with the following features:
//! - Table metadata storage (table_id, table_name, segment_id)
//! - System cache (syscache) for quick table lookups
//! - Table creation with automatic segment allocation

mod builder;
mod column;
mod syscache;
mod table;

pub(crate) use builder::TableBuilder;
pub use column::Column;
pub(crate) use table::Table;
pub(crate) use table::TableType;

#[cfg(test)]
pub(crate) use syscache::SysCache;

#[cfg(test)]
mod tests {
    include!("tests.rs");
}
