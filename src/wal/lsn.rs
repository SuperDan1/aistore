//! LSN (Log Sequence Number)
//!
//! LSN uniquely identifies each log record in WAL.
//! Structure: [File ID (16 bits)][Offset within file (48 bits)] = 64 bits

use std::fmt;

/// Maximum file ID
const MAX_FILE_ID: u16 = u16::MAX;
/// Offset mask (48 bits)
const OFFSET_MASK: u64 = 0xFFFFFFFFFFFF;

/// Invalid LSN constant
pub const INVALID_LSN: LSN = LSN(0);

/// Log Sequence Number
/// [File ID (16 bits)][Offset within file (48 bits)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LSN(u64);

impl LSN {
    /// Create a new LSN from file ID and offset
    #[inline]
    pub fn new(file_id: u16, offset: u64) -> Self {
        debug_assert!(offset < (1 << 48), "offset exceeds 48 bits");
        let file_id = (file_id as u64) << 48;
        let offset = offset & OFFSET_MASK;
        LSN(file_id | offset)
    }

    /// Create LSN from raw value
    #[inline]
    pub fn from_raw(raw: u64) -> Self {
        LSN(raw)
    }

    /// Get the file ID part
    #[inline]
    pub fn file_id(self) -> u16 {
        (self.0 >> 48) as u16
    }

    /// Get the offset within the file
    #[inline]
    pub fn offset(self) -> u64 {
        self.0 & OFFSET_MASK
    }

    /// Get raw value
    #[inline]
    pub fn raw(self) -> u64 {
        self.0
    }

    /// Check if LSN is valid
    #[inline]
    pub fn is_valid(self) -> bool {
        self.0 != 0
    }

    /// Get next LSN in the same file
    #[inline]
    pub fn next(self) -> LSN {
        LSN(self.0 + 1)
    }

    /// Add offset to LSN (same file)
    #[inline]
    pub fn add_offset(self, offset: u64) -> LSN {
        LSN(self.0 + offset)
    }

    /// Create an invalid LSN
    pub fn invalid() -> Self {
        INVALID_LSN
    }
}

impl Default for LSN {
    fn default() -> Self {
        INVALID_LSN
    }
}

impl fmt::Display for LSN {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LSN({}:{})", self.file_id(), self.offset())
    }
}

impl std::ops::Add<u64> for LSN {
    type Output = LSN;

    fn add(self, rhs: u64) -> LSN {
        LSN(self.0 + rhs)
    }
}

impl std::ops::Sub<u64> for LSN {
    type Output = LSN;

    fn sub(self, rhs: u64) -> LSN {
        LSN(self.0 - rhs)
    }
}

impl std::ops::Sub<LSN> for LSN {
    type Output = u64;

    fn sub(self, rhs: LSN) -> u64 {
        self.0 - rhs.0
    }
}

/// Convert from u64
impl From<u64> for LSN {
    fn from(raw: u64) -> Self {
        LSN(raw)
    }
}

/// Convert to u64
impl From<LSN> for u64 {
    fn from(lsn: LSN) -> Self {
        lsn.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsn_creation() {
        let lsn = LSN::new(1, 100);
        assert_eq!(lsn.file_id(), 1);
        assert_eq!(lsn.offset(), 100);
    }

    #[test]
    fn test_lsn_raw() {
        let lsn = LSN::new(1, 100);
        assert_eq!(lsn.raw(), (1u64 << 48) | 100);
    }

    #[test]
    fn test_lsn_conversion() {
        let raw = (1u64 << 48) | 100;
        let lsn = LSN::from_raw(raw);
        assert_eq!(lsn.file_id(), 1);
        assert_eq!(lsn.offset(), 100);
    }

    #[test]
    fn test_lsn_invalid() {
        let lsn = LSN::invalid();
        assert!(!lsn.is_valid());
        assert_eq!(lsn.file_id(), 0);
        assert_eq!(lsn.offset(), 0);
    }

    #[test]
    fn test_lsn_comparison() {
        let lsn1 = LSN::new(1, 100);
        let lsn2 = LSN::new(1, 200);
        let lsn3 = LSN::new(2, 50);

        assert!(lsn1 < lsn2);
        assert!(lsn2 < lsn3);
    }

    #[test]
    fn test_lsn_arithmetic() {
        let lsn = LSN::new(1, 100);
        let next = lsn.next();
        assert_eq!(next.file_id(), 1);
        assert_eq!(next.offset(), 101);

        let added = lsn + 50u64;
        assert_eq!(added.offset(), 150);

        let sub = added - lsn;
        assert_eq!(sub, 50);
    }
}
