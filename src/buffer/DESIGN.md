# Buffer Pool Design Document

## Overview

The Buffer Pool is a critical component of the storage engine that manages in-memory copies of disk pages. It provides:

- **Page caching**: Frequently accessed pages kept in memory for fast access
- **Page replacement**: LRU-based eviction when memory is full
- **Dirty page tracking**: Tracks modified pages for write-back
- **Concurrency control**: Safe multi-threaded access with locks
- **VFS integration**: Reads/writes pages via Virtual File System

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        BufferMgr                                  │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │ BufferDesc[] │  │  Vec<Page>   │  │  LruManager<usize>   │  │
│  │  (64B each)  │  │  page_data   │  │  (buffer_idx)       │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────────┐│
│  │              buf_hash_table (chained hash)                  ││
│  │   PageId(u64) → HashEntry链表 → BufferDesc *              ││
│  └─────────────────────────────────────────────────────────────┘│
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────────┐│
│  │  vfs: Arc<dyn VfsInterface>                                 ││
│  │  data_dir: PathBuf                                         ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

## Data Structures

### BufferTag

```rust
/// BufferTag encapsulates a PageId for buffer identification
///
/// PageId is a u64 that uniquely identifies a page in the storage engine.
/// The encoding combines file_id and block_id information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferTag {
    /// The PageId this buffer contains
    pub page_id: PageId,
}

impl BufferTag {
    /// Creates a new BufferTag from a PageId
    pub fn new(page_id: PageId) -> Self {
        Self { page_id }
    }
}
```

### BufferDesc

```rust
/// BufferDesc describes a buffer slot in the buffer pool
///
/// Aligned to cache line size (64B on x86_64, 128B on ARM) to prevent
/// false sharing between concurrent threads accessing different buffers.
#[cfg_attr(any(target_arch = "x86", target_arch = "x86_64"), repr(align(64)))]
#[cfg_attr(any(target_arch = "arm", target_arch = "aarch64"), repr(align(128)))]
pub struct BufferDesc {
    /// The page this buffer contains
    pub buf_tag: BufferTag,
    /// Atomic state variable (see STATE CONSTANTS below)
    pub state: AtomicU64,
    /// Lock for I/O in progress (prevents concurrent reads/writes to same buffer)
    pub io_in_progress_lock: RwLock<()>,
    /// Lock for content access (serializes modifications)
    pub content_lock: RwLock<()>,
}
```

#### State Constants

```rust
/// State bit layout (64-bit AtomicU64):
/// 
/// Bits 0:       Dirty flag (1 = modified, needs write-back)
/// Bits 1-7:     Reserved (future use)
/// Bits 8-63:    Pin count (reference count, max = 2^56 - 1)
///
/// +---+-------+-------------------------------------------------------+
/// | D | RSVD  |              Pin Count (56 bits)                      |
/// +---+-------+-------------------------------------------------------+
///  0   1-7                         8-63

const DIRTY_BIT: u64 = 1 << 0;
const PIN_COUNT_SHIFT: u8 = 8;
const PIN_COUNT_MASK: u64 = !(DIRTY_BIT | ((1 << PIN_COUNT_SHIFT) - 1));
const MAX_PIN_COUNT: u64 = 1 << (64 - PIN_COUNT_SHIFT); // 2^56
```

#### State Operations

