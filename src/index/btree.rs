use crate::buffer::BufferMgr;
use crate::types::PageId;
use parking_lot::RwLock;
use std::sync::Arc;

const INDEX_PAGE_TYPE_INTERNAL: u8 = 0;
const INDEX_PAGE_TYPE_LEAF: u8 = 1;

const INDEX_HEADER_SIZE: usize = 16;

#[derive(Debug)]
pub enum IndexError {
    KeyTooLong,
    DuplicateKey,
    KeyNotFound,
    PageError(String),
    Other(String),
}

pub type IndexResult<T> = Result<T, IndexError>;

impl std::fmt::Display for IndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexError::KeyTooLong => write!(f, "Key too long"),
            IndexError::DuplicateKey => write!(f, "Duplicate key"),
            IndexError::KeyNotFound => write!(f, "Key not found"),
            IndexError::PageError(msg) => write!(f, "Page error: {}", msg),
            IndexError::Other(msg) => write!(f, "Index error: {}", msg),
        }
    }
}

impl std::error::Error for IndexError {}

pub struct BTreeIndex {
    root_page_id: PageId,
    buffer_mgr: Arc<RwLock<BufferMgr>>,
    fill_factor: f32,
    max_key_size: usize,
}

impl BTreeIndex {
    pub fn new(
        root_page_id: PageId,
        buffer_mgr: Arc<RwLock<BufferMgr>>,
        fill_factor: f32,
        max_key_size: usize,
    ) -> Self {
        Self {
            root_page_id,
            buffer_mgr,
            fill_factor,
            max_key_size,
        }
    }

    pub fn root_page_id(&self) -> PageId {
        self.root_page_id
    }

    pub fn search(&self, _key: &[u8]) -> IndexResult<Option<(PageId, usize)>> {
        if self.root_page_id == 0 {
            return Ok(None);
        }
        Ok(None)
    }

    pub fn insert(
        &mut self,
        key: &[u8],
        _rid: (PageId, usize),
        check_unique: bool,
    ) -> IndexResult<()> {
        if key.len() > self.max_key_size {
            return Err(IndexError::KeyTooLong);
        }

        if check_unique {
            if let Some(_) = self.search(key)? {
                return Err(IndexError::DuplicateKey);
            }
        }

        Ok(())
    }

    pub fn delete(&mut self, _key: &[u8], _rid: (PageId, usize)) -> IndexResult<()> {
        Ok(())
    }
}

pub fn create_root_page(
    _buffer_mgr: &Arc<RwLock<BufferMgr>>,
    _is_leaf: bool,
) -> IndexResult<PageId> {
    Ok(0)
}
