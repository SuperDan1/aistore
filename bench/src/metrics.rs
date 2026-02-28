//! Metrics collection module

use std::sync::atomic::{AtomicU64, Ordering};

/// Metrics collector for benchmark
pub struct Metrics {
    ops_count: AtomicU64,
    latency_sum: AtomicU64,
    latency_max: AtomicU64,
    errors: AtomicU64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            ops_count: AtomicU64::new(0),
            latency_sum: AtomicU64::new(0),
            latency_max: AtomicU64::new(0),
            errors: AtomicU64::new(0),
        }
    }

    pub fn record_op(&self, latency_ns: u64) {
        self.ops_count.fetch_add(1, Ordering::Relaxed);
        self.latency_sum.fetch_add(latency_ns, Ordering::Relaxed);

        let mut current = self.latency_max.load(Ordering::Relaxed);
        while latency_ns > current {
            match self.latency_max.compare_exchange(
                current,
                latency_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(v) => current = v,
            }
        }
    }

    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn total_ops(&self) -> u64 {
        self.ops_count.load(Ordering::Relaxed)
    }

    pub fn total_latency_ns(&self) -> u64 {
        self.latency_sum.load(Ordering::Relaxed)
    }

    pub fn max_latency_ns(&self) -> u64 {
        self.latency_max.load(Ordering::Relaxed)
    }

    pub fn total_errors(&self) -> u64 {
        self.errors.load(Ordering::Relaxed)
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
