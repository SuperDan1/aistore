// Hash table wrappers for std::collections::HashMap and linked_hash_map::LinkedHashMap

use std::hash::Hash;
// Don't import HashMap directly to avoid name conflict with the re-export
use linked_hash_map::LinkedHashMap;

/// Hash table interface trait
pub trait HashMapInterface<K, V> {
    /// Create a new hash table
    fn new() -> Self;

    /// Create a new hash table with the specified capacity
    fn with_capacity(capacity: usize) -> Self;

    /// Insert a key-value pair into the hash table
    fn insert(&mut self, key: K, value: V);

    /// Get the value associated with a key
    fn get(&self, key: &K) -> Option<&V>;

    /// Remove a key-value pair from the hash table
    fn remove(&mut self, key: &K) -> Option<V>;

    /// Get the number of elements in the hash table
    fn size(&self) -> usize;

    /// Check if the hash table is empty
    fn is_empty(&self) -> bool;
}

/// Wrapper for std::collections::HashMap
pub struct StdHashMapWrapper<K, V> {
    inner: std::collections::HashMap<K, V>,
}

impl<K, V> HashMapInterface<K, V> for StdHashMapWrapper<K, V>
where
    K: Eq + Hash,
    V: Clone,
{
    fn new() -> Self {
        Self {
            inner: std::collections::HashMap::new(),
        }
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: std::collections::HashMap::with_capacity(capacity),
        }
    }

    fn insert(&mut self, key: K, value: V) {
        self.inner.insert(key, value);
    }

    fn get(&self, key: &K) -> Option<&V> {
        self.inner.get(key)
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        self.inner.remove(key)
    }

    fn size(&self) -> usize {
        self.inner.len()
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// Wrapper for linked_hash_map::LinkedHashMap
pub struct LinkedHashMapWrapper<K, V> {
    inner: LinkedHashMap<K, V>,
}

impl<K, V> HashMapInterface<K, V> for LinkedHashMapWrapper<K, V>
where
    K: Eq + Hash,
    V: Clone,
{
    fn new() -> Self {
        Self {
            inner: LinkedHashMap::new(),
        }
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: LinkedHashMap::with_capacity(capacity),
        }
    }

    fn insert(&mut self, key: K, value: V) {
        self.inner.insert(key, value);
    }

    fn get(&self, key: &K) -> Option<&V> {
        self.inner.get(key)
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        self.inner.remove(key)
    }

    fn size(&self) -> usize {
        self.inner.len()
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

// Re-export the wrappers for easy access
pub use StdHashMapWrapper as HashMap;

#[cfg(test)]
mod tests {
    include!("tests.rs");
}
