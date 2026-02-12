//! Catalog error definitions

use std::error::Error;
use std::fmt;

/// Catalog error types
///
/// Represents all possible errors that can occur during catalog operations
/// such as table creation, lookup, and persistence.
#[derive(Debug)]
pub enum CatalogError {
    /// Table already exists
    TableAlreadyExists(String),
    /// Table not found
    TableNotFound(String),
    /// Column already exists in table
    ColumnAlreadyExists(String),
    /// I/O error during catalog operation
    IoError(std::io::Error),
    /// Error parsing catalog data
    ParseError(String),
    /// Invalid argument provided
    InvalidArgument(String),
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CatalogError::TableAlreadyExists(name) => {
                write!(f, "Table already exists: {}", name)
            }
            CatalogError::TableNotFound(name) => write!(f, "Table not found: {}", name),
            CatalogError::ColumnAlreadyExists(name) => {
                write!(f, "Column already exists: {}", name)
            }
            CatalogError::IoError(err) => write!(f, "I/O error: {}", err),
            CatalogError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            CatalogError::InvalidArgument(msg) => write!(f, "Invalid argument: {}", msg),
        }
    }
}

impl Error for CatalogError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CatalogError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for CatalogError {
    fn from(err: std::io::Error) -> Self {
        CatalogError::IoError(err)
    }
}

/// Result type for catalog operations
pub type CatalogResult<T> = Result<T, CatalogError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_error_display() {
        let err = CatalogError::TableAlreadyExists("users".to_string());
        assert_eq!(err.to_string(), "Table already exists: users");

        let err = CatalogError::TableNotFound("orders".to_string());
        assert_eq!(err.to_string(), "Table not found: orders");
    }

    #[test]
    fn test_catalog_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let catalog_err: CatalogError = io_err.into();
        assert!(matches!(catalog_err, CatalogError::IoError(_)));
    }
}
