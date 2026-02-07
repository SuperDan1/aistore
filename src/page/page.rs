//! Page structure definition
//!
//! Defines the core Page structure used throughout the storage engine,
//! including page metadata, checksums, and LSN tracking fields.
//!
//! IMPORTANT: PageHeader uses #[repr(packed)] for persistent storage.
//! See PAGEDESIGN.md for design guidelines.

use crate::types::PageId;

/// Special page metadata containing offset and reserve bits
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(packed)]
pub struct Special {
    /// 14-bit offset value
    pub m_offset: u16,
    /// 2-bit reserve value
    pub m_reserve: u16,
}

impl Special {
    /// Creates a new Special with the given offset and reserve values
    #[inline]
    pub fn new(offset: u16, reserve: u16) -> Self {
        assert!(offset < 1 << 14, "offset must fit in 14 bits");
        assert!(reserve < 1 << 2, "reserve must fit in 2 bits");
        Self {
            m_offset: offset,
            m_reserve: reserve,
        }
    }

    /// Returns the packed 16-bit representation
    #[inline]
    pub fn pack(self) -> u16 {
        (self.m_offset & ((1 << 14) - 1)) | ((self.m_reserve & ((1 << 2) - 1)) << 14)
    }

    /// Unpacks a 16-bit value into Special struct
    #[inline]
    pub fn unpack(value: u16) -> Self {
        Self {
            m_offset: value & ((1 << 14) - 1),
            m_reserve: (value >> 14) & ((1 << 2) - 1),
        }
    }
}

/// Page type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum PageType {
    /// Invalid or unset page type
    Invalid = 0,
    /// Data page containing user data
    Data = 1,
    /// Index page containing index entries
    Index = 2,
    /// Directory page for page management
    Directory = 3,
    /// Free page (unused)
    Free = 4,
    /// Internal system page
    System = 5,
}

/// PageHeader - Packed persistent header for storage engine pages
///
/// **CRITICAL**: Uses `#[repr(packed)]` to ensure NO padding between fields.
/// This is required for persistent storage to guarantee consistent binary format.
///
/// Layout (48 bytes total, no padding):
/// ```text
/// Offset  Size  Field
///   0     4     checksum
///   4     8     glsn
///  12     8     plsn
///  20     8     wal_id
///  28     4     special (2 offset + 2 reserve)
///  32     2     flag
///  34     2     lower
///  36     2     upper
///  38     2     type_
///  40     8     myself
/// ```
#[repr(packed)]
#[derive(Debug, Copy, Clone)]
pub struct PageHeader {
    /// 32-bit checksum for data integrity
    pub checksum: u32,
    /// Global LSN (Log Sequence Number) for global ordering
    pub glsn: u64,
    /// Previous LSN for this page
    pub plsn: u64,
    /// Write-Ahead Log ID for recovery
    pub wal_id: u64,
    /// Special metadata (14-bit offset + 2-bit reserve)
    pub special: Special,
    /// Page flags
    pub flag: u16,
    /// Lower bound offset (slot area start)
    pub lower: u16,
    /// Upper bound offset (data area end)
    pub upper: u16,
    /// Page type
    pub type_: u16,
    /// Page ID of this page
    pub myself: PageId,
}

impl PageHeader {
    /// Creates a new uninitialized PageHeader
    #[inline]
    pub fn new() -> Self {
        Self {
            checksum: 0,
            glsn: 0,
            plsn: 0,
            wal_id: 0,
            special: Special::new(0, 0),
            flag: 0,
            lower: 0,
            upper: 0,
            type_: 0,
            myself: 0,
        }
    }

    /// Returns the size of the page header in bytes (48 bytes packed)
    #[inline]
    pub fn size() -> usize {
        48
    }

    /// Returns the available data space in the page
    #[inline]
    pub fn available_space(&self) -> usize {
        crate::types::PAGE_SIZE - self.upper as usize
    }

    /// Returns the slot area size
    #[inline]
    pub fn slot_space(&self) -> usize {
        self.lower as usize
    }
}

impl Default for PageHeader {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

/// Core Page structure for storage engine
///
/// Contains a packed PageHeader for persistent storage compatibility.
#[derive(Debug, Clone, Copy)]
pub struct Page {
    /// Packed page header for persistent storage
    pub header: PageHeader,
}

impl Page {
    /// Creates a new uninitialized Page
    #[inline]
    pub fn new() -> Self {
        Self {
            header: PageHeader::new(),
        }
    }

    /// Returns the size of the page header in bytes
    #[inline]
    pub fn header_size() -> usize {
        PageHeader::size()
    }

    /// Returns the available data space in the page
    #[inline]
    pub fn available_space(&self) -> usize {
        self.header.available_space()
    }

    /// Returns the slot area size
    #[inline]
    pub fn slot_space(&self) -> usize {
        self.header.slot_space()
    }
}

impl Default for Page {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    include!("tests.rs");
}