```rust
impl BufferDesc {
    /// Returns true if the buffer contains uncommitted modifications
    #[inline]
    pub fn is_dirty(&self) -> bool {
        self.state.load(Ordering::Relaxed) & DIRTY_BIT != 0
    }

    /// Marks the buffer as dirty (modified)
    #[inline]
    pub fn set_dirty(&self) {
        self.state.fetch_or(DIRTY_BIT, Ordering::Relaxed);
    }

    /// Clears the dirty flag
    #[inline]
    pub fn clear_dirty(&self) {
        self.state.fetch_and(!DIRTY_BIT, Ordering::Relaxed);
    }

    /// Increments pin count and returns new count
    #[inline]
    pub fn pin(&self) -> u32 {
        let mut old_state = self.state.load(Ordering::Acquire);
        loop {
            let pin_count = (old_state >> PIN_COUNT_SHIFT) as u32;
            if pin_count >= MAX_PIN_COUNT as u32 {
                panic!("Pin count overflow");
            }
            let new_state = old_state + (1 << PIN_COUNT_SHIFT);
            match self.state.compare_exchange_weak(
                old_state,
                new_state,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return pin_count + 1,
                Err(e) => old_state = e,
            }
        }
    }

    /// Decrements pin count and returns new count
    #[inline]
    pub fn unpin(&self) -> u32 {
        let mut old_state = self.state.load(Ordering::Acquire);
        loop {
            let pin_count = (old_state >> PIN_COUNT_SHIFT) as u32;
            if pin_count == 0 {
                panic!("Pin count underflow");
            }
            let new_state = old_state - (1 << PIN_COUNT_SHIFT);
            match self.state.compare_exchange_weak(
                old_state,
                new_state,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return pin_count - 1,
                Err(e) => old_state = e,
            }
        }
    }

    /// Returns current pin count
    #[inline]
    pub fn pin_count(&self) -> u32 {
        (self.state.load(Ordering::Relaxed) >> PIN_COUNT_SHIFT) as u32
    }

    /// Returns true if buffer can be evicted (pin_count == 0)
    #[inline]
    pub fn can_evict(&self) -> bool {
        self.pin_count() == 0
    }
}
```

### BufferMgr

```rust
/// BufferMgr manages the buffer pool for the storage engine
///
/// Responsibilities:
/// - Allocate and manage fixed-size buffer pool
/// - Track buffer-to-page mappings via hash table
/// - Implement LRU-based page replacement
/// - Handle page reads/writes via VFS
/// - Track dirty pages for write-back
pub struct BufferMgr {
    /// Total number of buffers in the pool
    buffer_size: usize,
    /// Array of BufferDesc structures (raw pointer for FFI compatibility)
    buffers: *mut BufferDesc,
    /// Hash table mapping PageId to buffer indices
    /// Each bucket contains a linked list of HashEntry
    buf_hash_table: *mut *mut HashEntry,
    /// In-memory page data storage (Vec for safety)
    page_data: Vec<Page>,
    /// LRU manager tracking buffer access order
    lru: LruManager<usize>,
    /// Virtual File System interface for disk I/O
    vfs: Arc<dyn VfsInterface>,
    /// Base directory for page files
    data_dir: PathBuf,
}

/// HashEntry for chaining in the hash table
struct HashEntry {
    /// The PageId this entry maps
    tag: BufferTag,
    /// Index into buffers[] and page_data[]
    buffer_idx: usize,
    /// Next entry in the chain (null if end of list)
    next: *mut HashEntry,
}
```

## LRU Integration

### LruManager Usage

```rust
/// LruManager tracks buffer access patterns using a 3-tier list:
///
/// - Hot List: Frequently accessed pages (promoted from cold)
/// - Cold List: Recently added pages (default landing spot)
/// - Free List: Pages available for eviction
///
/// Access Pattern:
/// 1. New page → Cold List
/// 2. Access in Cold → promoted to Hot
/// 3. Eviction order: Free → Cold → Hot (LRU within each)
```

### LruManager Adapter

```rust
/// LruManager is generic over T = usize (buffer index)
/// T represents the buffer_slot number in the pool
type BufferLru = LruManager<usize>;
```

## VFS Integration

### Page File Path

```rust
impl BufferMgr {
    /// Constructs the file path for a page
    ///
    /// Page files are stored in: {data_dir}/page_{page_id >> 32}.dat
    /// This creates subdirectories/files to avoid having too many files in one dir
    fn page_file_path(&self, page_id: PageId) -> PathBuf {
        // High bits of page_id identify the file
        let file_group = page_id >> 32;
        self.data_dir.join(format!("page_{}.dat", file_group))
    }

    /// Calculates the byte offset of a page within its file
    ///
    /// Page offset within file = (page_id & 0xFFFFFFFF) * PAGE_SIZE
    fn page_offset(&self, page_id: PageId) -> u64 {
        (page_id & 0xFFFFFFFF) as u64 * PAGE_SIZE as u64
    }
}
```

## Core Workflows

### Page Read (get_page)

