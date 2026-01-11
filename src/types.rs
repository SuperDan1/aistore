//! Global type definitions
//! 
//! Stores struct definitions, constants, and type aliases used globally by the storage engine
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