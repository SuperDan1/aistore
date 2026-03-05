use crate::types::PageId;

pub const MAX_KEY_SIZE: usize = 1024;
pub const DEFAULT_FILL_FACTOR: f32 = 0.8;

#[derive(Debug, Clone)]
pub struct IndexMeta {
    pub id: u64,
    pub name: String,
    pub table_id: u64,
    pub columns: Vec<String>,
    pub is_unique: bool,
    pub root_page_id: PageId,
    pub segment_id: u64,
    pub fill_factor: f32,
    pub max_key_size: usize,
}

impl IndexMeta {
    pub fn new(
        id: u64,
        name: String,
        table_id: u64,
        columns: Vec<String>,
        is_unique: bool,
    ) -> Self {
        Self {
            id,
            name,
            table_id,
            columns,
            is_unique,
            root_page_id: 0,
            segment_id: 0,
            fill_factor: DEFAULT_FILL_FACTOR,
            max_key_size: MAX_KEY_SIZE,
        }
    }

    pub fn with_root_page_id(mut self, page_id: PageId) -> Self {
        self.root_page_id = page_id;
        self
    }

    pub fn with_segment_id(mut self, segment_id: u64) -> Self {
        self.segment_id = segment_id;
        self
    }

    pub fn with_fill_factor(mut self, fill_factor: f32) -> Self {
        self.fill_factor = fill_factor;
        self
    }

    pub fn with_max_key_size(mut self, max_key_size: usize) -> Self {
        self.max_key_size = max_key_size;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_meta_creation() {
        let meta = IndexMeta::new(1, "idx_id".to_string(), 100, vec!["id".to_string()], true);

        assert_eq!(meta.id, 1);
        assert_eq!(meta.name, "idx_id");
        assert_eq!(meta.table_id, 100);
        assert_eq!(meta.columns, vec!["id"]);
        assert!(meta.is_unique);
        assert_eq!(meta.fill_factor, DEFAULT_FILL_FACTOR);
        assert_eq!(meta.max_key_size, MAX_KEY_SIZE);
    }
}
