//! Tablespace and Segment storage implementation

pub mod segment;

use crate::types::Timestamp;
use crc32fast;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

// Constants
pub const FILE_MAGIC: u32 = 0x41535452; // "ASTR"
pub const FILE_VERSION: u32 = 1;
pub const EXTENT_SIZE: usize = 1 << 20; // 1 MB
pub const EXTENT_PAGE_COUNT: u32 = 128;
pub const EXTENT_USABLE_PAGES: u32 = EXTENT_PAGE_COUNT - 1; // 127
pub const DEFAULT_INITIAL_FILE_SIZE: usize = 16 * 1024 * 1024;
pub const DEFAULT_AUTO_EXTEND_SIZE: usize = 16 * 1024 * 1024;
pub const FILE_HEADER_SIZE: usize = 40;
pub const EXTENT_HEADER_SIZE: usize = 56;

// Free Extent Structure
#[derive(Debug, Clone, Copy)]
pub struct FreeExtent {
    pub file_id: u32,
    pub extent_offset: u64,
    pub free_pages: u32,
}

impl FreeExtent {
    #[inline]
    pub fn new(file_id: u32, extent_offset: u64, free_pages: u32) -> Self {
        Self {
            file_id,
            extent_offset,
            free_pages,
        }
    }
}

// Free Extent List
#[derive(Debug)]
pub struct FreeExtentList {
    free_extents: Vec<FreeExtent>,
}

impl FreeExtentList {
    #[inline]
    pub fn new() -> Self {
        Self {
            free_extents: Vec::new(),
        }
    }

    pub fn get_extent(&mut self, min_pages: u32) -> Option<FreeExtent> {
        for (i, extent) in self.free_extents.iter().enumerate() {
            if extent.free_pages >= min_pages {
                return Some(self.free_extents.remove(i));
            }
        }
        None
    }

    #[inline]
    pub fn return_extent(&mut self, extent: FreeExtent) {
        self.free_extents.push(extent);
        self.free_extents
            .sort_by(|a, b| b.free_pages.cmp(&a.free_pages));
    }

    #[inline]
    pub fn add_extent(&mut self, file_id: u32, extent_offset: u64, free_pages: u32) {
        self.return_extent(FreeExtent::new(file_id, extent_offset, free_pages));
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.free_extents.is_empty()
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.free_extents.len()
    }
}

impl Default for FreeExtentList {
    fn default() -> Self {
        Self::new()
    }
}

// File Header (40 bytes)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FileHeader {
    pub magic: u32,
    pub version: u32,
    pub file_id: u32,
    pub reserved: u32,
    pub tablespace_id: u64,
    pub file_size: u64,
    pub extent_count: u32,
    pub free_pages: u32,
    pub flags: u32,
    pub checksum: u32,
}

impl FileHeader {
    #[inline]
    pub fn new(tablespace_id: u64, file_id: u32) -> Self {
        Self {
            magic: FILE_MAGIC,
            version: FILE_VERSION,
            file_id,
            reserved: 0,
            tablespace_id,
            file_size: FILE_HEADER_SIZE as u64,
            extent_count: 0,
            free_pages: 0,
            flags: 0,
            checksum: 0,
        }
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.magic == FILE_MAGIC
    }

    pub fn compute_checksum(&self) -> u32 {
        let mut header = *self;
        header.checksum = 0;
        let bytes = unsafe {
            std::slice::from_raw_parts(&header as *const FileHeader as *const u8, FILE_HEADER_SIZE)
        };
        crc32fast::hash(bytes)
    }

    #[inline]
    pub fn verify_checksum(&self) -> bool {
        self.checksum == 0 || self.compute_checksum() == self.checksum
    }

    #[inline]
    pub fn init_checksum(&mut self) {
        self.checksum = self.compute_checksum();
    }
}

// Extent Header (56 bytes)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ExtentHeader {
    pub file_id: u32,
    pub reserved1: u32,
    pub tablespace_id: u64,
    pub extent_offset: u64,
    pub page_count: u32,
    pub free_pages: u32,
    pub page_bitmap: [u8; 16],
    pub checksum: u32,
}

impl ExtentHeader {
    pub const PAGE_COUNT: u32 = 128;
    pub const USABLE_PAGES: u32 = 127;

    #[inline]
    pub fn new(tablespace_id: u64, file_id: u32, extent_offset: u64) -> Self {
        Self {
            file_id,
            reserved1: 0,
            tablespace_id,
            extent_offset,
            page_count: Self::PAGE_COUNT,
            free_pages: Self::USABLE_PAGES,
            page_bitmap: [0xFF; 16],
            checksum: 0,
        }
    }

