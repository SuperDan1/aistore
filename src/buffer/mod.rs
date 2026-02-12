//! Buffer Pool implementation for the storage engine
//!
//! Provides page caching, LRU-based replacement, dirty page tracking,
//! and VFS-based disk I/O.

pub mod lru;

use crate::infrastructure::hash::fnv1a_hash;
use crate::page::Page;
use crate::types::{PAGE_SIZE, PageId};
use crate::vfs::{VfsError, VfsInterface};
use lru::LruManager;
use std::alloc;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{fmt, mem};

/// Invalid page ID constant
pub const INVALID_PAGE_ID: PageId = PageId::MAX;

/// State bit layout (64-bit AtomicU64):
///
/// Bits 0:       Dirty flag (1 = modified, needs write-back)
/// Bits 1-7:     Reserved (future use)
/// Bits 8-63:    Pin count (reference count)
///
/// +---+-------+-------------------------------------------------------+
/// | D | RSVD  |              Pin Count (56 bits)                      |
/// +---+-------+-------------------------------------------------------+
///  0   1-7                         8-63

const DIRTY_BIT: u64 = 1 << 0;
const PIN_COUNT_SHIFT: u8 = 8;

/// BufferTag encapsulates a PageId for buffer identification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferTag {
    /// The PageId this buffer contains
    pub page_id: PageId,
}

impl BufferTag {
    /// Creates a new BufferTag from a PageId
    #[inline]
    pub fn new(page_id: PageId) -> Self {
        Self { page_id }
    }
}

/// Hash table entry for the buffer hash table
struct HashEntry {
    /// The PageId this entry maps
    tag: BufferTag,
    /// Index into buffers[] and page_data[]
    buffer_idx: usize,
    /// Next entry in the chain (null if end of list)
    next: *mut HashEntry,
}

impl HashEntry {
    /// Create a new HashEntry
    #[inline]
    fn new(tag: BufferTag, buffer_idx: usize) -> Self {
        Self {
            tag,
            buffer_idx,
            next: std::ptr::null_mut(),
        }
    }
}

/// BufferDesc struct, used to describe buffer properties
/// Aligned to cache line size to prevent false sharing
#[cfg_attr(any(target_arch = "x86", target_arch = "x86_64"), repr(align(64)))]
#[cfg_attr(any(target_arch = "arm", target_arch = "aarch64"), repr(align(128)))]
pub struct BufferDesc {
    /// Buffer tag
    pub buf_tag: BufferTag,
    /// 64-bit atomic state variable (dirty bit + pin count)
    state: AtomicU64,
    /// Read-write lock for controlling concurrent I/O access
    pub io_in_progress_lock: std::sync::RwLock<()>,
    /// Lock for content access (serializes modifications)
    pub content_lock: std::sync::RwLock<()>,
}

impl BufferDesc {
    /// Creates a new BufferDesc
    #[inline]
    fn new() -> Self {
        Self {
            buf_tag: BufferTag::new(INVALID_PAGE_ID),
            state: AtomicU64::new(0),
            io_in_progress_lock: std::sync::RwLock::new(()),
            content_lock: std::sync::RwLock::new(()),
        }
    }

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
            if pin_count >= (u64::MAX >> PIN_COUNT_SHIFT) as u32 {
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

/// Buffer manager errors
#[derive(Debug, PartialEq)]
pub enum BufferError {
    /// Page not found in buffer pool
    PageNotFound(PageId),
    /// Buffer pool is full
    BufferPoolFull,
    /// Page is pinned and cannot be evicted
    PagePinned(PageId),
    /// Invalid page ID
    InvalidPageId(PageId),
}

impl fmt::Display for BufferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BufferError::PageNotFound(id) => write!(f, "Page {} not found in buffer pool", id),
            BufferError::BufferPoolFull => write!(f, "Buffer pool is full"),
            BufferError::PagePinned(id) => write!(f, "Page {} is pinned", id),
            BufferError::InvalidPageId(id) => write!(f, "Invalid page ID: {}", id),
        }
    }
}

