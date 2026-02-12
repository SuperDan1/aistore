use std::fmt;

/// Global type definitions
///
/// Stores struct definitions, constants, and type aliases used globally by the storage engine
/// Block ID type
pub type BlockId = u64;

/// Invalid block ID
pub const INVALID_BLOCK_ID: BlockId = u64::MAX;

/// Data block size (8KB)
pub const BLOCK_SIZE: usize = 8192;

/// Page size (same as block size)
pub const PAGE_SIZE: usize = BLOCK_SIZE;

/// Segment size (64MB)
pub const SEGMENT_SIZE: usize = 64 * 1024 * 1024;

/// Index entry size
pub const INDEX_ENTRY_SIZE: usize = 16;

/// Cache line size - varies by architecture
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub const CACHELINE_SIZE: usize = 64;

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
pub const CACHELINE_SIZE: usize = 128;

/// Storage engine error type
#[derive(Debug)]
pub enum AistoreError {
    /// I/O operation error
    IoError(std::io::Error),
    /// Memory allocation error
    AllocError,
    /// Index error
    IndexError(String),
    /// Data format error
    DataFormatError(String),
    /// Lock error
    LockError(String),
    /// Not found error
    NotFound,
    /// Permission error
    PermissionError,
    /// Other error
    Other(String),
}

impl std::fmt::Display for AistoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AistoreError::IoError(err) => write!(f, "I/O error: {}", err),
            AistoreError::AllocError => write!(f, "Memory allocation error"),
            AistoreError::IndexError(msg) => write!(f, "Index error: {}", msg),
            AistoreError::DataFormatError(msg) => write!(f, "Data format error: {}", msg),
            AistoreError::LockError(msg) => write!(f, "Lock error: {}", msg),
            AistoreError::NotFound => write!(f, "Not found"),
            AistoreError::PermissionError => write!(f, "Permission error"),
            AistoreError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for AistoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AistoreError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for AistoreError {
    fn from(err: std::io::Error) -> Self {
        AistoreError::IoError(err)
    }
}

/// Storage engine result type
pub type AistoreResult<T> = Result<T, AistoreError>;

/// Page ID type
pub type PageId = u64;

/// Segment ID type
pub type SegmentId = u64;

/// Tablespace ID type
pub type TablespaceId = u32;

/// Index ID type
pub type IndexId = u32;

/// Offset type
pub type Offset = u64;

/// Length type
pub type Length = u64;

/// Timestamp type
pub type Timestamp = u64;

/// Data block header
#[derive(Debug, Clone, Copy)]
pub struct BlockHeader {
    /// Block ID
    pub block_id: BlockId,
    /// Block type
    pub block_type: BlockType,
    /// Data length
    pub data_len: Length,
    /// Checksum
    pub checksum: u32,
    /// Timestamp
    pub timestamp: Timestamp,
}

/// Block type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockType {
    /// Data block
    Data,
    /// Index block
    Index,
    /// Metadata block
    Metadata,
    /// Free block
    Free,
}

/// Page header
#[derive(Debug, Clone, Copy)]
pub struct PageHeader {
    /// Page ID
    pub page_id: PageId,
    /// Page type
    pub page_type: PageType,
    /// Data length
    pub data_len: Length,
    /// Free space
    pub free_space: Length,
    /// Timestamp
    pub timestamp: Timestamp,
}

/// Page type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageType {
    /// Data page
    Data,
    /// Index page
    Index,
    /// Directory page
    Directory,
    /// Free page
    Free,
}

/// Storage engine configuration
#[derive(Debug, Clone)]
pub struct AistoreConfig {
    /// Data directory path
    pub data_dir: String,
    /// Buffer size (in blocks)
    pub buffer_size: usize,
    /// Whether to enable logging
    pub enable_log: bool,
    /// Log level
    pub log_level: LogLevel,
}

impl Default for AistoreConfig {
    fn default() -> Self {
        Self {
            data_dir: String::from("./data"),
            buffer_size: 1024, // 1024 blocks, approximately 4MB
            enable_log: true,
            log_level: LogLevel::Info,
        }
    }
}

/// Log level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Debug level
    Debug,
    /// Info level
    Info,
    /// Warn level
    Warn,
    /// Error level
    Error,
}

/// Key-value pair
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyValue {
    /// Key
    pub key: Vec<u8>,
    /// Value
    pub value: Vec<u8>,
}

/// Transaction status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionStatus {
    /// Active
    Active,
    /// Committed
    Committed,
    /// RolledBack
    RolledBack,
}

/// Transaction ID type
pub type TransactionId = u64;

/// Transaction information
#[derive(Debug, Clone)]
pub struct TransactionInfo {
    /// Transaction ID
    pub txn_id: TransactionId,
    /// Transaction status
    pub status: TransactionStatus,
    /// Start time
    pub start_time: Timestamp,
    /// End time
    pub end_time: Option<Timestamp>,
}