    pub fn allocate_page(&mut self) -> Option<u32> {
        for i in 0..Self::USABLE_PAGES {
            let byte_idx = (i / 8) as usize;
            let bit_idx = i % 8;
            if self.page_bitmap[byte_idx] & (1 << bit_idx) != 0 {
                self.page_bitmap[byte_idx] &= !(1 << bit_idx);
                self.free_pages -= 1;
                return Some(i);
            }
        }
        None
    }

    #[inline]
    pub fn free_page(&mut self, page_idx: u32) {
        let byte_idx = (page_idx / 8) as usize;
        let bit_idx = page_idx % 8;
        self.page_bitmap[byte_idx] |= 1 << bit_idx;
        self.free_pages += 1;
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.free_pages == 0
    }
    #[inline]
    pub fn has_free_pages(&self) -> bool {
        self.free_pages > 0
    }

    pub fn compute_checksum(&self) -> u32 {
        let mut header = *self;
        header.checksum = 0;
        let bytes = unsafe {
            std::slice::from_raw_parts(
                &header as *const ExtentHeader as *const u8,
                EXTENT_HEADER_SIZE,
            )
        };
        crc32fast::hash(bytes)
    }

    #[inline]
    pub fn verify_checksum(&self) -> bool {
        self.checksum == 0 || self.compute_checksum() == self.checksum
    }

    #[inline]
    pub fn init_checksum(&mut self) {
        self.checksum = self.compute_checksum();
    }
}

// Tablespace Configuration
#[derive(Debug, Clone)]
pub struct TablespaceConfig {
    pub name: String,
    pub initial_file_size: usize,
    pub initial_files: u32,
    pub auto_extend_size: usize,
    pub max_size: usize,
}

impl Default for TablespaceConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            initial_file_size: DEFAULT_INITIAL_FILE_SIZE,
            initial_files: 1,
            auto_extend_size: DEFAULT_AUTO_EXTEND_SIZE,
            max_size: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TablespaceStatus {
    Creating,
    Active,
    Dropping,
    Recovering,
}

// Tablespace Metadata
#[derive(Debug, Clone)]
pub struct TablespaceMeta {
    pub tablespace_id: u64,
    pub name: String,
    pub status: TablespaceStatus,
    pub file_path: PathBuf,
    pub file_size: u64,
    pub segment_count: u32,
    pub created_at: Timestamp,
    pub modified_at: Timestamp,
}

impl TablespaceMeta {
    pub fn new(tablespace_id: u64, name: String, file_path: PathBuf) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self {
            tablespace_id,
            name,
            status: TablespaceStatus::Creating,
            file_path,
            file_size: FILE_HEADER_SIZE as u64,
            segment_count: 0,
            created_at: now,
            modified_at: now,
        }
    }
}

// Tablespace Errors
#[derive(Debug)]
pub enum TablespaceError {
    NotFound(String),
    FileNotFound(PathBuf),
    InvalidFileHeader,
    InvalidExtentHeader,
    ChecksumMismatch,
    NoFreeExtent,
    NoSpace,
    Io(std::io::Error),
    InvalidArgument(String),
}

impl fmt::Display for TablespaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TablespaceError::NotFound(s) => write!(f, "Tablespace not found: {}", s),
            TablespaceError::FileNotFound(p) => write!(f, "File not found: {:?}", p),
            TablespaceError::InvalidFileHeader => write!(f, "Invalid file header"),
            TablespaceError::InvalidExtentHeader => write!(f, "Invalid extent header"),
            TablespaceError::ChecksumMismatch => write!(f, "Checksum mismatch"),
            TablespaceError::NoFreeExtent => write!(f, "No free extent available"),
            TablespaceError::NoSpace => write!(f, "No space left in tablespace"),
            TablespaceError::Io(e) => write!(f, "I/O error: {}", e),
            TablespaceError::InvalidArgument(s) => write!(f, "Invalid argument: {}", s),
        }
    }
}

impl std::error::Error for TablespaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TablespaceError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for TablespaceError {
    #[inline]
    fn from(err: std::io::Error) -> Self {
        TablespaceError::Io(err)
    }
}

pub type TablespaceResult<T> = Result<T, TablespaceError>;

// File Utilities
fn read_file_header(file: &mut File) -> TablespaceResult<FileHeader> {
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = vec![0u8; FILE_HEADER_SIZE];
    file.read_exact(&mut bytes)?;
    let header_ptr = &mut bytes[0] as *mut u8 as *mut FileHeader;
    let header = unsafe { *header_ptr };
    if !header.is_valid() {
        return Err(TablespaceError::InvalidFileHeader);
    }
    if !header.verify_checksum() {
        return Err(TablespaceError::ChecksumMismatch);
    }
    Ok(header)
}

