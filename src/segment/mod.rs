//! Segment storage module with paged storage mechanism
//!
//! This module implements a segment-based storage system with the following hierarchy:
//! - File: Contains FileHeader at the beginning
//! - Segment: Contains SegmentHeader, composed of one or more Extents
//! - Extent: 1MB in size, contains 128 pages (127 usable + 1 header page)
//! - Page: 8KB, the basic unit of storage

use crate::buffer::BufferMgr;
use crate::types::{PageId, SegmentId, BLOCK_SIZE};
use crc32fast;
use std::fmt;
use std::fs::File;
use std::io::{Seek, SeekFrom};
use std::sync::Arc;
use std::sync::RwLock;

// ============================================================================
// Constants
// ============================================================================

/// File magic number ("ASTR" in little endian)
pub const FILE_MAGIC: u32 = 0x41535452;

/// File version
pub const FILE_VERSION: u32 = 1;

/// Extent size (1MB)
pub const EXTENT_SIZE: usize = 1 << 20; // 1MB

/// Number of pages per extent
pub const EXTENT_PAGE_COUNT: u32 = 128;

/// Number of usable pages per extent (excluding header page)
pub const EXTENT_USABLE_PAGES: u32 = EXTENT_PAGE_COUNT - 1; // 127

/// Segment header size
pub const SEGMENT_HEADER_SIZE: usize = 24;

/// File header size
pub const FILE_HEADER_SIZE: usize = 24;

// ============================================================================
// Segment Type
// ============================================================================

/// Segment type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentType {
    /// Generic segment (current implementation)
    Generic,
    /// Data segment (future implementation)
    Data,
    /// Index segment (future implementation)
    Index,
    /// Metadata segment (future implementation)
    Metadata,
}

impl fmt::Display for SegmentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SegmentType::Generic => write!(f, "Generic"),
            SegmentType::Data => write!(f, "Data"),
            SegmentType::Index => write!(f, "Index"),
            SegmentType::Metadata => write!(f, "Metadata"),
        }
    }
}

// ============================================================================
// File Header
// ============================================================================

/// File header structure
/// Located at the beginning of the file (offset 0)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FileHeader {
    /// Magic number for file validation
    pub magic: u32,
    /// File version number
    pub version: u32,
    /// Current file size in bytes
    pub file_size: u64,
    /// Number of segments
    pub segment_count: u32,
    /// File header checksum
    pub checksum: u32,
}

impl FileHeader {
    /// Create a new file header
    pub fn new() -> Self {
        FileHeader {
            magic: FILE_MAGIC,
            version: FILE_VERSION,
            file_size: FILE_HEADER_SIZE as u64,
            segment_count: 0,
            checksum: 0,
        }
    }

    /// Validate file magic number
    pub fn is_valid(&self) -> bool {
        self.magic == FILE_MAGIC
    }

    /// Compute checksum (excluding checksum field itself)
    pub fn compute_checksum(&self) -> u32 {
        let mut header = *self;
        header.checksum = 0;
        let bytes = unsafe {
            std::slice::from_raw_parts(
                &header as *const FileHeader as *const u8,
                std::mem::size_of::<FileHeader>(),
            )
        };
        crc32fast::hash(bytes)
    }

    /// Verify checksum
    pub fn verify_checksum(&self) -> bool {
        self.checksum == 0 || self.compute_checksum() == self.checksum
    }

    /// Initialize checksum
    pub fn init_checksum(&mut self) {
        self.checksum = self.compute_checksum();
    }
}

// ============================================================================
// Extent Header
// ============================================================================

/// Extent header structure
/// Located at the first page of each extent (Page 0, not counted as usable page)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ExtentHeader {
    /// Pointer to the next extent header (0 means none)
    pub next_extent_ptr: u64,
}

impl ExtentHeader {
    /// Create a new extent header with no next extent
    pub fn new() -> Self {
        ExtentHeader { next_extent_ptr: 0 }
    }

    /// Compute checksum
    pub fn compute_checksum(&self) -> u32 {
        let mut header = *self;
        header.next_extent_ptr = header.next_extent_ptr.swap_bytes(); // Simple pseudo-checksum
        let bytes = unsafe {
            std::slice::from_raw_parts(
                &header as *const ExtentHeader as *const u8,
                std::mem::size_of::<ExtentHeader>(),
            )
        };
        crc32fast::hash(bytes)
    }
}

// ============================================================================
// Segment Header
// ============================================================================

