// Table module tests

#[cfg(test)]
mod table_tests {
    use crate::table::{SysCache, Table, TableType};

    #[test]
    fn test_table_creation() {
        let table = Table::new(1, "users".to_string(), 100);
        assert_eq!(table.table_id, 1);
        assert_eq!(table.table_name, "users");
        assert_eq!(table.segment_id, 100);
        assert_eq!(table.table_type, TableType::User);
    }

    #[test]
    fn test_table_with_type() {
        let table = Table::with_type(2, "config".to_string(), 200, TableType::System);
        assert!(table.is_system());
        assert!(!table.is_temporary());
    }

    #[test]
    fn test_syscache_basic_operations() {
        let cache = SysCache::new();

        // Insert
        let table = Table::new(1, "test".to_string(), 100);
        cache.insert(table).unwrap();

        // Get by name
        let retrieved = cache.get_by_name("test").unwrap();
        assert_eq!(retrieved.table_id, 1);

        // Get by ID
        let retrieved = cache.get_by_id(1).unwrap();
        assert_eq!(retrieved.table_name(), "test");

        // Size
        assert_eq!(cache.size(), 1);
        assert!(!cache.is_empty());
    }

    #[test]
    fn test_syscache_remove() {
        let cache = SysCache::new();
        let table = Table::new(1, "test".to_string(), 100);

        cache.insert(table).unwrap();
        assert_eq!(cache.size(), 1);

        cache.remove_by_name("test").unwrap();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_syscache_allocate_id() {
        let cache = SysCache::new();

        assert_eq!(cache.allocate_table_id(), 1);
        assert_eq!(cache.allocate_table_id(), 2);
        assert_eq!(cache.allocate_table_id(), 3);
    }

    #[test]
    fn test_syscache_exists() {
        let cache = SysCache::new();
        let table = Table::new(1, "test".to_string(), 100);

        assert!(!cache.exists_by_name("test"));
        assert!(!cache.exists_by_id(1));

        cache.insert(table).unwrap();

        assert!(cache.exists_by_name("test"));
        assert!(cache.exists_by_id(1));
    }

    #[test]
    fn test_syscache_clear() {
        let cache = SysCache::new();

        cache.insert(Table::new(1, "t1".to_string(), 100)).unwrap();
        cache.insert(Table::new(2, "t2".to_string(), 200)).unwrap();

        assert_eq!(cache.size(), 2);

        cache.clear();

        assert!(cache.is_empty());
    }
}
