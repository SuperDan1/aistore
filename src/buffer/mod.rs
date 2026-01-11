
pub struct BufferTag {
    pub file_id: u16,
    pub block_id: u32
}

/// BufferDesc 结构体，用于描述缓冲区的相关属性
pub struct BufferDesc {
    /// 缓冲区大小（字节）
    pub buf_tag: BufferTag,
    /// 64位原子状态变量
    pub state: std::sync::atomic::AtomicU64,
    /// 读写锁，用于控制 I/O 操作的并发访问
    pub io_in_progress_lock: std::sync::RwLock<()>,
    pub content_lock: std::sync::RwLock<()>
}