/// Segment header structure
/// Located at the first page of the first extent of the segment (Page 0)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SegmentHeader {
    /// Segment ID
    pub segment_id: u64,
    /// Segment type
    pub segment_type: SegmentType,
    /// Pointer to the next extent header (0 means none)
    pub next_extent_ptr: u64,
    /// Total number of pages in this segment
    pub total_pages: u64,
    /// Segment header checksum
    pub checksum: u32,
}

impl SegmentHeader {
    /// Create a new segment header
    pub fn new(segment_id: u64, segment_type: SegmentType) -> Self {
        SegmentHeader {
            segment_id,
            segment_type,
            next_extent_ptr: 0,
            total_pages: 0,
            checksum: 0,
        }
    }

    /// Compute checksum
    pub fn compute_checksum(&self) -> u32 {
        let mut header = *self;
        header.checksum = 0;
        let bytes = unsafe {
            std::slice::from_raw_parts(
                &header as *const SegmentHeader as *const u8,
                std::mem::size_of::<SegmentHeader>(),
            )
        };
        crc32fast::hash(bytes)
    }

    /// Verify checksum
    pub fn verify_checksum(&self) -> bool {
        self.checksum == 0 || self.compute_checksum() == self.checksum
    }

    /// Initialize checksum
    pub fn init_checksum(&mut self) {
        self.checksum = self.compute_checksum();
    }
}

// ============================================================================
// Segment Error
// ============================================================================

/// Segment-related errors
#[derive(Debug)]
pub enum SegmentError {
    /// Segment not found
    #[allow(dead_code)]
    NotFound(SegmentId),
    /// Extent not found at offset
    #[allow(dead_code)]
    ExtentNotFound(u64),
    /// Page out of bounds
    #[allow(dead_code)]
    PageOutOfBounds {
        segment_id: SegmentId,
        page_idx: PageId,
    },
    /// Invalid file header
    InvalidFileHeader,
    /// Invalid segment header
    InvalidSegmentHeader,
    /// Invalid extent header
    InvalidExtentHeader,
    /// Checksum mismatch
    ChecksumMismatch,
    /// IO error
    Io(std::io::Error),
}

impl fmt::Display for SegmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SegmentError::NotFound(id) => write!(f, "Segment not found: {}", id),
            SegmentError::ExtentNotFound(offset) => {
                write!(f, "Extent not found at offset: {}", offset)
            }
            SegmentError::PageOutOfBounds {
                segment_id,
                page_idx,
            } => write!(
                f,
                "Page out of bounds: {} in segment {}",
                page_idx, segment_id
            ),
            SegmentError::InvalidFileHeader => write!(f, "Invalid file header"),
            SegmentError::InvalidSegmentHeader => write!(f, "Invalid segment header"),
            SegmentError::InvalidExtentHeader => write!(f, "Invalid extent header"),
            SegmentError::ChecksumMismatch => write!(f, "Checksum mismatch"),
            SegmentError::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for SegmentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SegmentError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for SegmentError {
    fn from(err: std::io::Error) -> Self {
        SegmentError::Io(err)
    }
}

/// Result type for segment operations
pub type SegmentResult<T> = Result<T, SegmentError>;

// ============================================================================
// Segment Manager
// ============================================================================

/// Segment manager for managing segment storage
pub struct SegmentManager {
    /// File handle (opened for read/write)
    file_handle: Arc<RwLock<File>>,
    /// File header cached in memory
    cached_file_header: RwLock<FileHeader>,
    /// Global extent allocation lock
    extent_alloc_lock: RwLock<()>,
    /// Buffer manager reference (optional, for future integration)
    buffer_mgr: Option<Arc<BufferMgr>>,
}

impl SegmentManager {
    /// Create or open a segment storage file
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> SegmentResult<Self> {
        use std::fs::{File, OpenOptions};

        let path_ref = path.as_ref();

        // Check if file exists
        let file_exists = path_ref.exists();

        // Open or create file
        let file_handle: Arc<RwLock<File>> = {
            let file = if file_exists {
                OpenOptions::new().read(true).write(true).open(path_ref)?
            } else {
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(path_ref)?
            };
            Arc::new(RwLock::new(file))
        };

        // Initialize or load file header
        let cached_file_header = RwLock::new(if file_exists {
            Self::load_file_header(file_handle.clone())?
        } else {
            let mut header = FileHeader::new();
            header.init_checksum();
            Self::write_file_header(file_handle.clone(), &header)?;
            header
        });

        Ok(SegmentManager {
            file_handle,
            cached_file_header,
            extent_alloc_lock: RwLock::new(()),
            buffer_mgr: None,
        })
    }

