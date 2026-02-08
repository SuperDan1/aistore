//! Table structure for storing table metadata

use crate::types::SegmentId;
use std::fmt;

/// Table type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableType {
    /// User-defined table
    User,
    /// System table
    System,
    /// Temporary table
    Temporary,
}

impl fmt::Display for TableType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TableType::User => write!(f, "User"),
            TableType::System => write!(f, "System"),
            TableType::Temporary => write!(f, "Temporary"),
        }
    }
}

/// Table metadata structure
///
/// Stores basic metadata for a table including:
/// - table_id: Unique identifier for the table
/// - table_name: Human-readable name
/// - segment_id: Associated storage segment
/// - table_type: Type of table (user/system/temporary)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// Unique table identifier
    pub table_id: u64,
    /// Table name
    pub table_name: String,
    /// Associated segment ID for storage
    pub segment_id: SegmentId,
    /// Table type
    pub table_type: TableType,
    /// Row count (current implementation: memory-only)
    pub row_count: u64,
    /// Column count
    pub column_count: u32,
    /// Creation timestamp
    pub created_at: u64,
}

impl Table {
    /// Create a new table with basic fields
    pub fn new(table_id: u64, table_name: String, segment_id: SegmentId) -> Self {
        Self {
            table_id,
            table_name,
            segment_id,
            table_type: TableType::User,
            row_count: 0,
            column_count: 0,
            created_at: current_timestamp(),
        }
    }

    /// Create a new table with specified type
    pub fn with_type(
        table_id: u64,
        table_name: String,
        segment_id: SegmentId,
        table_type: TableType,
    ) -> Self {
        Self {
            table_id,
            table_name,
            segment_id,
            table_type,
            row_count: 0,
            column_count: 0,
            created_at: current_timestamp(),
        }
    }

    /// Get table ID
    pub fn table_id(&self) -> u64 {
        self.table_id
    }

    /// Get table name
    pub fn table_name(&self) -> &str {
        &self.table_name
    }

    /// Get segment ID
    pub fn segment_id(&self) -> SegmentId {
        self.segment_id
    }

    /// Get table type
    pub fn table_type(&self) -> TableType {
        self.table_type
    }

    /// Check if this is a system table
    pub fn is_system(&self) -> bool {
        self.table_type == TableType::System
    }

    /// Check if this is a temporary table
    pub fn is_temporary(&self) -> bool {
        self.table_type == TableType::Temporary
    }
}

/// Get current timestamp (simple counter for now)
fn current_timestamp() -> u64 {
    // Use std::time::SystemTime::now() for actual timestamp
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_new() {
        let table = Table::new(1, "test_table".to_string(), 100);
        assert_eq!(table.table_id(), 1);
        assert_eq!(table.table_name(), "test_table");
        assert_eq!(table.segment_id(), 100);
        assert!(!table.is_system());
        assert!(!table.is_temporary());
    }

    #[test]
    fn test_table_with_type() {
        let table = Table::with_type(2, "sys_table".to_string(), 200, TableType::System);
        assert_eq!(table.table_id(), 2);
        assert_eq!(table.table_name(), "sys_table");
        assert!(table.is_system());
    }

    #[test]
    fn test_table_temporary() {
        let table = Table::with_type(3, "temp_table".to_string(), 300, TableType::Temporary);
        assert!(table.is_temporary());
    }
}
