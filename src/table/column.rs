//! Column structure for table schema definition

use crate::types::ColumnType;

/// Column metadata structure
///
/// Represents a single column in a table schema with:
/// - name: Column identifier
/// - column_type: Data type and constraints
/// - nullable: Whether NULL values are allowed
/// - ordinal: Position in table schema (0-indexed)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    /// Column name
    name: String,
    /// Column data type
    column_type: ColumnType,
    /// Whether NULL values are allowed
    nullable: bool,
    /// Column position in table (0-indexed)
    ordinal: u32,
}

impl Column {
    /// Create a new column
    ///
    /// # Arguments
    /// * `name` - Column name
    /// * `column_type` - Data type
    /// * `nullable` - Whether NULL is allowed
    /// * `ordinal` - Position in table schema
    pub fn new(name: String, column_type: ColumnType, nullable: bool, ordinal: u32) -> Self {
        Self {
            name,
            column_type,
            nullable,
            ordinal,
        }
    }

    /// Get column name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get column type
    pub fn column_type(&self) -> ColumnType {
        self.column_type
    }

    /// Check if column is nullable
    pub fn is_nullable(&self) -> bool {
        self.nullable
    }

    /// Get column ordinal (position in table)
    pub fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Get storage size for this column
    pub fn size(&self) -> usize {
        self.column_type.size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_new() {
        let col = Column::new("id".to_string(), ColumnType::Int64, false, 0);
        assert_eq!(col.name(), "id");
        assert_eq!(col.column_type(), ColumnType::Int64);
        assert!(!col.is_nullable());
        assert_eq!(col.ordinal(), 0);
        assert_eq!(col.size(), 8);
    }

    #[test]
    fn test_column_nullable() {
        let col = Column::new("name".to_string(), ColumnType::Varchar(255), true, 1);
        assert!(col.is_nullable());
        assert_eq!(col.size(), 259); // 4 + 255
    }

    #[test]
    fn test_column_ordinal() {
        let col = Column::new("age".to_string(), ColumnType::Int32, true, 5);
        assert_eq!(col.ordinal(), 5);
    }
}
