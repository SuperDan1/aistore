//! LSN (Log Sequence Number)
//!
//! LSN is a 64-bit monotonically increasing counter that uniquely identifies each log record.
//! Files are just physical segmentation - file name represents the starting LSN of that file.
//! Log is conceptually one large 64-bit address space.

use std::fmt;

/// Invalid LSN constant
pub const INVALID_LSN: LSN = LSN(0);

/// Log Sequence Number - 64-bit monotonically increasing counter
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LSN(u64);

impl LSN {
    /// Create a new LSN from raw value
    #[inline]
    pub fn new(lsn: u64) -> Self {
        LSN(lsn)
    }

    /// Create LSN from raw value (alias for new)
    #[inline]
    pub fn from_raw(raw: u64) -> Self {
        LSN(raw)
    }

    /// Get raw value
    #[inline]
    pub fn raw(self) -> u64 {
        self.0
    }

    /// Get the offset (same as raw for 64-bit LSN)
    #[inline]
    pub fn offset(self) -> u64 {
        self.0
    }

    /// Get file ID (derived from LSN for file segmentation)
    #[inline]
    pub fn file_id(self) -> u32 {
        (self.0 / (1 << 30)) as u32 // Every 1GB is a new file
    }

    /// Check if LSN is valid
    #[inline]
    pub fn is_valid(self) -> bool {
        self.0 != 0
    }

    /// Get next LSN
    #[inline]
    pub fn next(self) -> LSN {
        LSN(self.0 + 1)
    }

    /// Add offset to LSN
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
        write!(f, "LSN({})", self.0)
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
        let lsn = LSN::new(100);
        assert_eq!(lsn.raw(), 100);
    }

    #[test]
    fn test_lsn_raw() {
        let lsn = LSN::new(100);
        assert_eq!(lsn.raw(), 100);
    }

    #[test]
    fn test_lsn_conversion() {
        let raw = 100u64;
        let lsn = LSN::from_raw(raw);
        assert_eq!(lsn.raw(), 100);
    }

    #[test]
    fn test_lsn_invalid() {
        let lsn = LSN::invalid();
        assert!(!lsn.is_valid());
        assert_eq!(lsn.raw(), 0);
    }

    #[test]
    fn test_lsn_comparison() {
        let lsn1 = LSN::new(100);
        let lsn2 = LSN::new(200);
        let lsn3 = LSN::new(300);

        assert!(lsn1 < lsn2);
        assert!(lsn2 < lsn3);
    }

    #[test]
    fn test_lsn_arithmetic() {
        let lsn = LSN::new(100);
        let next = lsn.next();
        assert_eq!(next.raw(), 101);

        let added = lsn + 50u64;
        assert_eq!(added.raw(), 150);

        let sub = added - lsn;
        assert_eq!(sub, 50);
    }

    #[test]
    fn test_lsn_file_id_derived() {
        let segment_size = 1u64 << 30; // 1GB
        let lsn1 = LSN::new(0);
        let lsn2 = LSN::new(segment_size);
        let lsn3 = LSN::new(segment_size * 2);

        assert_eq!(lsn1.file_id(), 0);
        assert_eq!(lsn2.file_id(), 1);
        assert_eq!(lsn3.file_id(), 2);
    }
}