impl std::error::Error for BufferError {}

impl From<VfsError> for BufferError {
    fn from(err: VfsError) -> Self {
        BufferError::PageNotFound(0) // VFS errors treated as page not found
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
    /// In-memory page data storage
    page_data: Vec<Page>,
    /// LRU manager tracking buffer access order (using buffer_idx as key)
    lru: LruManager<usize>,
    /// Virtual File System interface for disk I/O
    vfs: Arc<dyn VfsInterface>,
    /// Base directory for page files
    data_dir: PathBuf,
}

impl BufferMgr {
    /// Creates a new BufferMgr with the specified buffer size
    ///
    /// # Arguments
    /// * `buffer_size` - Number of buffers in the pool
    /// * `vfs` - Virtual File System interface
    /// * `data_dir` - Directory containing page files
    pub fn init(buffer_size: usize, vfs: Arc<dyn VfsInterface>, data_dir: PathBuf) -> Self {
        // Calculate memory layouts
        let buf_size = mem::size_of::<BufferDesc>() * buffer_size;
        let buf_align = mem::align_of::<BufferDesc>();

        // Allocate buffer array
        let buffers_ptr = unsafe {
            let ptr = alloc::alloc_zeroed(alloc::Layout::from_size_align_unchecked(
                buf_size, buf_align,
            )) as *mut BufferDesc;

            // Initialize each BufferDesc in the array
            for i in 0..buffer_size {
                let buffer_ptr = ptr.add(i);
                std::ptr::write(buffer_ptr, BufferDesc::new());
            }

            ptr
        };

        // Allocate hash table
        let hash_table_size = mem::size_of::<*mut HashEntry>() * buffer_size;
        let hash_table_align = mem::align_of::<*mut HashEntry>();
        let hash_table_ptr = unsafe {
            let ptr = alloc::alloc_zeroed(alloc::Layout::from_size_align_unchecked(
                hash_table_size,
                hash_table_align,
            )) as *mut *mut HashEntry;

            // Initialize each hash table entry to null pointer
            for i in 0..buffer_size {
                let entry_ptr = ptr.add(i);
                std::ptr::write(entry_ptr, std::ptr::null_mut());
            }

            ptr
        };

        // Create page data storage (Vec for safety)
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

    /// Constructs the file path for a page
    fn page_file_path(&self, page_id: PageId) -> PathBuf {
        // High bits of page_id identify the file
        let file_group = page_id >> 32;
        self.data_dir.join(format!("page_{}.dat", file_group))
    }

    /// Calculates the byte offset of a page within its file
    fn page_offset(&self, page_id: PageId) -> u64 {
        (page_id & 0xFFFFFFFF) as u64 * PAGE_SIZE as u64
    }

    /// Looks up a PageId in the hash table
    ///
    /// # Returns
    /// * `Some(buffer_idx)` - Buffer index containing the page
    /// * `None` - Page not in buffer pool
    pub fn lookup(&self, page_id: PageId) -> Option<usize> {
        unsafe {
            // Hash the page_id as string
            let page_id_str = page_id.to_string();
            let hash = fnv1a_hash(&page_id_str);
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
            let page_id_str = page_id.to_string();
            let hash = fnv1a_hash(&page_id_str);
            let index = (hash as usize) % self.buffer_size;

            // Allocate new entry
            let entry_size = mem::size_of::<HashEntry>();
            let entry_align = mem::align_of::<HashEntry>();
            let new_entry_ptr = alloc::alloc(alloc::Layout::from_size_align_unchecked(
                entry_size,
                entry_align,
            )) as *mut HashEntry;

            // Initialize entry
            std::ptr::write(
                new_entry_ptr,
                HashEntry::new(BufferTag::new(page_id), buffer_idx),
            );

            // Insert at head
            let head_ptr = self.buf_hash_table.add(index);
            (*new_entry_ptr).next = *head_ptr;
            *head_ptr = new_entry_ptr;
        }
    }

    /// Removes a page_id from the hash table
    fn remove_hash_entry(&mut self, page_id: PageId) {
        unsafe {
            let page_id_str = page_id.to_string();
            let hash = fnv1a_hash(&page_id_str);
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
                    let entry_layout = alloc::Layout::new::<HashEntry>();
                    alloc::dealloc(entry_ptr as *mut u8, entry_layout);
                    return;
                }
                prev_ptr = entry_ptr;
                entry_ptr = entry.next;
            }
        }
    }