/// Column type enumeration for table schema
///
/// Represents the data type of a column in a table schema.
/// Each variant represents a different storage format and size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnType {
    /// 8-bit signed integer
    Int8,
    /// 16-bit signed integer
    Int16,
    /// 32-bit signed integer
    Int32,
    /// 64-bit signed integer
    Int64,
    /// 8-bit unsigned integer
    UInt8,
    /// 16-bit unsigned integer
    UInt16,
    /// 32-bit unsigned integer
    UInt32,
    /// 64-bit unsigned integer
    UInt64,
    /// 32-bit floating point
    Float32,
    /// 64-bit floating point
    Float64,
    /// Variable-length string with maximum length
    Varchar(u32),
    /// Binary large object with maximum length
    Blob(u32),
    /// Boolean
    Bool,
}

impl ColumnType {
    /// Returns the default storage size in bytes for this column type.
    ///
    /// For variable-length types (Varchar, Blob), returns the size of the
    /// length prefix (4 bytes) plus the maximum length.
    pub fn size(&self) -> usize {
        match self {
            ColumnType::Int8 | ColumnType::UInt8 | ColumnType::Bool => 1,
            ColumnType::Int16 | ColumnType::UInt16 => 2,
            ColumnType::Int32 | ColumnType::UInt32 | ColumnType::Float32 => 4,
            ColumnType::Int64 | ColumnType::UInt64 | ColumnType::Float64 => 8,
            ColumnType::Varchar(max_len) => 4 + *max_len as usize,
            ColumnType::Blob(max_len) => 4 + *max_len as usize,
        }
    }

    /// Returns true if this is a variable-length type.
    ///
    /// Variable-length types require additional metadata to store their actual length.
    pub fn is_variable_length(&self) -> bool {
        matches!(self, ColumnType::Varchar(_) | ColumnType::Blob(_))
    }

    /// Returns true if this is a numeric type (integer or floating point).
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            ColumnType::Int8
                | ColumnType::Int16
                | ColumnType::Int32
                | ColumnType::Int64
                | ColumnType::UInt8
                | ColumnType::UInt16
                | ColumnType::UInt32
                | ColumnType::UInt64
                | ColumnType::Float32
                | ColumnType::Float64
        )
    }
}

impl fmt::Display for ColumnType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColumnType::Int8 => write!(f, "INT8"),
            ColumnType::Int16 => write!(f, "INT16"),
            ColumnType::Int32 => write!(f, "INT32"),
            ColumnType::Int64 => write!(f, "INT64"),
            ColumnType::UInt8 => write!(f, "UINT8"),
            ColumnType::UInt16 => write!(f, "UINT16"),
            ColumnType::UInt32 => write!(f, "UINT32"),
            ColumnType::UInt64 => write!(f, "UINT64"),
            ColumnType::Float32 => write!(f, "FLOAT32"),
            ColumnType::Float64 => write!(f, "FLOAT64"),
            ColumnType::Varchar(max_len) => write!(f, "VARCHAR({})", max_len),
            ColumnType::Blob(max_len) => write!(f, "BLOB({})", max_len),
            ColumnType::Bool => write!(f, "BOOL"),
        }
    }
}

#[cfg(test)]
mod column_type_tests {
    use super::*;

    #[test]
    fn test_column_type_sizes() {
        assert_eq!(ColumnType::Int8.size(), 1);
        assert_eq!(ColumnType::Int16.size(), 2);
        assert_eq!(ColumnType::Int32.size(), 4);
        assert_eq!(ColumnType::Int64.size(), 8);
        assert_eq!(ColumnType::UInt8.size(), 1);
        assert_eq!(ColumnType::UInt16.size(), 2);
        assert_eq!(ColumnType::UInt32.size(), 4);
        assert_eq!(ColumnType::UInt64.size(), 8);
        assert_eq!(ColumnType::Float32.size(), 4);
        assert_eq!(ColumnType::Float64.size(), 8);
        assert_eq!(ColumnType::Bool.size(), 1);
    }

    #[test]
    fn test_column_type_variable_sizes() {
        assert_eq!(ColumnType::Varchar(100).size(), 104);
        assert_eq!(ColumnType::Varchar(255).size(), 259);
        assert_eq!(ColumnType::Blob(1024).size(), 1028);
    }

    #[test]
    fn test_column_type_display() {
        assert_eq!(ColumnType::Int8.to_string(), "INT8");
        assert_eq!(ColumnType::Int32.to_string(), "INT32");
        assert_eq!(ColumnType::UInt64.to_string(), "UINT64");
        assert_eq!(ColumnType::Float32.to_string(), "FLOAT32");
        assert_eq!(ColumnType::Varchar(255).to_string(), "VARCHAR(255)");
        assert_eq!(ColumnType::Blob(1024).to_string(), "BLOB(1024)");
        assert_eq!(ColumnType::Bool.to_string(), "BOOL");
    }

    #[test]
    fn test_column_type_is_variable_length() {
        assert!(!ColumnType::Int32.is_variable_length());
        assert!(!ColumnType::Float64.is_variable_length());
        assert!(ColumnType::Varchar(100).is_variable_length());
        assert!(ColumnType::Blob(256).is_variable_length());
    }

    #[test]
    fn test_column_type_is_numeric() {
        assert!(ColumnType::Int32.is_numeric());
        assert!(ColumnType::UInt64.is_numeric());
        assert!(ColumnType::Float32.is_numeric());
        assert!(!ColumnType::Varchar(100).is_numeric());
        assert!(!ColumnType::Bool.is_numeric());
    }
}
