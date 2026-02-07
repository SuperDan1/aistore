// Unit tests for Page module

use super::*;
use std::mem::size_of;

#[test]
fn test_special_pack_unpack() {
    let special = Special::new(0x1FFF, 0x3);
    // 0x1FFF (8191) | (0x3 << 14) = 8191 | 49152 = 57343 (0xDFFF)
    assert_eq!(special.pack(), 0xDFFF);
}

#[test]
fn test_special_unpack() {
    // Test unpack function returns correct values
    let special = Special::unpack(0xDFFF);
    // Verify by checking pack round-trip
    assert_eq!(special.pack(), 0xDFFF);
}

#[test]
fn test_page_header_layout() {
    // Verify packed layout: all fields without padding = 48 bytes
    // checksum(4) + glsn(8) + plsn(8) + wal_id(8) + special(4) + flag(2) + lower(2) + upper(2) + type_(2) + myself(8) = 48
    assert_eq!(size_of::<PageHeader>(), 48);
    assert_eq!(PageHeader::size(), 48);
}