    /// Reads a page from disk into the buffer pool
    fn read_page_from_disk(
        &mut self,
        page_id: PageId,
        buffer_idx: usize,
    ) -> Result<(), BufferError> {
        let file_path = self.page_file_path(page_id);
        let offset = self.page_offset(page_id);

        // Get exclusive access to the buffer
        let buffer = unsafe { &mut *self.buffers.add(buffer_idx) };
        let _io_guard = buffer.io_in_progress_lock.write().unwrap();

        // Read raw bytes from VFS using pread
        let page_ptr = &mut self.page_data[buffer_idx] as *mut Page as *mut u8;
        let read_buf = unsafe { std::slice::from_raw_parts_mut(page_ptr, PAGE_SIZE) };

        // Use VFS.pread to read at offset
        self.vfs
            .pread(file_path.to_str().unwrap(), read_buf, offset)?;

        Ok(())
    }

    /// Writes a page from buffer to disk
    fn write_page_to_disk(&self, page_id: PageId, page: &Page) -> Result<(), BufferError> {
        let file_path = self.page_file_path(page_id);
        let offset = self.page_offset(page_id);

        // Use VFS.pwrite to write at offset
        let page_ptr = page as *const Page as *const u8;
        let write_buf = unsafe { std::slice::from_raw_parts(page_ptr, PAGE_SIZE) };

        self.vfs
            .pwrite(file_path.to_str().unwrap(), write_buf, offset)?;

        Ok(())
    }

    /// Evicts a single page from the buffer pool
    ///
    /// # Returns
    /// * `Ok(Some(buffer_idx))` - Buffer index available for reuse
    /// * `Ok(None)` - All buffers pinned, try again
    fn evict_page(&mut self) -> Result<Option<usize>, BufferError> {
        // Try to evict from LRU
        if let Some(node) = self.lru.evict() {
            let buffer_idx = node.data;

            let buffer = unsafe { &mut *self.buffers.add(buffer_idx) };

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

            // Clear the hash table entry
            if buffer.buf_tag.page_id != INVALID_PAGE_ID {
                self.remove_hash_entry(buffer.buf_tag.page_id);
            }

            // Reset buffer state
            buffer.buf_tag = BufferTag::new(INVALID_PAGE_ID);

            return Ok(Some(buffer_idx));
        }

        Ok(None)
    }

    /// Allocates a buffer slot for the given page_id
    fn allocate_buffer(&mut self, page_id: PageId) -> Result<usize, BufferError> {
        // First, try to find an unpinned buffer
        for _ in 0..self.buffer_size {
            if let Some(buffer_idx) = self.evict_page()? {
                // Initialize new buffer
                unsafe {
                    let buffer = &mut *self.buffers.add(buffer_idx);
                    buffer.buf_tag = BufferTag::new(page_id);
                    buffer.state.store(0, Ordering::Relaxed);
                }
                return Ok(buffer_idx);
            }
        }

        Err(BufferError::BufferPoolFull)
    }

