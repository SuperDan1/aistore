use criterion::{criterion_group, criterion_main, Criterion};  
use std::sync::Arc;
use std::thread;

// Reference the main crate
extern crate aistore;

// Import the lock wrappers from the main crate
use aistore::infrastructure::lwlock::{StdRwLockWrapper, ParkingLotFairRwLockWrapper, ParkingLotRwLockWrapper};

// Test configuration
const READ_THREADS: usize = 8;
const WRITE_THREADS: usize = 1;
const OPERATIONS_PER_THREAD: usize = 10_000;

// Generic lock benchmark function
fn bench_lock_scenario<T: Send + Sync + 'static>(
    b: &mut criterion::Bencher,
    create_lock: fn(usize) -> Arc<T>,
    read_op: fn(&T) -> usize,
    write_op: fn(&T, usize)
) {
    b.iter(|| {
        let lock = create_lock(0);
        
        // Spawn read threads
        let read_handles: Vec<_> = (0..READ_THREADS).map(|_| {
            let lock = lock.clone();
            thread::spawn(move || {
                for _ in 0..OPERATIONS_PER_THREAD {
                    read_op(&lock);
                }
            })
        }).collect();
        
        // Spawn write threads
        let write_handles: Vec<_> = (0..WRITE_THREADS).map(|_| {
            let lock = lock.clone();
            thread::spawn(move || {
                for i in 0..OPERATIONS_PER_THREAD {
                    write_op(&lock, i);
                }
            })
        }).collect();
        
        // Wait for all threads to complete
        for handle in read_handles {
            handle.join().unwrap();
        }
        for handle in write_handles {
            handle.join().unwrap();
        }
    });
}

// Benchmark read-write mix scenario
pub fn bench_rw_mix(c: &mut Criterion) {
    let mut group = c.benchmark_group("ReadWriteMix");
    
    // StdRwLock
    group.bench_function("StdRwLock", |b| {
        bench_lock_scenario(
            b,
            |value| Arc::new(StdRwLockWrapper::new(value)),
            |lock: &StdRwLockWrapper<usize>| *lock.read(),
            |lock: &StdRwLockWrapper<usize>, value| *lock.write() = value
        )
    });
    
    // ParkingLotRwLock
    group.bench_function("ParkingLotRwLock", |b| {
        bench_lock_scenario(
            b,
            |value| Arc::new(ParkingLotRwLockWrapper::new(value)),
            |lock: &ParkingLotRwLockWrapper<usize>| *lock.read(),
            |lock: &ParkingLotRwLockWrapper<usize>, value| *lock.write() = value
        )
    });
    
    // ParkingLotFairRwLock
    group.bench_function("ParkingLotFairRwLock", |b| {
        bench_lock_scenario(
            b,
            |value| Arc::new(ParkingLotFairRwLockWrapper::new(value)),
            |lock: &ParkingLotFairRwLockWrapper<usize>| *lock.read(),
            |lock: &ParkingLotFairRwLockWrapper<usize>, value| *lock.write() = value
        )
    });
    
    group.finish();
}

// Benchmark read-only scenario
pub fn bench_read_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("ReadOnly");
    
    // StdRwLock
    group.bench_function("StdRwLock", |b| {
        bench_lock_scenario(
            b,
            |value| Arc::new(StdRwLockWrapper::new(value)),
            |lock: &StdRwLockWrapper<usize>| *lock.read(),
            |_: &StdRwLockWrapper<usize>, _| {}
        )
    });
    
    // ParkingLotRwLock
    group.bench_function("ParkingLotRwLock", |b| {
        bench_lock_scenario(
            b,
            |value| Arc::new(ParkingLotRwLockWrapper::new(value)),
            |lock: &ParkingLotRwLockWrapper<usize>| *lock.read(),
            |_: &ParkingLotRwLockWrapper<usize>, _| {}
        )
    });
    
    // ParkingLotFairRwLock
    group.bench_function("ParkingLotFairRwLock", |b| {
        bench_lock_scenario(
            b,
            |value| Arc::new(ParkingLotFairRwLockWrapper::new(value)),
            |lock: &ParkingLotFairRwLockWrapper<usize>| *lock.read(),
            |_: &ParkingLotFairRwLockWrapper<usize>, _| {}
        )
    });
    
    group.finish();
}

// Benchmark write-only scenario
pub fn bench_write_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("WriteOnly");
    
    // StdRwLock
    group.bench_function("StdRwLock", |b| {
        bench_lock_scenario(
            b,
            |value| Arc::new(StdRwLockWrapper::new(value)),
            |_: &StdRwLockWrapper<usize>| 0,
            |lock: &StdRwLockWrapper<usize>, value| *lock.write() = value
        )
    });
    
    // ParkingLotRwLock
    group.bench_function("ParkingLotRwLock", |b| {
        bench_lock_scenario(
            b,
            |value| Arc::new(ParkingLotRwLockWrapper::new(value)),
            |_: &ParkingLotRwLockWrapper<usize>| 0,
            |lock: &ParkingLotRwLockWrapper<usize>, value| *lock.write() = value
        )
    });
    
    // ParkingLotFairRwLock
    group.bench_function("ParkingLotFairRwLock", |b| {
        bench_lock_scenario(
            b,
            |value| Arc::new(ParkingLotFairRwLockWrapper::new(value)),
            |_: &ParkingLotFairRwLockWrapper<usize>| 0,
            |lock: &ParkingLotFairRwLockWrapper<usize>, value| *lock.write() = value
        )
    });
    
    group.finish();
}

// Export the benchmark group for criterion
criterion_group!(benches, bench_rw_mix, bench_read_only, bench_write_only);

// Only run the benchmark group when this file is executed directly
criterion_main!(benches);