fn write_file_header(file: &mut File, header: &FileHeader) -> TablespaceResult<()> {
    let bytes = unsafe {
        std::slice::from_raw_parts(header as *const FileHeader as *const u8, FILE_HEADER_SIZE)
    };
    file.seek(SeekFrom::Start(0))?;
    file.write_all(bytes)?;
    Ok(())
}

fn read_extent_header(file: &mut File, extent_offset: u64) -> TablespaceResult<ExtentHeader> {
    file.seek(SeekFrom::Start(extent_offset))?;
    let mut bytes = vec![0u8; EXTENT_HEADER_SIZE];
    file.read_exact(&mut bytes)?;
    let header_ptr = &mut bytes[0] as *mut u8 as *mut ExtentHeader;
    let header = unsafe { *header_ptr };
    if !header.verify_checksum() {
        return Err(TablespaceError::ChecksumMismatch);
    }
    Ok(header)
}

fn write_extent_header(
    file: &mut File,
    extent_offset: u64,
    header: &ExtentHeader,
) -> TablespaceResult<()> {
    let bytes = unsafe {
        std::slice::from_raw_parts(
            header as *const ExtentHeader as *const u8,
            EXTENT_HEADER_SIZE,
        )
    };
    file.seek(SeekFrom::Start(extent_offset))?;
    file.write_all(bytes)?;
    Ok(())
}

// Tablespace Manager
pub struct TablespaceManager {
    data_dir: PathBuf,
    tablespaces: RwLock<std::collections::HashMap<u64, Arc<RwLock<TablespaceMeta>>>>,
    free_lists: RwLock<std::collections::HashMap<u64, Arc<RwLock<FreeExtentList>>>>,
    name_to_id: RwLock<std::collections::HashMap<String, u64>>,
    file_lock: RwLock<()>,
}

impl TablespaceManager {
    #[inline]
    pub fn new<P: AsRef<Path>>(data_dir: P) -> Self {
        Self {
            data_dir: data_dir.as_ref().to_path_buf(),
            tablespaces: RwLock::new(std::collections::HashMap::new()),
            free_lists: RwLock::new(std::collections::HashMap::new()),
            name_to_id: RwLock::new(std::collections::HashMap::new()),
            file_lock: RwLock::new(()),
        }
    }

    pub fn create_tablespace(
        &self,
        name: &str,
        _config: TablespaceConfig,
    ) -> TablespaceResult<u64> {
        let _guard = self.file_lock.write().unwrap();

        // Check if name already exists
        {
            let name_to_id = self.name_to_id.read().unwrap();
            if name_to_id.contains_key(name) {
                return Err(TablespaceError::InvalidArgument(format!(
                    "Tablespace '{}' already exists",
                    name
                )));
            }
        }

        // Generate tablespace ID
        let tablespace_id = {
            let tablespaces = self.tablespaces.read().unwrap();
            (tablespaces.len() + 1) as u64
        };

        // Check ID already exists
        {
            let tablespaces = self.tablespaces.read().unwrap();
            if tablespaces.contains_key(&tablespace_id) {
                return Err(TablespaceError::InvalidArgument(format!(
                    "Tablespace ID {} exists",
                    tablespace_id
                )));
            }
        }

        // Create file
        let file_path = self.data_dir.join(format!("{}.tbl", name));
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&file_path)?;

        // Write file header
        let mut file_header = FileHeader::new(tablespace_id, 0);
        file_header.init_checksum();
        write_file_header(&mut file, &file_header)?;

        // Create metadata and free list
        let meta = TablespaceMeta::new(tablespace_id, name.to_string(), file_path);
        let free_list = Arc::new(RwLock::new(FreeExtentList::new()));

        // Store
        {
            let mut tablespaces = self.tablespaces.write().unwrap();
            tablespaces.insert(tablespace_id, Arc::new(RwLock::new(meta)));

            let mut free_lists = self.free_lists.write().unwrap();
            free_lists.insert(tablespace_id, free_list);

            let mut name_to_id = self.name_to_id.write().unwrap();
            name_to_id.insert(name.to_string(), tablespace_id);
        }

