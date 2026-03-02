//! Double Write Buffer - prevents partial page writes

use crate::types::PageId;

/// DoubleWriteBuffer placeholder
pub struct DoubleWriteBuffer {
    capacity: usize,
}

impl DoubleWriteBuffer {
    /// Create a new doublewrite buffer
    pub fn new(capacity: usize) -> Self {
        Self { capacity }
    }

    /// Prepare a page for writing
    pub fn prepare(&self, _page_id: PageId, _data: &[u8]) -> bool {
        true
    }

    /// Flush all to disk
    pub fn flush_all(&self) -> Result<(), crate::buffer::BufferError> {
        Ok(())
    }
}
