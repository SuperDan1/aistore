
pub struct BufferTag {
    pub file_id: u16,
    pub block_id: u32
}

/// BufferDesc struct, used to describe buffer properties
pub struct BufferDesc {
    /// Buffer tag
    pub buf_tag: BufferTag,
    /// 64-bit atomic state variable
    pub state: std::sync::atomic::AtomicU64,
    /// Read-write lock for controlling concurrent I/O access
    pub io_in_progress_lock: std::sync::RwLock<()>,
    pub content_lock: std::sync::RwLock<()>
}