        Ok(tablespace_id)
    }

    pub fn open_tablespace(&self, name: &str) -> TablespaceResult<u64> {
        let name_to_id = self.name_to_id.read().unwrap();
        name_to_id
            .get(name)
            .copied()
            .ok_or_else(|| TablespaceError::NotFound(name.to_string()))
    }

    pub fn allocate_extent(&self, tablespace_id: u64) -> TablespaceResult<FreeExtent> {
        let _guard = self.file_lock.write().unwrap();

        // Try free list first
        {
            let mut free_lists = self.free_lists.write().unwrap();
            if let Some(free_list) = free_lists.get_mut(&tablespace_id) {
                let mut free_list = free_list.write().unwrap();
                if let Some(extent) = free_list.get_extent(ExtentHeader::USABLE_PAGES) {
                    return Ok(extent);
                }
            }
        }

        // Need to extend
        self.extend_tablespace(tablespace_id)
    }

    fn extend_tablespace(&self, tablespace_id: u64) -> TablespaceResult<FreeExtent> {
        let tablespaces = self.tablespaces.read().unwrap();
        let meta_ref = tablespaces
            .get(&tablespace_id)
            .ok_or_else(|| TablespaceError::NotFound(format!("{}", tablespace_id)))?;
        let meta = meta_ref.read().unwrap();

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&meta.file_path)?;

        let extent_offset = file.seek(SeekFrom::End(0))?;
        let new_size = extent_offset + EXTENT_SIZE as u64;
        file.set_len(new_size)?;

        // Write extent header
        let mut extent_header = ExtentHeader::new(tablespace_id, 0, extent_offset);
        extent_header.init_checksum();
        write_extent_header(&mut file, extent_offset, &extent_header)?;

        // Update file header
        let mut file_header = read_file_header(&mut file)?;
        file_header.file_size = new_size;
        file_header.extent_count += 1;
        file_header.free_pages += ExtentHeader::USABLE_PAGES;
        file_header.init_checksum();
        write_file_header(&mut file, &file_header)?;

        // Add to free list
        let free_extent = FreeExtent::new(0, extent_offset, ExtentHeader::USABLE_PAGES);
        let mut free_lists = self.free_lists.write().unwrap();
        if let Some(free_list) = free_lists.get_mut(&tablespace_id) {
            let mut free_list = free_list.write().unwrap();
            free_list.add_extent(0, extent_offset, ExtentHeader::USABLE_PAGES);
        }

        Ok(free_extent)
    }

    pub fn get_file(&self, tablespace_id: u64, file_id: u32) -> TablespaceResult<File> {
        let tablespaces = self.tablespaces.read().unwrap();
        let meta_ref = tablespaces
            .get(&tablespace_id)
            .ok_or_else(|| TablespaceError::NotFound(format!("{}", tablespace_id)))?;
        let meta = meta_ref.read().unwrap();

        if file_id > 0 {
            return Err(TablespaceError::FileNotFound(meta.file_path.clone()));
        }

        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&meta.file_path)
            .map_err(TablespaceError::Io)
    }

    pub fn list_tablespaces(&self) -> Vec<String> {
        let name_to_id = self.name_to_id.read().unwrap();
        name_to_id.keys().cloned().collect()
    }
}

// Tests
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::remove_file;
    use tempfile::TempDir;

    fn get_test_temp_dir() -> TempDir {
        tempfile::Builder::new()
            .prefix("aistore_test_")
            .tempdir()
            .unwrap()
    }

    #[test]
    fn test_free_extent_list() {
        let mut list = FreeExtentList::new();
        list.add_extent(0, 0, 100);
        list.add_extent(1, 0x100000, 50);
        list.add_extent(2, 0x200000, 127);
        assert_eq!(list.len(), 3);

        let extent = list.get_extent(100).unwrap();
        assert_eq!(extent.free_pages, 127);
        assert_eq!(extent.file_id, 2);

        let extent = list.get_extent(50).unwrap();
        assert_eq!(extent.free_pages, 100);
    }

    #[test]
    fn test_extent_header() {
        let mut header = ExtentHeader::new(1, 0, 0);
        assert!(header.has_free_pages());
        assert!(!header.is_full());

        for i in 0..10 {
            let page_idx = header.allocate_page().unwrap();
            assert_eq!(page_idx, i as u32);
        }
        assert_eq!(header.free_pages, 117);

        for _ in 10..127 {
            assert!(header.allocate_page().is_some());
        }
        assert!(header.is_full());
        assert!(header.allocate_page().is_none());

        header.free_page(50);
        assert_eq!(header.free_pages, 1);
    }

    #[test]
    fn test_file_header() {
        let mut header = FileHeader::new(1, 0);
        assert!(header.is_valid());
        header.init_checksum();
        assert!(header.verify_checksum());
    }

    #[test]
    fn test_tablespace_creation() {
        let temp_dir = get_test_temp_dir();
        let mgr = TablespaceManager::new(temp_dir.path());

        let ts_id = mgr
            .create_tablespace("test_tablespace", TablespaceConfig::default())
            .unwrap();
        assert_eq!(ts_id, 1);

        let names = mgr.list_tablespaces();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], "test_tablespace");

        let _ = remove_file(temp_dir.path().join("test_tablespace.tbl"));
    }
}
