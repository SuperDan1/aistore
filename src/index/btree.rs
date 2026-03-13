use crate::buffer::BufferMgr;
use crate::page::Page;
use crate::types::PageId;
use std::sync::Arc;
use tokio::sync::RwLock;

const BTREE_MAX_KEY_SIZE: usize = 128;

#[derive(Debug)]
pub enum IndexError {
    KeyTooLong,
    DuplicateKey,
    KeyNotFound,
    PageError(String),
    BufferError(String),
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
            IndexError::BufferError(msg) => write!(f, "Buffer error: {}", msg),
            IndexError::Other(msg) => write!(f, "Index error: {}", msg),
        }
    }
}

impl std::error::Error for IndexError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BTreePageType {
    Internal = 0,
    Leaf = 1,
}

/// BTreePage that can be serialized to/from BufferPool Page
#[derive(Debug, Clone)]
pub struct BTreePage {
    pub page_id: PageId,
    pub page_type: BTreePageType,
    pub keys: Vec<Vec<u8>>,
    pub children: Vec<PageId>,
    pub values: Vec<(PageId, usize)>,
    pub right_sibling: PageId,
}

impl BTreePage {
    pub fn new_leaf(page_id: PageId) -> Self {
        Self {
            page_id,
            page_type: BTreePageType::Leaf,
            keys: Vec::new(),
            children: Vec::new(),
            values: Vec::new(),
            right_sibling: 0,
        }
    }

    pub fn new_internal(page_id: PageId) -> Self {
        Self {
            page_id,
            page_type: BTreePageType::Internal,
            keys: Vec::new(),
            children: Vec::new(),
            values: Vec::new(),
            right_sibling: 0,
        }
    }

    pub fn is_leaf(&self) -> bool {
        self.page_type == BTreePageType::Leaf
    }

    pub fn is_full(&self) -> bool {
        let max_keys = if self.is_leaf() { 64 } else { 128 };
        self.keys.len() >= max_keys
    }

    pub fn is_minimal(&self) -> bool {
        let min_keys = if self.is_leaf() { 1 } else { 2 };
        self.keys.len() < min_keys
    }

    pub fn find_key(&self, key: &[u8]) -> Option<usize> {
        for (i, k) in self.keys.iter().enumerate() {
            if k == key {
                return Some(i);
            }
        }
        None
    }

    pub fn find_insert_position(&self, key: &[u8]) -> usize {
        for (i, k) in self.keys.iter().enumerate() {
            if key < k.as_slice() {
                return i;
            }
        }
        self.keys.len()
    }

    /// Serialize BTreePage into a Page for BufferPool storage
    pub fn serialize(&self, page: &mut Page) {
        // Set page type
        page.header.type_ = if self.is_leaf() { 1 } else { 0 };
        page.header.myself = self.page_id;

        // Calculate available space: PAGE_SIZE - header (48) = 8144 bytes
        const HEADER_SIZE: usize = 48;
        const PAGE_SIZE: usize = 8192;
        const DATA_SIZE: usize = PAGE_SIZE - HEADER_SIZE;

        // Format:
        // - right_sibling: 8 bytes
        // - num_keys: 4 bytes
        // - num_children: 4 bytes (internal only)
        // - num_values: 4 bytes (leaf only)
        // - keys and values stored as: [key_len:2][key_data][value_data]

        let mut offset = 0;
        let data =
            unsafe { std::slice::from_raw_parts_mut(page as *mut Page as *mut u8, PAGE_SIZE) };

        // Write right_sibling
        data[offset..offset + 8].copy_from_slice(&self.page_id.to_le_bytes());
        offset += 8;

        // Write number of keys
        let num_keys = self.keys.len() as u32;
        data[offset..offset + 4].copy_from_slice(&num_keys.to_le_bytes());
        offset += 4;

        // Write number of children/values
        if self.is_leaf() {
            let num_values = self.values.len() as u32;
            data[offset..offset + 4].copy_from_slice(&num_values.to_le_bytes());
            offset += 4;
        } else {
            let num_children = self.children.len() as u32;
            data[offset..offset + 4].copy_from_slice(&num_children.to_le_bytes());
            offset += 4;
        }

        // Write keys and values/data
        if self.is_leaf() {
            // For leaf: [key_len:2][key_data][page_id:8][slot_idx:8]
            for (i, key) in self.keys.iter().enumerate() {
                // Key length (2 bytes)
                let key_len = key.len() as u16;
                data[offset..offset + 2].copy_from_slice(&key_len.to_le_bytes());
                offset += 2;

                // Key data
                if offset + key.len() > DATA_SIZE {
                    break; // Would overflow
                }
                data[offset..offset + key.len()].copy_from_slice(key);
                offset += key.len();

                // Value: (page_id, slot_idx)
                let (page_id, slot_idx) = self.values[i];
                data[offset..offset + 8].copy_from_slice(&page_id.to_le_bytes());
                offset += 8;
                data[offset..offset + 8].copy_from_slice(&slot_idx.to_le_bytes());
                offset += 8;
            }
        } else {
            // For internal: [key_len:2][key_data][child_page_id:8]
            for (i, key) in self.keys.iter().enumerate() {
                // Key length (2 bytes)
                let key_len = key.len() as u16;
                data[offset..offset + 2].copy_from_slice(&key_len.to_le_bytes());
                offset += 2;

                // Key data
                if offset + key.len() > DATA_SIZE {
                    break;
                }
                data[offset..offset + key.len()].copy_from_slice(key);
                offset += key.len();

                // Child page_id
                let child_id = self.children[i];
                data[offset..offset + 8].copy_from_slice(&child_id.to_le_bytes());
                offset += 8;
            }
            // Last child
            if !self.children.is_empty() {
                let last_child = *self.children.last().unwrap();
                data[offset..offset + 8].copy_from_slice(&last_child.to_le_bytes());
                offset += 8;
            }
        }

        // Update page header
        page.header.lower = offset as u16;
        page.header.upper = DATA_SIZE as u16;
    }

