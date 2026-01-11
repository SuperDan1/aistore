//! 全局类型定义
//! 
//! 存放存储引擎全局需要用到的结构体定义、常量和类型别名
/// 块 ID 类型
pub type BlockId = u64;

/// 无效块 ID
pub const INVALID_BLOCK_ID: BlockId = u64::MAX;

/// 数据块大小 (8KB)
pub const BLOCK_SIZE: usize = 8192;

/// 页大小 (与块大小相同)
pub const PAGE_SIZE: usize = BLOCK_SIZE;

/// 段大小 (64MB)
pub const SEGMENT_SIZE: usize = 64 * 1024 * 1024;

/// 索引项大小
pub const INDEX_ENTRY_SIZE: usize = 16;

/// 存储引擎错误类型
#[derive(Debug)]
pub enum AistoreError {
    /// I/O 操作错误
    IoError(std::io::Error),
    /// 内存分配错误
    AllocError,
    /// 索引错误
    IndexError(String),
    /// 数据格式错误
    DataFormatError(String),
    /// 锁错误
    LockError(String),
    /// 未找到错误
    NotFound,
    /// 权限错误
    PermissionError,
    /// 其他错误
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

/// 存储引擎结果类型
pub type AistoreResult<T> = Result<T, AistoreError>;



/// 页 ID 类型
pub type PageId = u64;

/// 段 ID 类型
pub type SegmentId = u64;

/// 表空间 ID 类型
pub type TablespaceId = u32;

/// 索引 ID 类型
pub type IndexId = u32;

/// 偏移量类型
pub type Offset = u64;

/// 长度类型
pub type Length = u64;

/// 时间戳类型
pub type Timestamp = u64;

/// 数据块头部
#[derive(Debug, Clone, Copy)]
pub struct BlockHeader {
    /// 块 ID
    pub block_id: BlockId,
    /// 块类型
    pub block_type: BlockType,
    /// 数据长度
    pub data_len: Length,
    /// 校验和
    pub checksum: u32,
    /// 时间戳
    pub timestamp: Timestamp,
}

/// 块类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockType {
    /// 数据块
    Data,
    /// 索引块
    Index,
    /// 元数据块
    Metadata,
    /// 空闲块
    Free,
}

/// 页头部
#[derive(Debug, Clone, Copy)]
pub struct PageHeader {
    /// 页 ID
    pub page_id: PageId,
    /// 页类型
    pub page_type: PageType,
    /// 数据长度
    pub data_len: Length,
    /// 空闲空间
    pub free_space: Length,
    /// 时间戳
    pub timestamp: Timestamp,
}

/// 页类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageType {
    /// 数据页
    Data,
    /// 索引页
    Index,
    /// 目录页
    Directory,
    /// 空闲页
    Free,
}

/// 存储引擎配置
#[derive(Debug, Clone)]
pub struct AistoreConfig {
    /// 数据目录路径
    pub data_dir: String,
    /// 缓冲区大小 (以块为单位)
    pub buffer_size: usize,
    /// 是否启用日志
    pub enable_log: bool,
    /// 日志级别
    pub log_level: LogLevel,
}

impl Default for AistoreConfig {
    fn default() -> Self {
        Self {
            data_dir: String::from("./data"),
            buffer_size: 1024, // 1024 个块，约 4MB
            enable_log: true,
            log_level: LogLevel::Info,
        }
    }
}

/// 日志级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// 调试级别
    Debug,
    /// 信息级别
    Info,
    /// 警告级别
    Warn,
    /// 错误级别
    Error,
}

/// 键值对
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyValue {
    /// 键
    pub key: Vec<u8>,
    /// 值
    pub value: Vec<u8>,
}

/// 事务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionStatus {
    /// 活跃
    Active,
    /// 已提交
    Committed,
    /// 已回滚
    RolledBack,
}

/// 事务 ID 类型
pub type TransactionId = u64;

/// 事务信息
#[derive(Debug, Clone)]
pub struct TransactionInfo {
    /// 事务 ID
    pub txn_id: TransactionId,
    /// 事务状态
    pub status: TransactionStatus,
    /// 开始时间
    pub start_time: Timestamp,
    /// 结束时间
    pub end_time: Option<Timestamp>,
}