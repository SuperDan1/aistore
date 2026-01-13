
pub struct BufferTag {
    pub file_id: u16,
    pub block_id: u32
}

impl BufferTag {
    /// Hash function for BufferTag
    pub fn hash(&self) -> u64 {
        // Simple hash function combining file_id and block_id
        let mut hash = self.file_id as u64;
        hash = (hash << 32) | self.block_id as u64;
        hash
    }
}

/// Hash table entry for the buffer hash table
struct HashEntry {
    /// Buffer tag
    pub tag: BufferTag,
    /// Pointer to the corresponding BufferDesc
    pub buffer_ptr: *mut BufferDesc,
    /// Pointer to the next entry in the linked list
    pub next: *mut HashEntry,
}

impl HashEntry {
    /// Create a new HashEntry
    fn new(tag: BufferTag, buffer_ptr: *mut BufferDesc) -> Self {
        HashEntry {
            tag,
            buffer_ptr,
            next: std::ptr::null_mut(),
        }
    }
}

/// BufferDesc struct, used to describe buffer properties
/// Aligned to cacheline size defined in types.rs
#[cfg_attr(any(target_arch = "x86", target_arch = "x86_64"), repr(align(64)))] // Match CACHELINE_SIZE in types.rs
#[cfg_attr(any(target_arch = "arm", target_arch = "aarch64"), repr(align(128)))] // Match CACHELINE_SIZE in types.rs
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
    fn new() -> Self {
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
    buffer_size: usize,
    /// Pointer to an array of BufferDesc structures
    buffers: *mut BufferDesc,
    /// Buffer hash table, size is buffer_size
    /// Each entry is a pointer to a linked list of HashEntry
    buf_hash_table: *mut *mut HashEntry,
}

use std::alloc;

impl BufferMgr {
    /// Initialize a new BufferMgr with the specified buffer size
    pub fn init(buffer_size: usize) -> Self {
        // Calculate memory size and alignment for the buffer array
        let buf_size = std::mem::size_of::<BufferDesc>() * buffer_size;
        let buf_align = std::mem::align_of::<BufferDesc>();
        
        // Allocate memory for the buffer array
        let buffers_ptr = unsafe {
            let ptr = alloc::alloc(alloc::Layout::from_size_align_unchecked(buf_size, buf_align)) as *mut BufferDesc;
            
            // Initialize each BufferDesc in the array
            for i in 0..buffer_size {
                let buffer_ptr = ptr.add(i);
                std::ptr::write(buffer_ptr, BufferDesc::new());
            }
            
            ptr
        };
        
        // Calculate memory size and alignment for the hash table
        let hash_table_size = std::mem::size_of::<*mut HashEntry>() * buffer_size;
        let hash_table_align = std::mem::align_of::<*mut HashEntry>();
        
        // Allocate memory for the hash table
        let hash_table_ptr = unsafe {
            let ptr = alloc::alloc(alloc::Layout::from_size_align_unchecked(hash_table_size, hash_table_align)) as *mut *mut HashEntry;
            
            // Initialize each hash table entry to null pointer
            for i in 0..buffer_size {
                let entry_ptr = ptr.add(i);
                std::ptr::write(entry_ptr, std::ptr::null_mut());
            }
            
            ptr
        };
        
        BufferMgr {
            buffer_size,
            buffers: buffers_ptr,
            buf_hash_table: hash_table_ptr,
        }
    }
    
    /// Lookup a BufferDesc by BufferTag
    /// Returns a pointer to the BufferDesc if found, otherwise returns null pointer
    fn lookup(&self, tag: &BufferTag) -> *mut BufferDesc {
        unsafe {
            // Calculate hash value and index
            let hash = tag.hash();
            let index = (hash as usize) % self.buffer_size;
            
            // Get the head of the linked list at the calculated index
            let mut entry_ptr = *self.buf_hash_table.add(index);
            
            // Traverse the linked list to find the matching tag
            while !entry_ptr.is_null() {
                let entry = &*entry_ptr;
                if entry.tag.file_id == tag.file_id && entry.tag.block_id == tag.block_id {
                    return entry.buffer_ptr;
                }
                entry_ptr = entry.next;
            }
        }
        
        // Return null pointer if not found
        std::ptr::null_mut()
    }
    
    /// Insert a BufferDesc pointer into the hash table
    fn insert_hash_entry(&self, tag: BufferTag, buffer_ptr: *mut BufferDesc) {
        unsafe {
            // Calculate hash value and index
            let hash = tag.hash();
            let index = (hash as usize) % self.buffer_size;
            
            // Allocate memory for a new hash entry
            let entry_size = std::mem::size_of::<HashEntry>();
            let entry_align = std::mem::align_of::<HashEntry>();
            let new_entry_ptr = alloc::alloc(alloc::Layout::from_size_align_unchecked(entry_size, entry_align)) as *mut HashEntry;
            
            // Initialize the new hash entry
            let new_entry = HashEntry::new(tag, buffer_ptr);
            std::ptr::write(new_entry_ptr, new_entry);
            
            // Insert the new entry at the beginning of the linked list
            let head_ptr = self.buf_hash_table.add(index);
            (*new_entry_ptr).next = *head_ptr;
            *head_ptr = new_entry_ptr;
        }
    }
    
    /// Read a buffer by BufferTag
    /// First step: look up the corresponding BufferDesc from the hash table
    pub fn read(&self, tag: BufferTag) -> *mut BufferDesc {
        // Step 1: Look up the corresponding BufferDesc from the hash table
        let buffer_ptr = self.lookup(&tag);
        buffer_ptr
    }
}