    /// Retrieves a page from the buffer pool
    ///
    /// # Arguments
    /// * `page_id` - The PageId to retrieve
    ///
    /// # Returns
    /// * `Ok(&mut Page)` - Mutable reference to the page
    /// * `Err(BufferError)` - If page cannot be loaded
    pub fn get_page(&mut self, page_id: PageId) -> Result<&mut Page, BufferError> {
        // Try to find in hash table
        if let Some(buffer_idx) = self.lookup(page_id) {
            // HIT: Update LRU and return page
            self.lru.access(&buffer_idx);

            // Pin the buffer
            let buffer = unsafe { &*self.buffers.add(buffer_idx) };
            buffer.pin();

            return Ok(&mut self.page_data[buffer_idx]);
        }

        // MISS: Need to load from disk
        let buffer_idx = self.allocate_buffer(page_id)?;
        self.read_page_from_disk(page_id, buffer_idx)?;
        self.insert_hash_entry(page_id, buffer_idx);
        self.lru.add(buffer_idx);

        // Pin the buffer
        let buffer = unsafe { &*self.buffers.add(buffer_idx) };
        buffer.pin();

        Ok(&mut self.page_data[buffer_idx])
    }

    /// Marks a page as dirty (modified)
    ///
    /// # Arguments
    /// * `page_id` - The PageId to mark dirty
    pub fn mark_dirty(&mut self, page_id: PageId) {
        if let Some(buffer_idx) = self.lookup(page_id) {
            let buffer = unsafe { &*self.buffers.add(buffer_idx) };
            buffer.set_dirty();
        }
    }

    /// Releases a pin on a page
    ///
    /// # Arguments
    /// * `page_id` - The PageId to unpin
    ///
    /// # Returns
    /// * `Ok(())` - Successfully unpinned
    /// * `Err(BufferError)` - Page not found
    pub fn unpin_page(&mut self, page_id: PageId) -> Result<(), BufferError> {
        if let Some(buffer_idx) = self.lookup(page_id) {
            let buffer = unsafe { &*self.buffers.add(buffer_idx) };
            buffer.unpin();
            return Ok(());
        }
        Err(BufferError::PageNotFound(page_id))
    }

    /// Flushes all dirty pages to disk
    pub fn flush_all(&mut self) -> Result<(), BufferError> {
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

    /// Returns the current number of buffers in the pool
    #[inline]
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }
}