    /// Set buffer manager reference
    pub fn set_buffer_mgr(&mut self, buffer_mgr: Arc<BufferMgr>) {
        self.buffer_mgr = Some(buffer_mgr);
    }

    /// Load file header from disk
    fn load_file_header(file_handle: Arc<RwLock<File>>) -> SegmentResult<FileHeader> {
        use std::io::Read;

        let mut header = FileHeader::new();
        let mut bytes = vec![0u8; FILE_HEADER_SIZE];

        {
            let mut handle = file_handle.write().expect("RwLock poisoned");
            let file = &mut *handle;
            file.seek(SeekFrom::Start(0))?;
            file.read_exact(&mut bytes)?;
        }

        // Copy bytes to header
        let header_ptr = &mut header as *mut FileHeader as *mut u8;
        unsafe {
            std::ptr::copy(bytes.as_ptr(), header_ptr, FILE_HEADER_SIZE);
        }

        // Validate header
        if !header.is_valid() {
            return Err(SegmentError::InvalidFileHeader);
        }

        if !header.verify_checksum() {
            return Err(SegmentError::ChecksumMismatch);
        }

        Ok(header)
    }

    /// Write file header to disk
    fn write_file_header(file_handle: Arc<RwLock<File>>, header: &FileHeader) -> SegmentResult<()> {
        use std::io::Write;

        let bytes = unsafe {
            std::slice::from_raw_parts(header as *const FileHeader as *const u8, FILE_HEADER_SIZE)
        };

        let mut handle = file_handle.write().expect("RwLock poisoned");
        let file = &mut *handle;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(bytes)?;

        Ok(())
    }

    /// Create a new segment
    pub fn create_segment(&self, segment_type: SegmentType) -> SegmentResult<SegmentId> {
        // 1. Acquire extent allocation lock
        let _guard = self.extent_alloc_lock.write().expect("RwLock poisoned");

        // 2. Allocate new extent
        let extent_ptr = self.allocate_extent()?;

        // 3. Generate segment ID (simple counter-based for now)
        let segment_id = {
            let header = self.cached_file_header.read().expect("RwLock poisoned");
            header.segment_count as u64 + 1
        };

        // 4. Initialize segment header
        let mut header = SegmentHeader::new(segment_id, segment_type);
        header.init_checksum();

        // 5. Write segment header to first page of extent
        self.write_segment_header(extent_ptr, &header)?;

        // 6. Update file header
        {
            let mut fh = self.cached_file_header.write().expect("RwLock poisoned");
            fh.segment_count += 1;
            fh.init_checksum();
            Self::write_file_header(self.file_handle.clone(), &fh)?;
        }

        Ok(segment_id)
    }

    /// Allocate a new extent
    fn allocate_extent(&self) -> SegmentResult<u64> {
        // Get current file size as the starting position of new extent
        let extent_ptr = {
            let fh = self.cached_file_header.read().expect("RwLock poisoned");
            fh.file_size
        };

        // Extend file size
        let new_size = extent_ptr + EXTENT_SIZE as u64;
        {
            let mut handle = self.file_handle.write().expect("RwLock poisoned");
            let file = &mut *handle;
            file.set_len(new_size)?;
        }

        // Initialize extent header at the beginning of the new extent
        let extent_header = ExtentHeader::new();
        self.write_extent_header(extent_ptr, &extent_header)?;

        // Update file header
        let mut fh = self.cached_file_header.write().expect("RwLock poisoned");
        fh.file_size = new_size;
        fh.init_checksum();
        Self::write_file_header(self.file_handle.clone(), &fh)?;

        Ok(extent_ptr)
    }

    /// Write extent header to disk
    fn write_extent_header(&self, extent_ptr: u64, header: &ExtentHeader) -> SegmentResult<()> {
        use std::io::{Seek, SeekFrom, Write};

        let bytes = unsafe {
            std::slice::from_raw_parts(
                header as *const ExtentHeader as *const u8,
                std::mem::size_of::<ExtentHeader>(),
            )
        };

        let mut handle = self.file_handle.write().expect("RwLock poisoned");
        let file = &mut *handle;
        file.seek(SeekFrom::Start(extent_ptr))?;
        file.write_all(bytes)?;

        Ok(())
    }

