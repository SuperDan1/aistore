//! Buffer pool persistence - flush policies

use std::time::Duration;

/// Flush policy strategy
#[derive(Debug, Clone)]
pub enum FlushPolicy {
    /// Flush based on dirty page ratio
    DirtyRatio { threshold: f64 },
    /// Flush based on time interval  
    Interval { duration: Duration },
    /// Flush based on number of dirty pages
    DirtyCount { count: usize },
    /// Manual flush only
    Manual,
}

impl Default for FlushPolicy {
    fn default() -> Self {
        FlushPolicy::DirtyRatio { threshold: 0.1 }
    }
}

/// Background page flusher (placeholder)
pub struct PageFlusher;

impl PageFlusher {
    /// Create a new flusher
    pub fn new(_policy: FlushPolicy, _interval: Duration) -> Self {
        Self
    }

    /// Stop the flusher
    pub fn stop(&mut self) {}
}
