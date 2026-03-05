pub mod btree;
pub mod key;
pub mod meta;

use crate::buffer::BufferMgr;
use crate::heap::{RowId, Value};
use crate::table::Column;
use crate::types::PageId;
use btree::{create_root_page, BTreeIndex, IndexError, IndexResult};
use meta::IndexMeta;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct IndexManager {
    buffer_mgr: Arc<RwLock<BufferMgr>>,
    indexes: HashMap<u64, IndexMeta>,
    next_index_id: u64,
}

impl IndexManager {
    pub fn new(buffer_mgr: Arc<RwLock<BufferMgr>>) -> Self {
        Self {
            buffer_mgr,
            indexes: HashMap::new(),
            next_index_id: 1,
        }
    }

    pub fn create_index(
        &mut self,
        table_id: u64,
        name: String,
        columns: Vec<String>,
        is_unique: bool,
    ) -> IndexResult<u64> {
        let index_id = self.next_index_id;
        self.next_index_id += 1;

        let root_page_id = create_root_page(&self.buffer_mgr, true)?;

        let meta = IndexMeta::new(index_id, name, table_id, columns, is_unique)
            .with_root_page_id(root_page_id);

        self.indexes.insert(index_id, meta.clone());

        Ok(index_id)
    }

    pub fn drop_index(&mut self, index_id: u64) -> IndexResult<()> {
        self.indexes
            .remove(&index_id)
            .ok_or(IndexError::KeyNotFound)?;
        Ok(())
    }

    pub fn get_index(&self, index_id: u64) -> Option<&IndexMeta> {
        self.indexes.get(&index_id)
    }

    pub fn get_index_by_name(&self, name: &str) -> Option<&IndexMeta> {
        self.indexes.values().find(|m| m.name == name)
    }

    pub fn get_table_indexes(&self, table_id: u64) -> Vec<&IndexMeta> {
        self.indexes
            .values()
            .filter(|m| m.table_id == table_id)
            .collect()
    }

    pub fn insert(
        &mut self,
        index_id: u64,
        values: &[Value],
        columns: &[Column],
        rid: RowId,
    ) -> IndexResult<()> {
        let meta = self.indexes.get(&index_id).ok_or(IndexError::KeyNotFound)?;

        let key = build_key(values, columns, &meta.columns)?;

        let mut btree = BTreeIndex::new(
            meta.root_page_id,
            Arc::clone(&self.buffer_mgr),
            meta.fill_factor,
            meta.max_key_size,
        );

        btree.insert(&key, (rid.page_id, rid.slot_idx), meta.is_unique)
    }

    pub fn delete(
        &mut self,
        index_id: u64,
        values: &[Value],
        columns: &[Column],
        rid: RowId,
    ) -> IndexResult<()> {
        let meta = self.indexes.get(&index_id).ok_or(IndexError::KeyNotFound)?;

        let key = build_key(values, columns, &meta.columns)?;

        let mut btree = BTreeIndex::new(
            meta.root_page_id,
            Arc::clone(&self.buffer_mgr),
            meta.fill_factor,
            meta.max_key_size,
        );

        btree.delete(&key, (rid.page_id, rid.slot_idx))
    }

    pub fn lookup(
        &self,
        index_id: u64,
        values: &[Value],
        columns: &[Column],
    ) -> IndexResult<Vec<RowId>> {
        let meta = self.indexes.get(&index_id).ok_or(IndexError::KeyNotFound)?;

        let key = build_key(values, columns, &meta.columns)?;

        let btree = BTreeIndex::new(
            meta.root_page_id,
            Arc::clone(&self.buffer_mgr),
            meta.fill_factor,
            meta.max_key_size,
        );

        match btree.search(&key)? {
            Some((page_id, slot_idx)) => Ok(vec![RowId::new(page_id, slot_idx)]),
            None => Ok(vec![]),
        }
    }

    pub fn all_indexes(&self) -> Vec<&IndexMeta> {
        self.indexes.values().collect()
    }
}

fn build_key(
    values: &[Value],
    columns: &[Column],
    index_columns: &[String],
) -> IndexResult<Vec<u8>> {
    let mut key = Vec::new();

    for col_name in index_columns {
        let col_idx = columns
            .iter()
            .position(|c| c.name() == col_name)
            .ok_or_else(|| IndexError::Other(format!("Column {} not found", col_name)))?;

        let value = values
            .get(col_idx)
            .ok_or_else(|| IndexError::Other(format!("Value for column {} not found", col_name)))?;

        let serialized = key::serialize_value(value)
            .ok_or_else(|| IndexError::Other("Failed to serialize value".to_string()))?;

        key.extend_from_slice(&serialized);
    }

    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::Column;
    use crate::types::ColumnType;

    fn create_test_columns() -> Vec<Column> {
        vec![
            Column::new("id".to_string(), ColumnType::Int64, false, 0),
            Column::new("name".to_string(), ColumnType::Varchar(255), true, 1),
        ]
    }

    #[test]
    fn test_index_manager_creation() {
        let buffer_mgr = Arc::new(RwLock::new(BufferMgr::init(
            100,
            Arc::new(crate::vfs::LocalFs::new()),
            PathBuf::from("./test_data"),
        )));

        let mut mgr = IndexManager::new(buffer_mgr);
        let index_id = mgr
            .create_index(1, "idx_id".to_string(), vec!["id".to_string()], true)
            .unwrap();

        assert_eq!(index_id, 1);

        let meta = mgr.get_index(1).unwrap();
        assert_eq!(meta.name, "idx_id");
    }
}