```rust
impl BufferMgr {
    /// Retrieves a page from the buffer pool
    ///
    /// # Arguments
    /// * `page_id` - The PageId to retrieve
    ///
    /// # Returns
    /// * `Ok(&mut Page)` - Mutable reference to the page
    /// * `Err(Error)` - If page cannot be loaded
    ///
    /// # Workflow
    /// 1. Look up page in hash table
    /// 2. If HIT:
    ///    - Update LRU (promote in access order)
    ///    - Return mutable reference to page_data[buffer_idx]
    /// 3. If MISS:
    ///    - Evict LRU pages until space available
    ///    - Read page from disk via VFS
    ///    - Insert into hash table
    ///    - Add to LRU
    ///    - Return mutable reference
    pub fn get_page(&mut self, page_id: PageId) -> Result<&mut Page, Error> {
        // Try to find in hash table
        if let Some(buffer_idx) = self.lookup(page_id) {
            // HIT: Update LRU and return page
            self.lru.access(&buffer_idx);
            return Ok(&mut self.page_data[buffer_idx]);
        }

        // MISS: Need to load from disk
        let buffer_idx = self.allocate_buffer(page_id)?;
        self.read_page_from_disk(page_id, buffer_idx)?;
        self.lru.add(buffer_idx);
        
        Ok(&mut self.page_data[buffer_idx])
    }
}
```

### Page Allocation (allocate_buffer)

```rust
impl BufferMgr {
    /// Allocates a buffer slot for the given page_id
    ///
    /// If buffer pool is full, evicts pages until space is available.
    /// Eviction prefers clean pages; dirty pages are flushed first.
    fn allocate_buffer(&mut self, page_id: PageId) -> Result<usize, Error> {
        // First, try to find an unpinned buffer
        for _ in 0..self.buffer_size {
            if let Some(buffer_idx) = self.evict_page()? {
                // Clear the old hash table entry
                let old_tag = self.buffers.add(buffer_idx).read().buf_tag;
                if old_tag.page_id != INVALID_PAGE_ID {
                    self.remove_hash_entry(old_tag.page_id);
                }
                
                // Initialize new buffer
                unsafe {
                    let buffer = &mut *self.buffers.add(buffer_idx);
                    buffer.buf_tag = BufferTag::new(page_id);
                    buffer.state.store(0, Ordering::Relaxed);
                }
                return Ok(buffer_idx);
            }
        }
        
        Err(Error::BufferPoolFull)
    }

    /// Evicts a single page from the buffer pool
    ///
    /// # Returns
    /// * `Ok(Some(buffer_idx))` - Buffer index available for reuse
    /// * `Ok(None)` - All buffers pinned, try again
    /// * `Err(Error)` - I/O error during flush
    fn evict_page(&mut self) -> Result<Option<usize>, Error> {
        // Try to evict from LRU
        if let Some(buffer_idx) = self.lru.evict() {
            let buffer = unsafe { &*self.buffers.add(buffer_idx) };
            
            // Check if buffer is pinned
            if !buffer.can_evict() {
                // Can't evict pinned buffer, put it back
                self.lru.add(buffer_idx);
                return Ok(None);
            }
            
            // Check if dirty and needs flush
            if buffer.is_dirty() {
                // Flush dirty page to disk
                let page = &self.page_data[buffer_idx];
                self.write_page_to_disk(buffer.buf_tag.page_id, page)?;
                buffer.clear_dirty();
            }
            
            return Ok(Some(buffer_idx));
        }
        
        Ok(None)
    }
}
```

### Page Write (mark_dirty)

```rust
impl BufferMgr {
    /// Marks a page as dirty (modified)
    ///
    /// The dirty flag indicates the page needs to be written to disk
    /// before being evicted from the buffer pool.
    ///
    /// # Arguments
    /// * `page_id` - The PageId to mark dirty
    ///
    /// # Returns
    /// * `Ok(())` - Successfully marked
    /// * `Err(Error)` - Page not found in buffer pool
    pub fn mark_dirty(&mut self, page_id: PageId) -> Result<(), Error> {
        let buffer_idx = self.lookup(page_id)
            .ok_or_else(|| Error::PageNotFound(page_id))?;
        
        let buffer = unsafe { &*self.buffers.add(buffer_idx) };
        buffer.set_dirty();
        
        Ok(())
    }
}
```

### Page Release (unpin_page)