    /// Deserialize BTreePage from a BufferPool Page
    pub fn deserialize(page: &Page) -> Self {
        const HEADER_SIZE: usize = 48;
        const PAGE_SIZE: usize = 8192;
        const DATA_SIZE: usize = PAGE_SIZE - HEADER_SIZE;

        let data =
            unsafe { std::slice::from_raw_parts(page as *const Page as *const u8, PAGE_SIZE) };

        let mut offset = 0;

        // Read right_sibling
        let right_sibling = PageId::from_le_bytes([
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

        // Read number of keys
        let num_keys = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        // Read number of children/values
        let is_leaf = page.header.type_ == 1;
        let num_children_or_values = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        let page_id = page.header.myself;
        let mut keys = Vec::new();
        let mut children = Vec::new();
        let mut values = Vec::new();

        if is_leaf {
            // Read keys and values
            for _ in 0..num_keys {
                // Key length
                let key_len = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
                offset += 2;

                // Key data
                let key = data[offset..offset + key_len].to_vec();
                offset += key_len;

                // Value
                let page_id_val = PageId::from_le_bytes([
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
                let slot_idx = usize::from_le_bytes([
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

                keys.push(key);
                values.push((page_id_val, slot_idx));
            }
        } else {
            // Read keys and children
            for i in 0..num_keys {
                // Key length
                let key_len = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
                offset += 2;

                // Key data
                let key = data[offset..offset + key_len].to_vec();
                offset += key.len();

                // Child page_id
                let child_id = PageId::from_le_bytes([
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

                keys.push(key);
                children.push(child_id);
            }
            // Last child
            if num_children_or_values > num_keys {
                let last_child = PageId::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                    data[offset + 4],
                    data[offset + 5],
                    data[offset + 6],
                    data[offset + 7],
                ]);
                children.push(last_child);
            }
        }

        Self {
            page_id,
            page_type: if is_leaf {
                BTreePageType::Leaf
            } else {
                BTreePageType::Internal
            },
            keys,
            children,
            values,
            right_sibling,
        }
    }

    pub fn split_leaf(&mut self) -> (Vec<u8>, BTreePage) {
        let mid = self.keys.len() / 2;
        let split_key = self.keys[mid].clone();

        let mut new_page = BTreePage::new_leaf(self.page_id + 1);
        new_page.right_sibling = self.right_sibling;
        self.right_sibling = new_page.page_id;

        for i in mid..self.keys.len() {
            new_page.keys.push(self.keys[i].clone());
            new_page.values.push(self.values[i]);
        }

        self.keys.truncate(mid);
        self.values.truncate(mid);

        (split_key, new_page)
    }

    pub fn split_internal(&mut self) -> (Vec<u8>, BTreePage) {
        let mid = self.keys.len() / 2;
        let split_key = self.keys[mid].clone();

        let mut new_page = BTreePage::new_internal(self.page_id + 1);

        for i in mid + 1..self.keys.len() {
            new_page.keys.push(self.keys[i].clone());
            new_page.children.push(self.children[i]);
        }
        new_page
            .children
            .push(self.children.last().copied().unwrap_or(0));

        self.keys.truncate(mid);
        self.children.truncate(mid + 1);

        (split_key, new_page)
    }
}

/// BTreeIndex that uses BufferPool for page I/O
pub struct BTreeIndex {
    root_page_id: PageId,
    fill_factor: f32,
    max_key_size: usize,
    buffer_mgr: Arc<RwLock<BufferMgr>>,
    next_page_id: u64,
    dirty_pages: Vec<PageId>,
    index_id: u64,
    allocated_pages: Vec<PageId>,
}

impl BTreeIndex {
    pub fn new(
        root_page_id: PageId,
        buffer_mgr: Arc<RwLock<BufferMgr>>,
        fill_factor: f32,
        max_key_size: usize,
        index_id: u64,
    ) -> Self {
        let mut allocated_pages = vec![root_page_id];
        Self {
            root_page_id: if root_page_id == 0 { 1 } else { root_page_id },
            fill_factor,
            max_key_size,
            buffer_mgr,
            next_page_id: if root_page_id == 0 {
                2
            } else {
                root_page_id + 1
            },
            dirty_pages: Vec::new(),
            index_id,
            allocated_pages,
        }
    }

    pub fn root_page_id(&self) -> PageId {
        self.root_page_id
    }

    /// Load a page from BufferPool
    fn load_page(&self, page_id: PageId) -> IndexResult<Option<BTreePage>> {
        let buf = self.buffer_mgr.blocking_read();
        if let Some(page) = buf.get_page_data(page_id) {
            let btree_page = BTreePage::deserialize(page);
            Ok(Some(btree_page))
        } else {
            Ok(None)
        }
    }

    /// Save a page to BufferPool and mark dirty
    fn save_page(&mut self, btree_page: &BTreePage) -> IndexResult<()> {
        let mut buf = self.buffer_mgr.blocking_write();
        match buf.get_page(btree_page.page_id) {
            Ok(page) => {
                btree_page.serialize(page);
                buf.mark_dirty(btree_page.page_id);
                if !self.dirty_pages.contains(&btree_page.page_id) {
                    self.dirty_pages.push(btree_page.page_id);
                }
                Ok(())
            }
            Err(e) => Err(IndexError::BufferError(e.to_string())),
        }
    }

    pub fn set_next_page_id(&mut self, page_id: PageId) {
        if page_id > self.next_page_id {
            self.next_page_id = page_id;
        }
    }

    pub fn index_id(&self) -> u64 {
        self.index_id
    }

    pub fn allocated_pages(&self) -> &[PageId] {
        &self.allocated_pages
    }

    fn allocate_page(&mut self) -> PageId {
        let page_id = self.next_page_id;
        self.next_page_id += 1;
        self.allocated_pages.push(page_id);
        page_id
    }

    pub fn search(&self, key: &[u8]) -> IndexResult<Option<(PageId, usize)>> {
        if self.root_page_id == 0 {
            return Ok(None);
        }
        self.search_node(self.root_page_id, key)
    }

    fn search_node(&self, page_id: PageId, key: &[u8]) -> IndexResult<Option<(PageId, usize)>> {
        if let Some(page) = self.load_page(page_id)? {
            if page.is_leaf() {
                if let Some(idx) = page.find_key(key) {
                    return Ok(Some(page.values[idx]));
                }
                return Ok(None);
            }

            let idx = page.find_insert_position(key);
            let child_id = page.children.get(idx).copied().unwrap_or(0);

            if child_id > 0 {
                return self.search_node(child_id, key);
            }
        }

        Ok(None)
    }

    pub fn insert(
        &mut self,
        key: &[u8],
        rid: (PageId, usize),
        check_unique: bool,
    ) -> IndexResult<()> {
        if key.len() > self.max_key_size {
            return Err(IndexError::KeyTooLong);
        }

        if self.root_page_id == 0 {
            // Create root page
            let root_id = self.allocate_page();
            let mut root = BTreePage::new_leaf(root_id);
            root.keys.push(key.to_vec());
            root.values.push(rid);
            self.save_page(&root)?;
            self.root_page_id = root_id;
            // Save root pointer
            self.save_root_pointer()?;
            return Ok(());
        }

        if check_unique {
            if self.search(key)?.is_some() {
                return Err(IndexError::DuplicateKey);
            }
        }

        self.insert_into(self.root_page_id, key, rid)
    }

    fn insert_into(
        &mut self,
        page_id: PageId,
        key: &[u8],
        rid: (PageId, usize),
    ) -> IndexResult<()> {
        if let Some(mut page) = self.load_page(page_id)? {
            if page.is_leaf() {
                let pos = page.find_insert_position(key);
                page.keys.insert(pos, key.to_vec());
                page.values.insert(pos, rid);
                self.save_page(&page)?;

                if page.is_full() {
                    let (split_key, new_page) = page.split_leaf();
                    let new_id = self.allocate_page();
                    let mut new_page_with_id = new_page;
                    new_page_with_id.page_id = new_id;
                    self.save_page(&new_page_with_id)?;

                    // Reload parent and update
                    if page_id == self.root_page_id {
                        let new_root_id = self.allocate_page();
                        let mut new_root = BTreePage::new_internal(new_root_id);
                        new_root.keys.push(split_key);
                        new_root.children.push(page_id);
                        new_root.children.push(new_id);
                        self.save_page(&new_root)?;
                        self.root_page_id = new_root_id;
                    } else {
                        self.push_up(page_id, split_key, new_id)?;
                    }
                    // Save updated split page
                    if let Some(updated_page) = self.load_page(page_id)? {
                        self.save_page(&updated_page)?;
                    }
                }

                // Save root pointer after any modification
                self.save_root_pointer()?;
                return Ok(());
            }

            let pos = page.find_insert_position(key);
            let child_id = page.children.get(pos).copied().unwrap_or(0);

            if child_id > 0 {
                let result = self.insert_into(child_id, key, rid);
                // After insert, may need to update parent if split occurred
                if let Some(updated_parent) = self.load_page(page_id)? {
                    self.save_page(&updated_parent)?;
                }
                return result;
            }
        }

        Ok(())
    }

    fn push_up(
        &mut self,
        parent_id: PageId,
        split_key: Vec<u8>,
        new_child_id: PageId,
    ) -> IndexResult<()> {
        if let Some(mut parent) = self.load_page(parent_id)? {
            let pos = parent.find_insert_position(&split_key);
            parent.keys.insert(pos, split_key);
            parent.children.insert(pos + 1, new_child_id);
            self.save_page(&parent)?;

            if parent.is_full() {
                let (new_split_key, mut new_page) = parent.split_internal();
                new_page.page_id = self.allocate_page();
                self.save_page(&new_page)?;

                if parent_id == self.root_page_id {
                    let new_root_id = self.allocate_page();
                    let mut new_root = BTreePage::new_internal(new_root_id);
                    new_root.keys.push(new_split_key);
                    new_root.children.push(parent_id);
                    new_root.children.push(new_page.page_id);
                    self.save_page(&new_root)?;
                    self.root_page_id = new_root_id;
                } else {
                    self.push_up(self.root_page_id, new_split_key, new_page.page_id)?;
                }
            }
            self.save_root_pointer()?;
        }

        Ok(())
    }

    /// Save root page ID to a special metadata page (page_id = 0)
    fn save_root_pointer(&self) -> IndexResult<()> {
        let mut buf = self.buffer_mgr.blocking_write();
        match buf.get_page(0) {
            Ok(page) => {
                // Page 0: store root_page_id at offset 0
                let data = unsafe {
                    std::slice::from_raw_parts_mut(page as *mut crate::page::Page as *mut u8, 8192)
                };
                data[0..8].copy_from_slice(&self.root_page_id.to_le_bytes());
                buf.mark_dirty(0);
                Ok(())
            }
            Err(_) => Ok(()), // Ignore if page 0 doesn't exist
        }
    }

    pub fn load_root_pointer(buffer_mgr: &Arc<RwLock<BufferMgr>>) -> IndexResult<PageId> {
        let buf = buffer_mgr.blocking_read();
        if let Some(page) = buf.get_page_data(0) {
            let data = unsafe {
                std::slice::from_raw_parts(page as *const crate::page::Page as *const u8, 8192)
            };
            let root_page_id = u64::from_le_bytes([
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
            ]);
            Ok(root_page_id)
        } else {
            Ok(0)
        }
    }

    pub fn delete(&mut self, key: &[u8], _rid: (PageId, usize)) -> IndexResult<()> {
        if self.root_page_id == 0 {
            return Ok(());
        }

        self.delete_from(self.root_page_id, key)?;
        self.save_root_pointer()
    }

    fn delete_from(&mut self, page_id: PageId, key: &[u8]) -> IndexResult<()> {
        if let Some(mut page) = self.load_page(page_id)? {
            if page.is_leaf() {
                if let Some(pos) = page.find_key(key) {
                    page.keys.remove(pos);
                    page.values.remove(pos);
                    self.save_page(&page)?;
                }
                return Ok(());
            }

            let pos = page.find_insert_position(key);
            let child_id = page.children.get(pos).copied().unwrap_or(0);

            if child_id > 0 {
                self.delete_from(child_id, key)?;
                // Reload and save parent after child deletion
                if let Some(updated_parent) = self.load_page(page_id)? {
                    self.save_page(&updated_parent)?;
                }
            }
        }

        Ok(())
    }
}

pub fn create_root_page(buffer_mgr: &Arc<RwLock<BufferMgr>>, is_leaf: bool) -> IndexResult<PageId> {
    let page_id = {
        let buf = buffer_mgr.blocking_write();
        // Allocate a new page (next available)
        // For now, just return a new ID
        1
    };
    Ok(page_id)
}
