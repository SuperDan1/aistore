//! Segment storage implementation

use crate::tablespace::FreeExtent;
use crate::types::Timestamp;
use crc32fast;
use std::fmt;

pub const SEGMENT_MAGIC: u32 = 0x53454721; // "SEG!"
pub const SEGMENT_VERSION: u32 = 1;
pub const SEGMENT_HEADER_SIZE: usize = 40;
pub const SEGMENT_PAGE_COUNT: u32 = 128;
pub const SEGMENT_USABLE_PAGES: u32 = SEGMENT_PAGE_COUNT - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentType {
    Data,
    Index,
    Rollback,
    System,
    Temporary,
    Undo,
}

impl fmt::Display for SegmentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SegmentType::Data => write!(f, "Data"),
            SegmentType::Index => write!(f, "Index"),
            SegmentType::Rollback => write!(f, "Rollback"),
            SegmentType::System => write!(f, "System"),
            SegmentType::Temporary => write!(f, "Temporary"),
            SegmentType::Undo => write!(f, "Undo"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SegmentHeader {
    pub magic: u32,
    pub version: u32,
    pub segment_id: u64,
    pub segment_type: u8,
    pub reserved: [u8; 7],
    pub tablespace_id: u64,
    pub extent_ptr: u64,
    pub total_pages: u64,
    pub free_pages: u64,
    pub used_pages: u64,
    pub flags: u32,
    pub checksum: u32,
}

impl SegmentHeader {
    pub fn new(segment_id: u64, segment_type: SegmentType, tablespace_id: u64) -> Self {
        Self {
            magic: SEGMENT_MAGIC,
            version: SEGMENT_VERSION,
            segment_id,
            segment_type: segment_type as u8,
            reserved: [0; 7],
            tablespace_id,
            extent_ptr: 0,
            total_pages: 0,
            free_pages: 0,
            used_pages: 0,
            flags: 0,
            checksum: 0,
        }
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.magic == SEGMENT_MAGIC
    }

    pub fn compute_checksum(&self) -> u32 {
        let mut h = *self;
        h.checksum = 0;
        let bytes = unsafe {
            std::slice::from_raw_parts(&h as *const SegmentHeader as *const u8, SEGMENT_HEADER_SIZE)
        };
        crc32fast::hash(bytes)
    }

    #[inline]
    pub fn verify_checksum(&self) -> bool {
        self.checksum == 0 || self.compute_checksum() == self.checksum
    }

    #[inline]
    pub fn init_checksum(&mut self) {
        self.checksum = self.compute_checksum();
    }
}

#[derive(Debug, Clone)]
pub struct SegmentDirEntry {
    pub segment_id: u64,
    pub segment_type: SegmentType,
    pub tablespace_id: u64,
    pub extent: FreeExtent,
    pub total_pages: u64,
    pub free_pages: u64,
    pub created_at: Timestamp,
    pub modified_at: Timestamp,
}

impl SegmentDirEntry {
    pub fn new(
        segment_id: u64,
        segment_type: SegmentType,
        tablespace_id: u64,
        extent: FreeExtent,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self {
            segment_id,
            segment_type,
            tablespace_id,
            extent,
            total_pages: 0,
            free_pages: SEGMENT_USABLE_PAGES as u64,
            created_at: now,
            modified_at: now,
        }
    }
}

#[derive(Debug)]
pub struct SegmentDirectory {
    segments: Vec<Option<SegmentDirEntry>>,
}

impl SegmentDirectory {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    pub fn create_segment(
        &mut self,
        tablespace_id: u64,
        segment_type: SegmentType,
        extent: FreeExtent,
    ) -> u64 {
        let segment_id = self.segments.len() as u64 + 1;
        while self.segments.len() < segment_id as usize {
            self.segments.push(None);
        }
        self.segments[segment_id as usize - 1] = Some(SegmentDirEntry::new(
            segment_id,
            segment_type,
            tablespace_id,
            extent,
        ));
        segment_id
    }

    pub fn get(&self, segment_id: u64) -> Option<&SegmentDirEntry> {
        if segment_id == 0 || segment_id > self.segments.len() as u64 {
            None
        } else {
            self.segments[segment_id as usize - 1].as_ref()
        }
    }

    pub fn get_mut(&mut self, segment_id: u64) -> Option<&mut SegmentDirEntry> {
        if segment_id == 0 || segment_id > self.segments.len() as u64 {
            None
        } else {
            self.segments[segment_id as usize - 1].as_mut()
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.segments.len()
    }
}

impl Default for SegmentDirectory {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub enum SegmentError {
    NotFound(u64),
    Io(std::io::Error),
    InvalidHeader,
    ChecksumMismatch,
    Full,
}

impl fmt::Display for SegmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SegmentError::NotFound(id) => write!(f, "Segment {} not found", id),
            SegmentError::Io(e) => write!(f, "I/O error: {}", e),
            SegmentError::InvalidHeader => write!(f, "Invalid segment header"),
            SegmentError::ChecksumMismatch => write!(f, "Checksum mismatch"),
            SegmentError::Full => write!(f, "Segment full"),
        }
    }
}

impl std::error::Error for SegmentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SegmentError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for SegmentError {
    #[inline]
    fn from(err: std::io::Error) -> Self {
        SegmentError::Io(err)
    }
}

pub type SegmentResult<T> = Result<T, SegmentError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment_header() {
        let header = SegmentHeader::new(1, SegmentType::Data, 1);
        assert!(header.is_valid());
        let mut h = header;
        h.init_checksum();
        assert!(h.verify_checksum());
    }

    #[test]
    fn test_segment_directory() {
        let mut dir = SegmentDirectory::new();
        let extent = FreeExtent::new(0, 0, SEGMENT_USABLE_PAGES);
        let seg_id = dir.create_segment(1, SegmentType::Data, extent);
        assert_eq!(seg_id, 1);
        assert!(dir.get(1).is_some());
        assert!(dir.get(2).is_none());
    }

    #[test]
    fn test_segment_creation_multiple() {
        let mut dir = SegmentDirectory::new();
        for i in 0..5 {
            let extent = FreeExtent::new(0, (i * 1024 * 1024) as u64, SEGMENT_USABLE_PAGES);
            let seg_id = dir.create_segment(1, SegmentType::Data, extent);
            assert_eq!(seg_id, i as u64 + 1);
        }
        assert_eq!(dir.len(), 5);
    }
}
