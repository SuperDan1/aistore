
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

impl BufferDesc {
    /// Initialize a new BufferDesc
    pub fn new() -> Self {
        BufferDesc {
            buf_tag: BufferTag { file_id: 0, block_id: 0 },
            state: std::sync::atomic::AtomicU64::new(0),
            io_in_progress_lock: std::sync::RwLock::new(()),
            content_lock: std::sync::RwLock::new(()),
        }
    }
}

/// Buffer manager struct
pub struct BufferMgr {
    /// Buffer pool size
    pub buffer_size: usize,
    /// Pointer to an array of BufferDesc structures
    pub buffers: *mut BufferDesc,
}

use std::alloc;

impl BufferMgr {
    /// Initialize a new BufferMgr with the specified buffer size
    pub fn init(buffer_size: usize) -> Self {
        // Calculate memory size and alignment for the buffer array
        let size = std::mem::size_of::<BufferDesc>() * buffer_size;
        let align = std::mem::align_of::<BufferDesc>();
        
        // Allocate memory for the buffer array
        let buffers_ptr = unsafe {
            let ptr = alloc::alloc(alloc::Layout::from_size_align_unchecked(size, align)) as *mut BufferDesc;
            
            // Initialize each BufferDesc in the array
            for i in 0..buffer_size {
                let buffer_ptr = ptr.add(i);
                std::ptr::write(buffer_ptr, BufferDesc::new());
            }
            
            ptr
        };
        
        BufferMgr {
            buffer_size,
            buffers: buffers_ptr,
        }
    }
}