```rust
impl BufferMgr {
    /// Releases a pin on a page
    ///
    /// Pages are pinned when accessed and must be unpinned when done.
    /// When a page's pin count reaches 0, it becomes eligible for eviction.
    ///
    /// # Arguments
    /// * `page_id` - The PageId to unpin
    ///
    /// # Returns
    /// * `Ok(())` - Successfully unpinned
    /// * `Err(Error)` - Page not found
    pub fn unpin_page(&mut self, page_id: PageId) -> Result<(), Error> {
        let buffer_idx = self.lookup(page_id)
            .ok_or_else(|| Error::PageNotFound(page_id))?;
        
        let buffer = unsafe { &*self.buffers.add(buffer_idx) };
        buffer.unpin();
        
        Ok(())
    }
}
```

### Hash Table Operations

```rust
impl BufferMgr {
    /// Looks up a PageId in the hash table
    ///
    /// # Returns
    /// * `Some(buffer_idx)` - Buffer index containing the page
    /// * `None` - Page not in buffer pool
    pub fn lookup(&self, page_id: PageId) -> Option<usize> {
        unsafe {
            // Hash the page_id
            let hash = fnv1a_hash(&page_id.to_le_bytes());
            let index = (hash as usize) % self.buffer_size;
            
            // Traverse the chain
            let mut entry_ptr = *self.buf_hash_table.add(index);
            while !entry_ptr.is_null() {
                let entry = &*entry_ptr;
                if entry.tag.page_id == page_id {
                    return Some(entry.buffer_idx);
                }
                entry_ptr = entry.next;
            }
        }
        None
    }

    /// Inserts a page_id -> buffer_idx mapping
    fn insert_hash_entry(&mut self, page_id: PageId, buffer_idx: usize) {
        unsafe {
            let hash = fnv1a_hash(&page_id.to_le_bytes());
            let index = (hash as usize) % self.buffer_size;
            
            // Allocate new entry
            let entry_size = std::mem::size_of::<HashEntry>();
            let entry_align = std::mem::align_of::<HashEntry>();
            let new_entry_ptr = std::alloc::alloc(
                std::alloc::Layout::from_size_align_unchecked(entry_size, entry_align)
            ) as *mut HashEntry;
            
            // Initialize entry
            std::ptr::write(new_entry_ptr, HashEntry {
                tag: BufferTag::new(page_id),
                buffer_idx,
                next: *self.buf_hash_table.add(index),
            });
            
            // Insert at head
            *self.buf_hash_table.add(index) = new_entry_ptr;
        }
    }

    /// Removes a page_id from the hash table
    fn remove_hash_entry(&mut self, page_id: PageId) {
        unsafe {
            let hash = fnv1a_hash(&page_id.to_le_bytes());
            let index = (hash as usize) % self.buffer_size;
            
            let mut entry_ptr = *self.buf_hash_table.add(index);
            let mut prev_ptr: *mut HashEntry = std::ptr::null_mut();
            
            while !entry_ptr.is_null() {
                let entry = &*entry_ptr;
                if entry.tag.page_id == page_id {
                    let next = entry.next;
                    if prev_ptr.is_null() {
                        // Head of list
                        *self.buf_hash_table.add(index) = next;
                    } else {
                        (*prev_ptr).next = next;
                    }
                    // Free the entry
                    let entry_layout = std::alloc::Layout::new::<HashEntry>();
                    std::alloc::dealloc(entry_ptr as *mut u8, entry_layout);
                    return;
                }
                prev_ptr = entry_ptr;
                entry_ptr = entry.next;
            }
        }
    }
}
```

### Disk I/O Operations

```rust
impl BufferMgr {
    /// Reads a page from disk into the buffer pool
    fn read_page_from_disk(&self, page_id: PageId, buffer_idx: usize) -> Result<(), Error> {
        let file_path = self.page_file_path(page_id);
        let offset = self.page_offset(page_id);
        
        // Get exclusive access to the buffer
        let buffer = unsafe { &*self.buffers.add(buffer_idx) };
        let _io_guard = buffer.io_in_progress_lock.write().unwrap();
        
        // Read raw bytes from VFS using pread
        let page_ptr = &mut self.page_data[buffer_idx] as *mut Page as *mut u8;
        let mut read_buf = unsafe { std::slice::from_raw_parts_mut(page_ptr, PAGE_SIZE) };
        
        // Use VFS.pread to read at offset
        self.vfs.pread(&file_path, &mut read_buf, offset)?;
        
        Ok(())
    }

    /// Writes a page from buffer to disk
    fn write_page_to_disk(&self, page_id: PageId, page: &Page) -> Result<(), Error> {
        let file_path = self.page_file_path(page_id);
        let offset = self.page_offset(page_id);
        
        // Use VFS.pwrite to write at offset
        let page_ptr = page as *const Page as *const u8;
        let write_buf = unsafe { std::slice::from_raw_parts(page_ptr, PAGE_SIZE) };
        
        self.vfs.pwrite(&file_path, write_buf, offset)?;
        
        Ok(())
    }
}
```

