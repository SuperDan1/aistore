//! Buffered Tablespace/Segment operations with BufferPool integration

use super::{
    ExtentHeader, FreeExtent, SegmentDirEntry, SegmentDirectory, SegmentError, SegmentHeader,
    SegmentResult, SegmentType, TablespaceError, TablespaceManager,
};
use crate::buffer::{BufferDesc, BufferError, BufferMgr};
use crate::page::Page;
use crate::types::PageId;
use crate::vfs::VfsInterface;
use std::path::PathBuf;
use std::sync::Arc;

// BufferedTablespace integrates BufferPool with Tablespace operations
pub struct BufferedTablespace {
    tablespace_id: u64,
    buffer_mgr: Arc<BufferMgr>,
    tablespace_mgr: TablespaceManager,
}

impl BufferedTablespace {
    pub fn new(tablespace_id: u64, buffer_mgr: Arc<BufferMgr>, data_dir: PathBuf) -> Self {
        Self {
            tablespace_id,
            buffer_mgr,
            tablespace_mgr: TablespaceManager::new(data_dir),
        }
    }

    pub fn create_tablespace(&mut self, name: &str) -> Result<u64, TablespaceError> {
        self.tablespace_mgr
            .create_tablespace(name, Default::default())
    }

    pub fn open_tablespace(&self, name: &str) -> Result<u64, TablespaceError> {
        self.tablespace_mgr.open_tablespace(name)
    }

    pub fn create_segment(
        &mut self,
        tablespace_id: u64,
        segment_type: SegmentType,
    ) -> Result<u64, TablespaceError> {
        // Allocate extent from tablespace
        let extent = self
            .tablespace_mgr
            .allocate_extent(tablespace_id)
            .map_err(|_| TablespaceError::NoSpace)?;

        // Initialize extent header
        let mut file = self
            .tablespace_mgr
            .get_file(tablespace_id, extent.file_id)
            .map_err(|_| TablespaceError::InvalidFileHeader)?;

        let mut extent_header =
            ExtentHeader::new(tablespace_id, extent.file_id, extent.extent_offset);
        super::write_extent_header(&mut file, extent.extent_offset, &extent_header)
            .map_err(|_| TablespaceError::InvalidExtentHeader)?;

        // Note: Would need to add segment creation to TablespaceManager
        // For now, return Ok with segment_id
        Ok(1) // Placeholder
    }

    pub fn allocate_page(&mut self, segment_id: u64) -> Result<PageId, TablespaceError> {
        Ok(1) // Placeholder
    }

    pub fn get_page(&mut self, page_id: PageId) -> Result<&mut Page, BufferError> {
        self.buffer_mgr.get_page(page_id)
    }

    pub fn mark_dirty(&mut self, page_id: PageId) {
        self.buffer_mgr.mark_dirty(page_id);
    }

    pub fn unpin_page(&mut self, page_id: PageId) -> Result<(), BufferError> {
        self.buffer_mgr.unpin_page(page_id)
    }

    pub fn flush_all(&mut self) -> Result<(), BufferError> {
        self.buffer_mgr.flush_all()
    }
}
