use criterion::{criterion_group, criterion_main, Criterion};  
use std::sync::Arc;
use std::thread;

// Import the hash map wrappers and interface from the crate
use aistore::infrastructure::hash_table::{StdHashMapWrapper, LinkedHashMapWrapper, HashMapInterface};

// Test configuration
const THREAD_COUNT: usize = 8;
const OPERATIONS_PER_THREAD: usize = 10_000;

// Benchmark concurrent insertions for StdHashMapWrapper
pub fn bench_concurrent_insertions_std_hash_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("ConcurrentInsertions");
    
    group.bench_function("StdHashMap", |b| {
        b.iter(|| {
            let map = Arc::new(std::sync::Mutex::new(StdHashMapWrapper::new()));
            
            // Spawn threads for concurrent insertions
            let handles: Vec<_> = (0..THREAD_COUNT).map(|thread_id| {
                let map = map.clone();
                thread::spawn(move || {
                    for i in 0..OPERATIONS_PER_THREAD {
                        let key = (thread_id * OPERATIONS_PER_THREAD + i) as u32;
                        let value = "test_value";
                        map.lock().unwrap().insert(key, value);
                    }
                })
            }).collect();
            
            // Wait for all threads to complete
            for handle in handles {
                handle.join().unwrap();
            }
        });
    });
    
    group.finish();
}

// Benchmark concurrent insertions for LinkedHashMapWrapper
pub fn bench_concurrent_insertions_linked_hash_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("ConcurrentInsertions");
    
    group.bench_function("LinkedHashMap", |b| {
        b.iter(|| {
            let map = Arc::new(std::sync::Mutex::new(LinkedHashMapWrapper::new()));
            
            // Spawn threads for concurrent insertions
            let handles: Vec<_> = (0..THREAD_COUNT).map(|thread_id| {
                let map = map.clone();
                thread::spawn(move || {
                    for i in 0..OPERATIONS_PER_THREAD {
                        let key = (thread_id * OPERATIONS_PER_THREAD + i) as u32;
                        let value = "test_value";
                        map.lock().unwrap().insert(key, value);
                    }
                })
            }).collect();
            
            // Wait for all threads to complete
            for handle in handles {
                handle.join().unwrap();
            }
        });
    });
    
    group.finish();
}

// Benchmark single-threaded operations for StdHashMapWrapper
pub fn bench_single_threaded_std_hash_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("SingleThreaded");
    
    group.bench_function("StdHashMap", |b| {
        b.iter(|| {
            let mut map = StdHashMapWrapper::new();
            
            // Insert operations
            for i in 0..OPERATIONS_PER_THREAD {
                map.insert(i as u32, "test_value");
            }
            
            // Read operations
            for i in 0..OPERATIONS_PER_THREAD {
                assert!(map.get(&(i as u32)).is_some());
            }
            
            // Remove operations
            for i in 0..OPERATIONS_PER_THREAD {
                map.remove(&(i as u32));
            }
        });
    });
    
    group.finish();
}

// Benchmark single-threaded operations for LinkedHashMapWrapper
pub fn bench_single_threaded_linked_hash_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("SingleThreaded");
    
    group.bench_function("LinkedHashMap", |b| {
        b.iter(|| {
            let mut map = LinkedHashMapWrapper::new();
            
            // Insert operations
            for i in 0..OPERATIONS_PER_THREAD {
                map.insert(i as u32, "test_value");
            }
            
            // Read operations
            for i in 0..OPERATIONS_PER_THREAD {
                assert!(map.get(&(i as u32)).is_some());
            }
            
            // Remove operations
            for i in 0..OPERATIONS_PER_THREAD {
                map.remove(&(i as u32));
            }
        });
    });
    
    group.finish();
}

// Benchmark read-heavy workloads for StdHashMapWrapper
pub fn bench_read_heavy_std_hash_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("ReadHeavy");
    
    group.bench_function("StdHashMap", |b| {
        b.iter(|| {
            // Pre-populate the map
            let mut map = StdHashMapWrapper::new();
            for i in 0..OPERATIONS_PER_THREAD {
                map.insert(i as u32, "test_value");
            }
            
            // Perform read-heavy operations (90% reads, 10% writes)
            for i in 0..OPERATIONS_PER_THREAD {
                if i % 10 == 0 {
                    // Write operation
                    map.insert(i as u32, "updated_value");
                } else {
                    // Read operation
                    assert!(map.get(&(i as u32)).is_some());
                }
            }
        });
    });
    
    group.finish();
}

// Benchmark read-heavy workloads for LinkedHashMapWrapper
pub fn bench_read_heavy_linked_hash_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("ReadHeavy");
    
    group.bench_function("LinkedHashMap", |b| {
        b.iter(|| {
            // Pre-populate the map
            let mut map = LinkedHashMapWrapper::new();
            for i in 0..OPERATIONS_PER_THREAD {
                map.insert(i as u32, "test_value");
            }
            
            // Perform read-heavy operations (90% reads, 10% writes)
            for i in 0..OPERATIONS_PER_THREAD {
                if i % 10 == 0 {
                    // Write operation
                    map.insert(i as u32, "updated_value");
                } else {
                    // Read operation
                    assert!(map.get(&(i as u32)).is_some());
                }
            }
        });
    });
    
    group.finish();
}

// Export the benchmark group for criterion
criterion_group!(benches, 
    bench_concurrent_insertions_std_hash_map,
    bench_concurrent_insertions_linked_hash_map,
    bench_single_threaded_std_hash_map,
    bench_single_threaded_linked_hash_map,
    bench_read_heavy_std_hash_map,
    bench_read_heavy_linked_hash_map
);

// Only run the benchmark group when this file is executed directly
criterion_main!(benches);