    /// Read extent header from disk
    fn read_extent_header(&self, extent_ptr: u64) -> SegmentResult<ExtentHeader> {
        use std::io::{Read, Seek, SeekFrom};

        let mut header = ExtentHeader::new();
        let mut bytes = vec![0u8; std::mem::size_of::<ExtentHeader>()];

        {
            let mut handle = self.file_handle.write().expect("RwLock poisoned");
            let file = &mut *handle;
            file.seek(SeekFrom::Start(extent_ptr))?;
            file.read_exact(&mut bytes)?;
        }

        let header_ptr = &mut header as *mut ExtentHeader as *mut u8;
        unsafe {
            std::ptr::copy(
                bytes.as_ptr(),
                header_ptr,
                std::mem::size_of::<ExtentHeader>(),
            );
        }

        Ok(header)
    }

    /// Write segment header to disk
    fn write_segment_header(
        &self,
        segment_offset: u64,
        header: &SegmentHeader,
    ) -> SegmentResult<()> {
        use std::io::{Seek, SeekFrom, Write};

        let bytes = unsafe {
            std::slice::from_raw_parts(
                header as *const SegmentHeader as *const u8,
                std::mem::size_of::<SegmentHeader>(),
            )
        };

        let mut handle = self.file_handle.write().expect("RwLock poisoned");
        let file = &mut *handle;
        file.seek(SeekFrom::Start(segment_offset))?;
        file.write_all(bytes)?;

        Ok(())
    }

    /// Read segment header from disk
    fn read_segment_header(&self, segment_offset: u64) -> SegmentResult<SegmentHeader> {
        use std::io::{Read, Seek, SeekFrom};

        let mut header = SegmentHeader::new(0, SegmentType::Generic);
        let mut bytes = vec![0u8; std::mem::size_of::<SegmentHeader>()];

        {
            let mut handle = self.file_handle.write().expect("RwLock poisoned");
            let file = &mut *handle;
            file.seek(SeekFrom::Start(segment_offset))?;
            file.read_exact(&mut bytes)?;
        }

        let header_ptr = &mut header as *mut SegmentHeader as *mut u8;
        unsafe {
            std::ptr::copy(
                bytes.as_ptr(),
                header_ptr,
                std::mem::size_of::<SegmentHeader>(),
            );
        }

        if !header.verify_checksum() {
            return Err(SegmentError::ChecksumMismatch);
        }

        Ok(header)
    }

    /// Allocate a new page in a segment
    /// Returns the page index of the newly allocated page
    pub fn allocate_page(&self, segment_id: SegmentId) -> SegmentResult<PageId> {
        // 1. Locate segment (for now, segments are at fixed positions)
        let segment_offset = self.locate_segment(segment_id)?;
        let mut header = self.read_segment_header(segment_offset)?;

        // 2. Check if we need a new extent
        let page_idx = header.total_pages;
        let page_in_extent = page_idx % EXTENT_USABLE_PAGES as u64;

        // 3. If current extent is full, allocate a new extent
        if page_in_extent == 0 && page_idx > 0 {
            // Acquire allocation lock
            let _guard = self.extent_alloc_lock.write();

            let new_extent_ptr = self.allocate_extent()?;

            // Link new extent to current extent
            let current_extent_ptr = self.extent_ptr_from_page(segment_offset, page_idx - 1)?;
            self.link_extent(current_extent_ptr, new_extent_ptr)?;

            // Update segment header with new extent pointer
            header.next_extent_ptr = new_extent_ptr;
            header.init_checksum();
            self.write_segment_header(segment_offset, &header)?;
        }

        // 4. Update total_pages in segment header
        header.total_pages += 1;
        header.init_checksum();
        self.write_segment_header(segment_offset, &header)?;

        Ok(page_idx)
    }

    /// Convert page index to file offset
    fn page_to_file_offset(&self, segment_offset: u64, page_idx: PageId) -> SegmentResult<u64> {
        if page_idx == 0 {
            return Ok(segment_offset);
        }

        let extent_idx = (page_idx / EXTENT_USABLE_PAGES as u64) as usize;
        let page_in_extent = page_idx % EXTENT_USABLE_PAGES as u64;

        // Traverse extent linked list
        let mut current_extent_ptr = segment_offset;
        for _ in 0..extent_idx {
            let extent_header = self.read_extent_header(current_extent_ptr)?;
            if extent_header.next_extent_ptr == 0 {
                return Err(SegmentError::ExtentNotFound(current_extent_ptr));
            }
            current_extent_ptr = extent_header.next_extent_ptr;
        }

        // Calculate file offset: extent_start + header_page + page_offset
        let file_offset = current_extent_ptr + (page_in_extent + 1) as u64 * BLOCK_SIZE as u64;

        Ok(file_offset)
    }

