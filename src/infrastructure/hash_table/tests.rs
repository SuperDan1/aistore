use super::*;

// Test both hash map implementations

// Test for StdHashMapWrapper
#[test]
fn test_std_hash_map_basic() {
    // Create a new hash table with capacity
    let mut hash_table = StdHashMapWrapper::with_capacity(10);
    
    // Insert some key-value pairs
    hash_table.insert(1, "one");
    hash_table.insert(2, "two");
    hash_table.insert(3, "three");
    
    // Check the size
    assert_eq!(hash_table.size(), 3);
    
    // Get values
    assert_eq!(hash_table.get(&1), Some(&"one"));
    assert_eq!(hash_table.get(&2), Some(&"two"));
    assert_eq!(hash_table.get(&3), Some(&"three"));
    assert_eq!(hash_table.get(&4), None);
    
    // Update a value
    hash_table.insert(1, "uno");
    assert_eq!(hash_table.get(&1), Some(&"uno"));
    assert_eq!(hash_table.size(), 3);
    
    // Remove a value
    assert_eq!(hash_table.remove(&2), Some("two"));
    assert_eq!(hash_table.size(), 2);
    assert_eq!(hash_table.get(&2), None);
    
    // Remove a non-existent key
    assert_eq!(hash_table.remove(&4), None);
    assert_eq!(hash_table.size(), 2);
}

#[test]
fn test_std_hash_map_empty() {
    let mut hash_table = StdHashMapWrapper::new();
    
    // Check if empty
    assert!(hash_table.is_empty());
    assert_eq!(hash_table.size(), 0);
    
    // Remove from empty table
    assert_eq!(hash_table.remove(&1), None);
    
    // Get from empty table
    assert_eq!(hash_table.get(&1), None);
    
    // Insert and check not empty
    hash_table.insert(1, "value");
    assert!(!hash_table.is_empty());
    assert_eq!(hash_table.size(), 1);
}

// Test for LinkedHashMapWrapper
#[test]
fn test_linked_hash_map_basic() {
    // Create a new hash table with capacity
    let mut hash_table = LinkedHashMapWrapper::with_capacity(10);
    
    // Insert some key-value pairs
    hash_table.insert(1, "one");
    hash_table.insert(2, "two");
    hash_table.insert(3, "three");
    
    // Check the size
    assert_eq!(hash_table.size(), 3);
    
    // Get values
    assert_eq!(hash_table.get(&1), Some(&"one"));
    assert_eq!(hash_table.get(&2), Some(&"two"));
    assert_eq!(hash_table.get(&3), Some(&"three"));
    assert_eq!(hash_table.get(&4), None);
    
    // Update a value
    hash_table.insert(1, "uno");
    assert_eq!(hash_table.get(&1), Some(&"uno"));
    assert_eq!(hash_table.size(), 3);
    
    // Remove a value
    assert_eq!(hash_table.remove(&2), Some("two"));
    assert_eq!(hash_table.size(), 2);
    assert_eq!(hash_table.get(&2), None);
    
    // Remove a non-existent key
    assert_eq!(hash_table.remove(&4), None);
    assert_eq!(hash_table.size(), 2);
}

#[test]
fn test_linked_hash_map_empty() {
    let mut hash_table = LinkedHashMapWrapper::new();
    
    // Check if empty
    assert!(hash_table.is_empty());
    assert_eq!(hash_table.size(), 0);
    
    // Remove from empty table
    assert_eq!(hash_table.remove(&1), None);
    
    // Get from empty table
    assert_eq!(hash_table.get(&1), None);
    
    // Insert and check not empty
    hash_table.insert(1, "value");
    assert!(!hash_table.is_empty());
    assert_eq!(hash_table.size(), 1);
}
