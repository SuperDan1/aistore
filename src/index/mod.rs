pub mod btree;
pub mod key;
pub mod meta;

use crate::buffer::BufferMgr;
use crate::heap::{RowId, Value};
use crate::table::Column;
use crate::types::PageId;
use crate::vfs::{FileHandle, VfsInterface};
use btree::{create_root_page, BTreeIndex, IndexError, IndexResult};
use meta::IndexMeta;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

const INDEX_FILE_MAGIC: u32 = 0x494e4458;
const INDEX_FILE_VERSION: u32 = 1;

pub struct IndexManager {
    buffer_mgr: Arc<RwLock<BufferMgr>>,
    vfs: Arc<dyn VfsInterface>,
    data_dir: PathBuf,
    indexes: HashMap<u64, IndexMeta>,
    btrees: HashMap<u64, BTreeIndex>,
    next_index_id: u64,
}

impl IndexManager {
    pub fn new(buffer_mgr: Arc<RwLock<BufferMgr>>, data_dir: PathBuf) -> Self {
        let vfs: Arc<dyn VfsInterface> = Arc::new(crate::vfs::LocalFs::new());
        Self {
            buffer_mgr,
            vfs,
            data_dir,
            indexes: HashMap::new(),
            btrees: HashMap::new(),
            next_index_id: 1,
        }
    }

    pub fn load(&mut self) -> IndexResult<()> {
        let index_file = self.data_dir.join("index.dat");

        let mut data = vec![0u8; 65536];
        let n = match self.vfs.pread(index_file.to_str().unwrap(), &mut data, 0) {
            Ok(n) => n,
            Err(_) => return Ok(()),
        };
        data.truncate(n);

        if data.len() < 16 {
            return Ok(());
        }

        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != INDEX_FILE_MAGIC {
            return Ok(());
        }

        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        if version != INDEX_FILE_VERSION {
            return Ok(());
        }

        let mut offset = 16;
        let num_indexes = if offset + 4 <= data.len() {
            u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize
        } else {
            0
        };
        offset += 4;

        for _ in 0..num_indexes {
            if offset + 8 > data.len() {
                break;
            }
            let id = u64::from_le_bytes([
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

            if offset + 4 > data.len() {
                break;
            }
            let name_len = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            offset += 4;

            if offset + name_len > data.len() {
                break;
            }
            let name = String::from_utf8_lossy(&data[offset..offset + name_len]).to_string();
            offset += name_len;

            if offset + 17 > data.len() {
                break;
            }
            let table_id = u64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let is_unique = data[offset + 8] != 0;
            let root_page_id = u64::from_le_bytes([
                data[offset + 9],
                data[offset + 10],
                data[offset + 11],
                data[offset + 12],
                data[offset + 13],
                data[offset + 14],
                data[offset + 15],
                data[offset + 16],
            ]);
            offset += 17;

            let num_cols = if offset + 4 <= data.len() {
                u32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]) as usize
            } else {
                0
            };
            offset += 4;

            let mut columns = Vec::new();
            for _ in 0..num_cols {
                if offset + 4 > data.len() {
                    break;
                }
                let col_len = u32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]) as usize;
                offset += 4;
                if offset + col_len > data.len() {
                    break;
                }
                columns.push(String::from_utf8_lossy(&data[offset..offset + col_len]).to_string());
                offset += col_len;
            }

            let meta = IndexMeta::new(id, name, table_id, columns, is_unique)
                .with_root_page_id(root_page_id);
            self.indexes.insert(id, meta);

            self.next_index_id = self.next_index_id.max(id + 1);

            self.btrees.insert(
                id,
                BTreeIndex::new(root_page_id, Arc::clone(&self.buffer_mgr), 0.8, 1024),
            );
        }

        Ok(())
    }

    pub fn flush(&self) -> IndexResult<()> {
        let index_file = self.data_dir.join("index.dat");

        let mut data = Vec::new();
        data.extend_from_slice(&INDEX_FILE_MAGIC.to_le_bytes());
        data.extend_from_slice(&INDEX_FILE_VERSION.to_le_bytes());
        data.extend_from_slice(&(self.indexes.len() as u32).to_le_bytes());

        for (id, meta) in &self.indexes {
            data.extend_from_slice(&id.to_le_bytes());

            let name_bytes = meta.name.as_bytes();
            data.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
            data.extend_from_slice(name_bytes);

            data.extend_from_slice(&meta.table_id.to_le_bytes());
            data.push(if meta.is_unique { 1 } else { 0 });
            data.extend_from_slice(&meta.root_page_id.to_le_bytes());

            data.extend_from_slice(&(meta.columns.len() as u32).to_le_bytes());
            for col in &meta.columns {
                let col_bytes = col.as_bytes();
                data.extend_from_slice(&(col_bytes.len() as u32).to_le_bytes());
                data.extend_from_slice(col_bytes);
            }
        }

        let handle = self.vfs.create_file(index_file.to_str().unwrap()).ok();
        if let Some(mut h) = handle {
            h.pwrite(&data, 0)
                .map_err(|e| IndexError::Other(e.to_string()))?;
        }

        Ok(())
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

        let btree = BTreeIndex::new(
            root_page_id,
            Arc::clone(&self.buffer_mgr),
            meta.fill_factor,
            meta.max_key_size,
        );
        self.btrees.insert(index_id, btree);

        Ok(index_id)
    }

    pub fn drop_index(&mut self, index_id: u64) -> IndexResult<()> {
        self.indexes
            .remove(&index_id)
            .ok_or(IndexError::KeyNotFound)?;
        self.btrees.remove(&index_id);
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

        let btree = self
            .btrees
            .get_mut(&index_id)
            .ok_or(IndexError::KeyNotFound)?;

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

        let btree = self
            .btrees
            .get_mut(&index_id)
            .ok_or(IndexError::KeyNotFound)?;

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

        let btree = self.btrees.get(&index_id).ok_or(IndexError::KeyNotFound)?;

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

        let mut mgr = IndexManager::new(buffer_mgr, PathBuf::from("./test_data"));
        let index_id = mgr
            .create_index(1, "idx_id".to_string(), vec!["id".to_string()], true)
            .unwrap();

        assert_eq!(index_id, 1);

        let meta = mgr.get_index(1).unwrap();
        assert_eq!(meta.name, "idx_id");
    }
}
