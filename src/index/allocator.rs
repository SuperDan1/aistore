use crate::types::PageId;
use crate::vfs::VfsInterface;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

const ALLOCATOR_MAGIC: u32 = 0x49444150;
const ALLOCATOR_VERSION: u32 = 1;

#[derive(Debug)]
pub enum AllocatorError {
    IoError(std::io::Error),
    VfsError(crate::vfs::VfsError),
}

impl std::fmt::Display for AllocatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AllocatorError::IoError(e) => write!(f, "IO error: {}", e),
            AllocatorError::VfsError(e) => write!(f, "VFS error: {}", e),
        }
    }
}

impl std::error::Error for AllocatorError {}

pub type AllocatorResult<T> = Result<T, AllocatorError>;

impl From<std::io::Error> for AllocatorError {
    fn from(e: std::io::Error) -> Self {
        AllocatorError::IoError(e)
    }
}

impl From<crate::vfs::VfsError> for AllocatorError {
    fn from(e: crate::vfs::VfsError) -> Self {
        AllocatorError::VfsError(e)
    }
}

pub struct IndexPageAllocator {
    vfs: Arc<dyn VfsInterface>,
    data_dir: PathBuf,
    next_page_id: u64,
    freed_pages: HashSet<PageId>,
}

impl IndexPageAllocator {
    pub fn new(vfs: Arc<dyn VfsInterface>, data_dir: PathBuf) -> Self {
        Self {
            vfs,
            data_dir,
            next_page_id: 1,
            freed_pages: HashSet::new(),
        }
    }

    pub fn load(&mut self) -> AllocatorResult<()> {
        let alloc_file = self.data_dir.join("index_alloc.dat");
        let mut data = vec![0u8; 8192];
        let n = match self.vfs.pread(alloc_file.to_str().unwrap(), &mut data, 0) {
            Ok(n) => n,
            Err(_) => return Ok(()),
        };
        if n < 16 {
            return Ok(());
        }
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != ALLOCATOR_MAGIC {
            return Ok(());
        }
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        if version != ALLOCATOR_VERSION {
            return Ok(());
        }
        let mut offset = 16;
        self.next_page_id = u64::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        offset += 8;
        let num_freed = u64::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        offset += 8;
        for _ in 0..num_freed {
            if offset + 8 > data.len() {
                break;
            }
            let freed_id = u64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            self.freed_pages.insert(freed_id);
            offset += 8;
        }
        Ok(())
    }

    pub fn save(&self) -> AllocatorResult<()> {
        let alloc_file = self.data_dir.join("index_alloc.dat");
        let mut data = vec![0u8; 8192];
        data[0..4].copy_from_slice(&ALLOCATOR_MAGIC.to_le_bytes());
        data[4..8].copy_from_slice(&ALLOCATOR_VERSION.to_le_bytes());
        let mut offset = 16;
        data[offset..offset + 8].copy_from_slice(&self.next_page_id.to_le_bytes());
        offset += 8;
        let num_freed = self.freed_pages.len() as u64;
        data[offset..offset + 8].copy_from_slice(&num_freed.to_le_bytes());
        offset += 8;
        for freed_id in &self.freed_pages {
            if offset + 8 > data.len() {
                break;
            }
            data[offset..offset + 8].copy_from_slice(&freed_id.to_le_bytes());
            offset += 8;
        }
        self.vfs
            .pwrite(alloc_file.to_str().unwrap(), &data, 0)
            .map_err(AllocatorError::VfsError)?;
        Ok(())
    }

    pub fn allocate(&mut self) -> PageId {
        if let Some(freed_id) = self.freed_pages.iter().next().copied() {
            self.freed_pages.remove(&freed_id);
            return freed_id;
        }
        let page_id = self.next_page_id;
        self.next_page_id += 1;
        page_id
    }

    pub fn deallocate(&mut self, page_id: PageId) {
        self.freed_pages.insert(page_id);
    }

    pub fn next_page_id(&self) -> PageId {
        self.next_page_id
    }

    pub fn set_next_page_id(&mut self, page_id: PageId) {
        if page_id > self.next_page_id {
            self.next_page_id = page_id;
        }
    }
}
