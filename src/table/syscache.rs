//! System cache (syscache) for storing table metadata
//!
//! This module provides a simple in-memory hash table for storing
//! and retrieving table metadata. It supports:
//! - Insert: Add a new table entry
//! - Get: Retrieve table by name or ID
//! - Remove: Delete a table entry
//! - Size: Get the number of entries

use crate::table::Table;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// System cache error types
#[derive(Debug, PartialEq, Eq)]
pub enum SysCacheError {
    /// Table not found in cache
    NotFound(String),
    /// Table already exists
    AlreadyExists(String),
}

impl std::fmt::Display for SysCacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SysCacheError::NotFound(name) => write!(f, "Table not found: {}", name),
            SysCacheError::AlreadyExists(name) => write!(f, "Table already exists: {}", name),
        }
    }
}

impl std::error::Error for SysCacheError {}

/// System cache result type
pub type SysCacheResult<T> = Result<T, SysCacheError>;

/// System cache for storing table metadata
///
/// Provides thread-safe storage for table metadata with
/// name-based and ID-based lookups.
///
/// # Examples
///
/// ```
/// use aistore::table::SysCache;
/// use aistore::table::Table;
///
/// let cache = SysCache::new();
/// let table = Table::new(1, "test".to_string(), 100);
///
/// // Insert table
/// cache.insert(table.clone());
///
/// // Get by name
/// let retrieved = cache.get_by_name("test").unwrap();
/// assert_eq!(retrieved.table_id, table.table_id);
///
/// // Get by ID
/// let retrieved = cache.get_by_id(1).unwrap();
/// assert_eq!(retrieved.table_name, table.table_name);
/// ```
pub struct SysCache {
    /// HashMap from table name to Table
    by_name: RwLock<HashMap<String, Arc<Table>>>,
    /// HashMap from table ID to Table
    by_id: RwLock<HashMap<u64, Arc<Table>>>,
    /// Next available table ID
    next_table_id: RwLock<u64>,
}

impl SysCache {
    /// Create a new empty system cache
    pub fn new() -> Self {
        Self {
            by_name: RwLock::new(HashMap::new()),
            by_id: RwLock::new(HashMap::new()),
            next_table_id: RwLock::new(1),
        }
    }

    /// Create a system cache with specified initial capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            by_name: RwLock::new(HashMap::with_capacity(capacity)),
            by_id: RwLock::new(HashMap::with_capacity(capacity)),
            next_table_id: RwLock::new(1),
        }
    }

    /// Insert a table into the cache
    pub fn insert(&self, table: Table) -> SysCacheResult<()> {
        let table_arc = Arc::new(table);
        let name = table_arc.table_name().to_string();
        let id = table_arc.table_id();

        let mut by_name = self.by_name.write().unwrap();
        let mut by_id = self.by_id.write().unwrap();

        if by_name.contains_key(&name) {
            return Err(SysCacheError::AlreadyExists(name));
        }
        if by_id.contains_key(&id) {
            return Err(SysCacheError::AlreadyExists(format!("table ID {}", id)));
        }

        by_name.insert(name, Arc::clone(&table_arc));
        by_id.insert(id, table_arc);

        Ok(())
    }

    /// Get a table by name
    pub fn get_by_name(&self, name: &str) -> SysCacheResult<Arc<Table>> {
        let by_name = self.by_name.read().unwrap();
        by_name
            .get(name)
            .map(Arc::clone)
            .ok_or_else(|| SysCacheError::NotFound(name.to_string()))
    }

    /// Get a table by ID
    pub fn get_by_id(&self, id: u64) -> SysCacheResult<Arc<Table>> {
        let by_id = self.by_id.read().unwrap();
        by_id
            .get(&id)
            .map(Arc::clone)
            .ok_or_else(|| SysCacheError::NotFound(format!("table ID {}", id)))
    }

    /// Remove a table by name
    pub fn remove_by_name(&self, name: &str) -> SysCacheResult<Arc<Table>> {
        let mut by_name = self.by_name.write().unwrap();
        let mut by_id = self.by_id.write().unwrap();

        let table = by_name
            .remove(name)
            .ok_or_else(|| SysCacheError::NotFound(name.to_string()))?;

        by_id.remove(&table.table_id());
        Ok(table)
    }

    /// Remove a table by ID
    pub fn remove_by_id(&self, id: u64) -> SysCacheResult<Arc<Table>> {
        let mut by_id = self.by_id.write().unwrap();
        let mut by_name = self.by_name.write().unwrap();

        let table = by_id
            .remove(&id)
            .ok_or_else(|| SysCacheError::NotFound(format!("table ID {}", id)))?;

        by_name.remove(table.table_name());
        Ok(table)
    }

    /// Check if a table exists by name
    pub fn exists_by_name(&self, name: &str) -> bool {
        let by_name = self.by_name.read().unwrap();
        by_name.contains_key(name)
    }

    /// Check if a table exists by ID
    pub fn exists_by_id(&self, id: u64) -> bool {
        let by_id = self.by_id.read().unwrap();
        by_id.contains_key(&id)
    }

    /// Get the number of tables in the cache
    pub fn size(&self) -> usize {
        let by_name = self.by_name.read().unwrap();
        by_name.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        let by_name = self.by_name.read().unwrap();
        by_name.is_empty()
    }

    /// Generate and reserve next table ID
    pub fn allocate_table_id(&self) -> u64 {
        let mut next_id = self.next_table_id.write().unwrap();
        let id = *next_id;
        *next_id += 1;
        id
    }

    /// Clear all entries from the cache
    pub fn clear(&self) {
        let mut by_name = self.by_name.write().unwrap();
        let mut by_id = self.by_id.write().unwrap();
        by_name.clear();
        by_id.clear();
    }
}

impl Default for SysCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syscache_insert_and_get() {
        let cache = SysCache::new();
        let table = Table::new(1, "test_table".to_string(), 100);

        cache.insert(table).unwrap();

        let retrieved = cache.get_by_name("test_table").unwrap();
        assert_eq!(retrieved.table_id, 1);
        assert_eq!(retrieved.table_name(), "test_table");

        let retrieved = cache.get_by_id(1).unwrap();
        assert_eq!(retrieved.segment_id, 100);
    }

    #[test]
    fn test_syscache_remove() {
        let cache = SysCache::new();
        let table = Table::new(1, "test_table".to_string(), 100);

        cache.insert(table).unwrap();
        assert_eq!(cache.size(), 1);

        let removed = cache.remove_by_name("test_table").unwrap();
        assert_eq!(removed.table_id, 1);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_syscache_duplicate_insert() {
        let cache = SysCache::new();
        let table = Table::new(1, "test_table".to_string(), 100);

        cache.insert(table).unwrap();
        let duplicate = Table::new(2, "test_table".to_string(), 200);

        assert!(matches!(
            cache.insert(duplicate),
            Err(SysCacheError::AlreadyExists(_))
        ));
    }

    #[test]
    fn test_syscache_not_found() {
        let cache = SysCache::new();

        assert!(matches!(
            cache.get_by_name("nonexistent"),
            Err(SysCacheError::NotFound(_))
        ));

        assert!(matches!(
            cache.get_by_id(999),
            Err(SysCacheError::NotFound(_))
        ));
    }

    #[test]
    fn test_syscache_allocate_id() {
        let cache = SysCache::new();

        let id1 = cache.allocate_table_id();
        let id2 = cache.allocate_table_id();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }
}
