# LWLock - Lightweight Lock Implementations

This module provides unified interfaces for different lock implementations, allowing users to easily switch between different lock types based on their performance requirements.

## Lock Implementations

### 1. StdRwLockWrapper
- Wrapper around `std::sync::RwLock`
- Standard library implementation
- Good general-purpose choice

### 2. ParkingLotRwLockWrapper
- Wrapper around `parking_lot::RwLock`
- Faster than standard RwLock in most scenarios
- No poisoning
- Uses eventual fairness to balance throughput and fairness

### 3. ParkingLotFairRwLockWrapper
- Wrapper around `parking_lot::RwLock` configured for fairness
- Ensures threads acquire the lock in the order they requested it
- Prevents thread starvation
- Slightly slower than regular ParkingLotRwLock but more fair

## Performance Results

Performance benchmarks were run with the following configuration:
- **Read Threads**: 8
- **Write Threads**: 1
- **Operations per Thread**: 10,000
- **Total Operations per Benchmark**: 90,000 (Read-Write Mix), 80,000 (Read-Only), 10,000 (Write-Only)

### 1. Read-Write Mix (8 readers, 1 writer)

| Lock Type | Operations per Second |
|-----------|----------------------|
| StdRwLock | 12,888,000 |
| ParkingLotRwLock | 12,344,000 |
| ParkingLotFairRwLock | 11,978,000 |

### 2. Read-Only (8 readers, 0 writers)

| Lock Type | Operations per Second |
|-----------|----------------------|
| StdRwLock | 11,973,000 |
| ParkingLotRwLock | 13,726,000 |
| ParkingLotFairRwLock | 9,539,000 |

### 3. Write-Only (0 readers, 1 writer)

| Lock Type | Operations per Second |
|-----------|----------------------|
| StdRwLock | 29,000,000 |
| ParkingLotRwLock | 28,600,000 |
| ParkingLotFairRwLock | 28,400,000 |

## Usage Example

```rust
use aistore::infrastructure::lwlock::{StdRwLockWrapper, ParkingLotRwLockWrapper, ParkingLotFairRwLockWrapper};

// Create a lock (choose any implementation)
let lock = StdRwLockWrapper::new(0);
// let lock = ParkingLotRwLockWrapper::new(0);
// let lock = ParkingLotFairRwLockWrapper::new(0);

// Read data
let value = *lock.read();

// Write data
*lock.write() = 42;
```

## Performance Characteristics

- **StdRwLock**: Good general-purpose lock from the standard library, balanced performance
- **ParkingLotRwLock**: High-performance lock with eventual fairness, best for most scenarios
- **ParkingLotFairRwLock**: Fair lock that prevents starvation, best for scenarios where fairness is important

Choose the lock type based on your specific workload characteristics:
- For general use: ParkingLotRwLock (best balance of performance and fairness)
- For write-heavy workloads: ParkingLotRwLock (highest throughput)
- For scenarios where fairness is critical: ParkingLotFairRwLock (prevents thread starvation)
- For compatibility with standard library code: StdRwLock

## API

All lock implementations implement the `RwLockInterface` trait, providing a unified API:

```rust
pub trait RwLockInterface<T> {
    type ReadGuard<'a> where Self: 'a, T: 'a;
    type WriteGuard<'a> where Self: 'a, T: 'a;
    
    fn new(data: T) -> Self;
    fn read(&self) -> Self::ReadGuard<'_>;
    fn write(&self) -> Self::WriteGuard<'_>;
}
```

This allows for easy switching between lock implementations without changing the rest of your code.