### Flush Operations

```rust
impl BufferMgr {
    /// Flushes all dirty pages to disk
    ///
    /// Iterates through all buffers and writes dirty pages.
    /// Does NOT clear dirty flags (caller may want to keep tracking).
    pub fn flush_all(&mut self) -> Result<(), Error> {
        for buffer_idx in 0..self.buffer_size {
            let buffer = unsafe { &*self.buffers.add(buffer_idx) };
            
            if buffer.is_dirty() {
                let page = &self.page_data[buffer_idx];
                self.write_page_to_disk(buffer.buf_tag.page_id, page)?;
                buffer.clear_dirty();
            }
        }
        Ok(())
    }
}
```

## Concurrency Control

### Lock Ordering

To prevent deadlocks, always acquire locks in this order:

1. `io_in_progress_lock` (RwLock)
2. `content_lock` (RwLock)

### Thread Safety Guarantees

```rust
/// Thread Safety:
///
/// - Multiple threads can READ the same buffer concurrently
/// - Single thread can WRITE (mark dirty) with exclusive access
/// - I/O operations are serialized per buffer
/// - Hash table operations are lock-free (atomic pointers)
/// - LRU modifications require exclusive access (mut&)
///
/// # Example: Reading a page
///
/// ```
/// let page = buffer_mgr.get_page(page_id)?;
/// // Multiple threads can read simultaneously
/// let checksum = calculate_checksum(&page.header);
/// ```
///
/// # Example: Modifying a page
///
/// ```
/// let page = buffer_mgr.get_page(page_id)?;
/// let _content_guard = buffer.content_lock.write().unwrap();
/// // Exclusive access for modification
/// page.header.checksum = recalculate_checksum(page);
/// buffer_mgr.mark_dirty(page_id)?;
/// ```
```

## Initialization

```rust
impl BufferMgr {
    /// Creates a new BufferMgr with the specified buffer size
    ///
    /// # Arguments
    /// * `buffer_size` - Number of buffers in the pool
    /// * `vfs` - Virtual File System interface
    /// * `data_dir` - Directory containing page files
    pub fn init(
        buffer_size: usize,
        vfs: Arc<dyn VfsInterface>,
        data_dir: PathBuf,
    ) -> Self {
        // Calculate memory layouts
        let buf_size = std::mem::size_of::<BufferDesc>() * buffer_size;
        let buf_align = std::mem::align_of::<BufferDesc>();
        
        // Allocate buffer array
        let buffers_ptr = unsafe {
            std::alloc::alloc_zeroed(
                std::alloc::Layout::from_size_align_unchecked(buf_size, buf_align)
            ) as *mut BufferDesc
        };
        
        // Initialize buffers
        for i in 0..buffer_size {
            let buffer_ptr = buffers_ptr.add(i);
            std::ptr::write(buffer_ptr, BufferDesc {
                buf_tag: BufferTag { page_id: INVALID_PAGE_ID },
                state: AtomicU64::new(0),
                io_in_progress_lock: RwLock::new(()),
                content_lock: RwLock::new(()),
            });
        }
        
        // Allocate hash table
        let hash_size = std::mem::size_of::<*mut HashEntry>() * buffer_size;
        let hash_align = std::mem::align_of::<*mut HashEntry>();
        let hash_table_ptr = unsafe {
            std::alloc::alloc_zeroed(
                std::alloc::Layout::from_size_align_unchecked(hash_size, hash_align)
            ) as *mut *mut HashEntry
        };
        
        // Initialize hash table entries to null
        for i in 0..buffer_size {
            let entry_ptr = hash_table_ptr.add(i);
            std::ptr::write(entry_ptr, std::ptr::null_mut());
        }
        
        // Create page data storage
        let page_data = vec![Page::new(); buffer_size];
        
        // LRU manager (hot=50%, cold=30%, free=20% of pool)
        let hot_cap = buffer_size / 2;
        let cold_cap = buffer_size * 3 / 10;
        let free_cap = buffer_size / 5;
        let lru = LruManager::new(hot_cap, cold_cap, free_cap);
        
        BufferMgr {
            buffer_size,
            buffers: buffers_ptr,
            buf_hash_table: hash_table_ptr,
            page_data,
            lru,
            vfs,
            data_dir,
        }
    }
}
```

## Error Handling

```rust
/// Buffer Pool Errors
#[derive(Debug, thiserror::Error)]
pub enum BufferError {
    #[error("Page {0} not found in buffer pool")]
    PageNotFound(PageId),
    
