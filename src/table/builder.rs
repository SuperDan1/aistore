//! TableBuilder for fluent table construction
//!
//! Provides a builder pattern for creating tables with:
//! - Table ID assignment
//! - Column definition
//! - Validation before build

use crate::table::{Column, Table, TableType};
use crate::types::SegmentId;

/// Builder for constructing Table instances
///
/// # Example
/// ```
/// use aistore::table::{TableBuilder, Column};
/// use aistore::types::ColumnType;
///
/// let table = TableBuilder::new(1, "users".to_string())
///     .segment_id(100)
///     .column(Column::new("id".to_string(), ColumnType::Int64, false, 0))
///     .column(Column::new("name".to_string(), ColumnType::Varchar(255), true, 1))
///     .build();
/// ```
#[derive(Debug)]
pub struct TableBuilder {
    table_id: u64,
    table_name: String,
    segment_id: SegmentId,
    table_type: TableType,
    columns: Vec<Column>,
}

impl TableBuilder {
    /// Create a new table builder with required fields
    pub fn new(table_id: u64, table_name: String) -> Self {
        Self {
            table_id,
            table_name,
            segment_id: 0,
            table_type: TableType::User,
            columns: Vec::new(),
        }
    }

    /// Set the table ID
    pub fn table_id(mut self, id: u64) -> Self {
        self.table_id = id;
        self
    }

    /// Set the segment ID (required - must be allocated by Tablespace/Segment)
    pub fn segment_id(mut self, segment_id: SegmentId) -> Self {
        self.segment_id = segment_id;
        self
    }

    /// Set the table type
    pub fn table_type(mut self, table_type: TableType) -> Self {
        self.table_type = table_type;
        self
    }

    /// Add a single column
    ///
    /// Automatically assigns ordinal based on current column count
    pub fn column(mut self, column: Column) -> Self {
        let ordinal = self.columns.len() as u32;
        // Recreate the column with correct ordinal
        let new_column = Column::new(
            column.name().to_string(),
            column.column_type(),
            column.is_nullable(),
            ordinal,
        );
        self.columns.push(new_column);
        self
    }

    /// Add multiple columns at once
    pub fn columns(mut self, columns: Vec<Column>) -> Self {
        for col in columns {
            let ordinal = self.columns.len() as u32;
            let new_col = Column::new(
                col.name().to_string(),
                col.column_type(),
                col.is_nullable(),
                ordinal,
            );
            self.columns.push(new_col);
        }
        self
    }

    /// Build the table
    ///
    /// # Panics
    /// Panics if segment_id is 0 (not set)
    pub fn build(self) -> Table {
        if self.segment_id == 0 {
            panic!("TableBuilder: segment_id must be set before building");
        }

        let mut table = Table::with_columns(
            self.table_id,
            self.table_name,
            self.segment_id,
            self.columns,
        );
        table.table_type = self.table_type;
        table
    }

    /// Build with validation, returning Result instead of panicking
    pub fn try_build(self) -> Result<Table, String> {
        if self.segment_id == 0 {
            return Err("segment_id must be set".to_string());
        }

        if self.table_name.is_empty() {
            return Err("table_name cannot be empty".to_string());
        }

        let mut table = Table::with_columns(
            self.table_id,
            self.table_name,
            self.segment_id,
            self.columns,
        );
        table.table_type = self.table_type;
        Ok(table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ColumnType;

    #[test]
    fn test_table_builder_basic() {
        let table = TableBuilder::new(1, "users".to_string())
            .segment_id(100)
            .column(Column::new("id".to_string(), ColumnType::Int64, false, 0))
            .build();

        assert_eq!(table.table_id(), 1);
        assert_eq!(table.table_name(), "users");
        assert_eq!(table.segment_id(), 100);
        assert_eq!(table.column_count(), 1);
    }

    #[test]
    fn test_table_builder_multiple_columns() {
        let table = TableBuilder::new(1, "users".to_string())
            .segment_id(100)
            .column(Column::new("id".to_string(), ColumnType::Int64, false, 0))
            .column(Column::new(
                "name".to_string(),
                ColumnType::Varchar(255),
                true,
                0,
            ))
            .column(Column::new(
                "email".to_string(),
                ColumnType::Varchar(512),
                true,
                0,
            ))
            .build();

        assert_eq!(table.column_count(), 3);

        // Verify ordinals were auto-assigned
        assert_eq!(table.get_column_by_ordinal(0).unwrap().name(), "id");
        assert_eq!(table.get_column_by_ordinal(1).unwrap().name(), "name");
        assert_eq!(table.get_column_by_ordinal(2).unwrap().name(), "email");
    }

    #[test]
    fn test_table_builder_columns_batch() {
        let cols = vec![
            Column::new("a".to_string(), ColumnType::Int32, false, 0),
            Column::new("b".to_string(), ColumnType::Int32, false, 0),
        ];

        let table = TableBuilder::new(1, "test".to_string())
            .segment_id(100)
            .columns(cols)
            .build();

        assert_eq!(table.column_count(), 2);
    }

    #[test]
    fn test_table_builder_try_build() {
        // Should fail without segment_id
        let result = TableBuilder::new(1, "users".to_string()).try_build();
        assert!(result.is_err());

        // Should succeed with segment_id
        let result = TableBuilder::new(1, "users".to_string())
            .segment_id(100)
            .try_build();
        assert!(result.is_ok());
    }

    #[test]
    #[should_panic(expected = "segment_id must be set")]
    fn test_table_builder_panics_without_segment() {
        TableBuilder::new(1, "users".to_string())
            .column(Column::new("id".to_string(), ColumnType::Int64, false, 0))
            .build();
    }

    #[test]
    fn test_table_builder_with_type() {
        let table = TableBuilder::new(1, "sys_table".to_string())
            .segment_id(100)
            .table_type(TableType::System)
            .column(Column::new("id".to_string(), ColumnType::Int64, false, 0))
            .build();

        assert!(table.is_system());
    }
}