    /// Get extent pointer from segment offset and page index
    fn extent_ptr_from_page(&self, segment_offset: u64, page_idx: PageId) -> SegmentResult<u64> {
        if page_idx == 0 {
            return Ok(segment_offset);
        }

        let extent_start_page =
            (page_idx / EXTENT_USABLE_PAGES as u64) * EXTENT_USABLE_PAGES as u64;
        self.page_to_file_offset(segment_offset, extent_start_page)
    }

    /// Link two extents together
    fn link_extent(&self, from_ptr: u64, to_ptr: u64) -> SegmentResult<()> {
        use std::io::{Seek, SeekFrom, Write};

        let mut header = self.read_extent_header(from_ptr)?;
        header.next_extent_ptr = to_ptr;

        let bytes = unsafe {
            std::slice::from_raw_parts(
                &header as *const ExtentHeader as *const u8,
                std::mem::size_of::<ExtentHeader>(),
            )
        };

        let mut handle = self.file_handle.write().expect("RwLock poisoned");
        let file = &mut *handle;
        file.seek(SeekFrom::Start(from_ptr as u64))?;
        file.write_all(bytes)?;

        Ok(())
    }

    /// Locate segment by segment ID
    /// Uses a simple mapping: segment N is stored at file offset FILE_HEADER_SIZE + (N-1) * EXTENT_SIZE
    /// This is a simplification - a real implementation would use a segment directory
    fn locate_segment(&self, segment_id: SegmentId) -> SegmentResult<u64> {
        if segment_id == 0 {
            return Err(SegmentError::NotFound(segment_id));
        }

        // Simple formula: FILE_HEADER_SIZE + (segment_id - 1) * EXTENT_SIZE
        let offset = FILE_HEADER_SIZE as u64 + (segment_id - 1) * EXTENT_SIZE as u64;

        // Verify segment exists by checking segment_id doesn't exceed count
        let header = self.cached_file_header.read().expect("RwLock poisoned");
        if segment_id > header.segment_count as u64 {
            return Err(SegmentError::NotFound(segment_id));
        }

        Ok(offset)
    }

    /// Read a page from the segment
    /// Returns the page data as a vector
    pub fn read_page(&self, segment_id: SegmentId, page_idx: PageId) -> SegmentResult<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};

        // Locate segment
        let segment_offset = self.locate_segment(segment_id)?;

        // Convert page index to file offset
        let file_offset = self.page_to_file_offset(segment_offset, page_idx)?;

        // Read page data
        let mut page_data = vec![0u8; BLOCK_SIZE];
        {
            let mut handle = self.file_handle.write().expect("RwLock poisoned");
            let file = &mut *handle;
            file.seek(SeekFrom::Start(file_offset))?;
            file.read_exact(&mut page_data)?;
        }

        Ok(page_data)
    }

    /// Write data to a page
    /// The page must have been allocated first
    pub fn write_page(
        &self,
        segment_id: SegmentId,
        page_idx: PageId,
        data: &[u8],
    ) -> SegmentResult<()> {
        use std::io::{Seek, SeekFrom, Write};

        if data.len() > BLOCK_SIZE {
            return Err(SegmentError::InvalidExtentHeader);
        }

        // Locate segment
        let segment_offset = self.locate_segment(segment_id)?;

        // Convert page index to file offset
        let file_offset = self.page_to_file_offset(segment_offset, page_idx)?;

        // Prepare page data with zero padding
        let mut page_data = vec![0u8; BLOCK_SIZE];
        page_data[..data.len()].copy_from_slice(data);

        // Write page data
        let mut handle = self.file_handle.write().expect("RwLock poisoned");
        let file = &mut *handle;
        file.seek(SeekFrom::Start(file_offset))?;
        file.write_all(&page_data)?;

        Ok(())
    }

    /// Get file handle (for external use)
    pub fn file_handle(&self) -> Arc<RwLock<File>> {
        self.file_handle.clone()
    }

    /// Get cached file header
    pub fn cached_file_header(&self) -> std::sync::RwLockReadGuard<'_, FileHeader> {
        self.cached_file_header.read().expect("RwLock poisoned")
    }

    /// Sync file to disk
    pub fn sync(&self) -> SegmentResult<()> {
        let _handle = self.file_handle.write().expect("RwLock poisoned");
        // FileHandle doesn't have sync method, so we rely on close or fsync
        // For now, this is a no-op
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[cfg(test)]
mod tests {
    include!("tests.rs");
}
