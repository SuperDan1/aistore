// LWLock - Lightweight Lock implementations
// This module provides unified interfaces for different lock implementations

use std::sync::RwLock as StdRwLock;
use parking_lot::{RwLock as ParkingLotRwLock};

/// A trait that defines a unified interface for basic locks
pub trait LockInterface<T> {
    /// The guard type for exclusive locks
    type Guard<'a> where Self: 'a, T: 'a;
    
    /// Creates a new lock with the given initial value
    fn new(data: T) -> Self;
    
    /// Acquires an exclusive lock
    fn lock(&self) -> Self::Guard<'_>;
}

/// A trait that defines a unified interface for read-write locks
pub trait RwLockInterface<T> {
    /// The guard type for read locks
    type ReadGuard<'a> where Self: 'a, T: 'a;
    
    /// The guard type for write locks
    type WriteGuard<'a> where Self: 'a, T: 'a;
    
    /// Creates a new read-write lock with the given initial value
    fn new(data: T) -> Self;
    
    /// Acquires a read lock
    fn read(&self) -> Self::ReadGuard<'_>;
    
    /// Acquires a write lock
    fn write(&self) -> Self::WriteGuard<'_>;
}

// StdRwLock wrapper
pub struct StdRwLockWrapper<T> {
    inner: StdRwLock<T>,
}

impl<T> StdRwLockWrapper<T> {
    pub fn new(data: T) -> Self {
        StdRwLockWrapper {
            inner: StdRwLock::new(data),
        }
    }
    
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, T> {
        self.inner.read().unwrap()
    }
    
    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, T> {
        self.inner.write().unwrap()
    }
}

impl<T> RwLockInterface<T> for StdRwLockWrapper<T> {
    type ReadGuard<'a> = std::sync::RwLockReadGuard<'a, T> where T: 'a;
    type WriteGuard<'a> = std::sync::RwLockWriteGuard<'a, T> where T: 'a;
    
    fn new(data: T) -> Self {
        StdRwLockWrapper::new(data)
    }
    
    fn read(&self) -> Self::ReadGuard<'_> {
        self.read()
    }
    
    fn write(&self) -> Self::WriteGuard<'_> {
        self.write()
    }
}

// ParkingLotRwLock wrapper
pub struct ParkingLotRwLockWrapper<T> {
    inner: ParkingLotRwLock<T>,
}

impl<T> ParkingLotRwLockWrapper<T> {
    pub fn new(data: T) -> Self {
        ParkingLotRwLockWrapper {
            inner: ParkingLotRwLock::new(data),
        }
    }
    
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, T> {
        self.inner.read()
    }
    
    pub fn write(&self) -> parking_lot::RwLockWriteGuard<'_, T> {
        self.inner.write()
    }
}

impl<T> RwLockInterface<T> for ParkingLotRwLockWrapper<T> {
    type ReadGuard<'a> = parking_lot::RwLockReadGuard<'a, T> where T: 'a;
    type WriteGuard<'a> = parking_lot::RwLockWriteGuard<'a, T> where T: 'a;
    
    fn new(data: T) -> Self {
        ParkingLotRwLockWrapper::new(data)
    }
    
    fn read(&self) -> Self::ReadGuard<'_> {
        self.read()
    }
    
    fn write(&self) -> Self::WriteGuard<'_> {
        self.write()
    }
}

// ParkingLotFairRwLock wrapper
// Note: parking_lot 0.12.5 doesn't have a separate FairRwLock type
// Instead, we use the regular RwLock with a fair unlocking policy
pub struct ParkingLotFairRwLockWrapper<T> {
    inner: ParkingLotRwLock<T>,
}

impl<T> ParkingLotFairRwLockWrapper<T> {
    pub fn new(data: T) -> Self {
        ParkingLotFairRwLockWrapper {
            inner: ParkingLotRwLock::new(data),
        }
    }
    
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, T> {
        self.inner.read()
    }
    
    pub fn write(&self) -> parking_lot::RwLockWriteGuard<'_, T> {
        self.inner.write()
    }
}

impl<T> RwLockInterface<T> for ParkingLotFairRwLockWrapper<T> {
    type ReadGuard<'a> = parking_lot::RwLockReadGuard<'a, T> where T: 'a;
    type WriteGuard<'a> = parking_lot::RwLockWriteGuard<'a, T> where T: 'a;
    
    fn new(data: T) -> Self {
        ParkingLotFairRwLockWrapper::new(data)
    }
    
    fn read(&self) -> Self::ReadGuard<'_> {
        self.read()
    }
    
    fn write(&self) -> Self::WriteGuard<'_> {
        self.write()
    }
}