impl Drop for BufferMgr {
    fn drop(&mut self) {
        // Flush all dirty pages on drop
        let _ = self.flush_all();

        unsafe {
            // Free hash table entries
            for i in 0..self.buffer_size {
                let mut entry_ptr = *self.buf_hash_table.add(i);
                while !entry_ptr.is_null() {
                    let next = (*entry_ptr).next;
                    let entry_layout = alloc::Layout::new::<HashEntry>();
                    alloc::dealloc(entry_ptr as *mut u8, entry_layout);
                    entry_ptr = next;
                }
            }

            // Free hash table
            let hash_layout = alloc::Layout::from_size_align_unchecked(
                mem::size_of::<*mut HashEntry>() * self.buffer_size,
                mem::align_of::<*mut HashEntry>(),
            );
            alloc::dealloc(self.buf_hash_table as *mut u8, hash_layout);

            // Free buffer array
            let buf_layout = alloc::Layout::from_size_align_unchecked(
                mem::size_of::<BufferDesc>() * self.buffer_size,
                mem::align_of::<BufferDesc>(),
            );
            alloc::dealloc(self.buffers as *mut u8, buf_layout);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock VfsInterface for testing
    struct MockVfs;

    impl VfsInterface for MockVfs {
        fn create_dir(&self, _path: &str) -> crate::vfs::VfsResult<()> {
            Ok(())
        }
        fn remove_dir(&self, _path: &str) -> crate::vfs::VfsResult<()> {
            Ok(())
        }
        fn create_file(
            &self,
            _path: &str,
        ) -> crate::vfs::VfsResult<Box<dyn crate::vfs::FileHandle>> {
            Ok(Box::new(MockFileHandle))
        }
        fn open_file(&self, _path: &str) -> crate::vfs::VfsResult<Box<dyn crate::vfs::FileHandle>> {
            Ok(Box::new(MockFileHandle))
        }
        fn remove_file(&self, _path: &str) -> crate::vfs::VfsResult<()> {
            Ok(())
        }
        fn truncate(&self, _path: &str, _length: u64) -> crate::vfs::VfsResult<()> {
            Ok(())
        }
        fn pread(
            &self,
            _path: &str,
            _buf: &mut [u8],
            _offset: u64,
        ) -> crate::vfs::VfsResult<usize> {
            Ok(0)
        }
        fn pwrite(&self, _path: &str, _buf: &[u8], _offset: u64) -> crate::vfs::VfsResult<usize> {
            Ok(0)
        }
    }

    struct MockFileHandle;

    impl crate::vfs::FileHandle for MockFileHandle {
        fn read(&mut self, _buf: &mut [u8]) -> crate::vfs::VfsResult<usize> {
            Ok(0)
        }
        fn write(&mut self, _buf: &[u8]) -> crate::vfs::VfsResult<usize> {
            Ok(0)
        }
        fn pread(&self, _buf: &mut [u8], _offset: u64) -> crate::vfs::VfsResult<usize> {
            Ok(0)
        }
        fn pwrite(&self, _buf: &[u8], _offset: u64) -> crate::vfs::VfsResult<usize> {
            Ok(0)
        }
        fn truncate(&self, _length: u64) -> crate::vfs::VfsResult<()> {
            Ok(())
        }
        fn close(self: Box<Self>) -> crate::vfs::VfsResult<()> {
            Ok(())
        }
    }

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
        desc.unpin();
    }

    #[test]
    fn test_buffer_mgr_init() {
        let vfs: Arc<dyn VfsInterface> = Arc::new(MockVfs);
        let mgr = BufferMgr::init(8, vfs, PathBuf::from("/tmp/test"));
        assert_eq!(mgr.buffer_size(), 8);
    }

    // Integration tests for Buffer Pool functionality

    #[test]
    fn test_lookup_returns_none_for_unknown_page() {
        let vfs: Arc<dyn VfsInterface> = Arc::new(MockVfs);
        let mgr = BufferMgr::init(8, vfs, PathBuf::from("/tmp/test"));
        assert_eq!(mgr.lookup(99999), None);
    }

    #[test]
    fn test_mark_dirty_nonexistent_page() {
        let vfs: Arc<dyn VfsInterface> = Arc::new(MockVfs);
        let mut mgr = BufferMgr::init(8, vfs, PathBuf::from("/tmp/test"));
        // Should not panic, just no-op
        mgr.mark_dirty(99999);
    }

    #[test]
    fn test_unpin_page_nonexistent_page() {
        let vfs: Arc<dyn VfsInterface> = Arc::new(MockVfs);
        let mut mgr = BufferMgr::init(8, vfs, PathBuf::from("/tmp/test"));
        let result = mgr.unpin_page(99999);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), BufferError::PageNotFound(99999));
    }

    #[test]
    fn test_buffer_desc_state_initial_values() {
        let desc = BufferDesc::new();
        assert_eq!(desc.pin_count(), 0);
        assert!(!desc.is_dirty());
        assert!(desc.can_evict());
    }

    #[test]
    fn test_buffer_desc_dirty_set_clear() {
        let desc = BufferDesc::new();
        assert!(!desc.is_dirty());
        desc.set_dirty();
        assert!(desc.is_dirty());
        desc.clear_dirty();
        assert!(!desc.is_dirty());
    }

    #[test]
    fn test_buffer_desc_pin_unpin() {
        let desc = BufferDesc::new();
        assert_eq!(desc.pin_count(), 0);
        assert!(desc.can_evict());

        desc.pin();
        assert_eq!(desc.pin_count(), 1);
        assert!(!desc.can_evict());

        desc.pin();
        assert_eq!(desc.pin_count(), 2);

        desc.unpin();
        assert_eq!(desc.pin_count(), 1);
        assert!(!desc.can_evict());

        desc.unpin();
        assert_eq!(desc.pin_count(), 0);
        assert!(desc.can_evict());
    }
}