    #[error("Buffer pool is full")]
    BufferPoolFull,
    
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("VFS error: {0}")]
    VfsError(#[from] VfsError),
    
    #[error("Page {0} is pinned and cannot be evicted")]
    PagePinned(PageId),
    
    #[error("Invalid page ID: {0}")]
    InvalidPageId(PageId),
}

pub type BufferResult<T> = Result<T, BufferError>;
```

## Performance Considerations

### Cache Line Alignment

```rust
/// BufferDesc is aligned to cache line size (64B on x86_64)
/// This prevents false sharing between threads accessing different buffers.
/// 
/// Each BufferDesc occupies exactly one cache line when allocated,
/// ensuring that concurrent access to different buffers doesn't cause
/// cache line ping-ponging between CPU cores.
```

### Memory Layout

```
Buffer Pool Memory Layout (8 buffers example):

┌─────────────────────────────────────────────────────────────────────┐
│ BufferDesc[0]  │ BufferDesc[1]  │ ... │ BufferDesc[7]            │
│   (64B each)   │   (64B each)   │     │   (64B each)             │
├─────────────────────────────────────────────────────────────────────┤
│ page_data[0]  │ page_data[1]   │ ... │ page_data[7]              │
│   (8KB each)  │   (8KB each)   │     │   (8KB each)             │
├─────────────────────────────────────────────────────────────────────┤
│ Hash Table (8 entries, each 8 bytes)                               │
└─────────────────────────────────────────────────────────────────────┘

Total: 8 * (64 + 8192) + 64 ≈ 65.5 KB
```

### Hash Table Performance

```rust
/// The hash table uses chaining for collision resolution.
/// Expected chain length = buffer_size / hash_table_size ≈ 1
/// 
/// In the worst case with many hash collisions,
/// chain traversal is O(n) where n is the chain length.
/// With proper hash function (FNV-1a), collisions are rare.
```

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_buffer_tag_creation() {
        let tag = BufferTag::new(12345);
        assert_eq!(tag.page_id, 12345);
    }
    
    #[test]
    fn test_dirty_flag() {
        let desc = BufferDesc::new();
        assert!(!desc.is_dirty());
        desc.set_dirty();
        assert!(desc.is_dirty());
        desc.clear_dirty();
        assert!(!desc.is_dirty());
    }
    
    #[test]
    fn test_pin_count() {
        let desc = BufferDesc::new();
        assert_eq!(desc.pin_count(), 0);
        assert_eq!(desc.pin(), 1);
        assert_eq!(desc.pin(), 2);
        assert_eq!(desc.unpin(), 1);
        assert_eq!(desc.unpin(), 0);
    }
    
    #[test]
    #[should_panic(expected = "Pin count underflow")]
    fn test_pin_underflow_panics() {
        let desc = BufferDesc::new();
        desc.unpin(); // Should panic
    }
}
```

### Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    #[test]
    fn test_get_page_hit() {
        // Setup buffer pool
        // Insert a page
        // Call get_page with same page_id
        // Verify page returned and LRU updated
    }
    
    #[test]
    fn test_get_page_miss() {
        // Setup buffer pool with limited size
        // Fill pool
        // Access new page (trigger eviction)
        // Verify page loaded from disk
    }
    
    #[test]
    fn test_dirty_page_eviction() {
        // Load page, mark dirty
        // Evict it
        // Verify write to disk occurred
    }
}
```

## Future Enhancements

- [ ] **Clock-Pro LRU**: More accurate LRU approximation for large pools
- [ ] **Hot/Cold Separation**: Separate pools for read-heavy vs write-heavy workloads
- [ ] **Compression**: Compress pages in memory
- [ ] **Prefetching**: Anticipate page access patterns
- [ ] **Buffer Pool Advisors**: Statistics for query optimization
