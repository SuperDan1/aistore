# HashTable - Hash Map Implementations

This module provides unified interfaces for different hash map implementations, allowing users to easily switch between different hash map types based on their performance requirements.

## Hash Map Implementations

### 1. StdHashMapWrapper
- Wrapper around `std::collections::HashMap`
- Standard library implementation
- Good general-purpose choice
- Uses a hash brown table internally (since Rust 1.36)

### 2. LinkedHashMapWrapper
- Wrapper around `linked_hash_map::LinkedHashMap`
- Maintains insertion order
- Allows iteration in insertion order
- Useful for LRU caches and other ordered hash map use cases
- Slightly slower than regular HashMap but provides ordering guarantees

## Performance Results

Performance benchmarks were run with the following configuration:
- **Thread Count**: 8
- **Operations per Thread**: 10,000
- **Total Operations**: 80,000

### 1. Concurrent Insertions

| Hash Map Type | Time per Iteration |
|---------------|-------------------|
| StdHashMap | 10.8 ms |
| LinkedHashMap | 17.4 ms |

### 2. Single-threaded Operations

| Hash Map Type | Time per Iteration |
|---------------|-------------------|
| StdHashMap | 386 µs |
| LinkedHashMap | 725 µs |

### 3. Read-heavy Workloads (90% reads, 10% writes)

| Hash Map Type | Time per Iteration |
|---------------|-------------------|
| StdHashMap | 281 µs |
| LinkedHashMap | 621 µs |

## Usage Example

```rust
use aistore::infrastructure::hash_table::{StdHashMapWrapper, LinkedHashMapWrapper};

// Create a standard hash map
let mut std_map = StdHashMapWrapper::new();
// or use the convenient alias
// let mut std_map = HashMap::new();

// Create a linked hash map (maintains insertion order)
let mut linked_map = LinkedHashMapWrapper::new();

// Insert values
std_map.insert(1, "one");
linked_map.insert(1, "one");

// Get values
let value1 = std_map.get(&1);
let value2 = linked_map.get(&1);

// Remove values
let removed1 = std_map.remove(&1);
let removed2 = linked_map.remove(&1);
```

## Performance Characteristics

- **StdHashMap**: High-performance general-purpose hash map, best for most scenarios
- **LinkedHashMap**: Maintains insertion order at the cost of slightly lower performance

Choose the hash map type based on your specific requirements:
- For general use: StdHashMap (best performance)
- When insertion order matters: LinkedHashMap (e.g., LRU caches, ordered iteration)
- For maximum compatibility: StdHashMap (standard library)

## API

All hash map implementations implement the `HashMapInterface` trait, providing a unified API:

```rust
pub trait HashMapInterface<K, V> {
    fn new() -> Self;
    fn with_capacity(capacity: usize) -> Self;
    fn insert(&mut self, key: K, value: V);
    fn get(&self, key: &K) -> Option<&V>;
    fn remove(&mut self, key: &K) -> Option<V>;
    fn size(&self) -> usize;
    fn is_empty(&self) -> bool;
}
```

This allows for easy switching between hash map implementations without changing the rest of your code.

## Running Tests

```bash
cargo test -p aistore --test hash_table
```

## Running Benchmarks

```bash
cargo bench --bench hash_table
```
