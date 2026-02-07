# Page Design Guidelines

## Critical Rule: Persistent Structures Must Be Packed

**When defining structures for persistent storage (on-disk binary format), ALWAYS use `#[repr(packed)]` to ensure no padding between fields.**

### Wrong ❌

```rust
// Fields with random sizes → compiler adds padding → inconsistent disk format
#[derive(Debug, Clone, Copy)]
pub struct Page {
    pub checksum: u32,  // 4 bytes
    pub special: Special, // 2 bytes + padding
    pub glsn: u64,      // 8 bytes
    // ... more fields
}
```

### Correct ✅

```rust
// Field order by size (descending) + #[repr(packed)] = guaranteed compact layout
#[repr(packed)]
#[derive(Debug, Copy, Clone)]
pub struct PageHeader {
    pub checksum: u32,
    pub glsn: u64,
    pub plsn: u64,
    pub wal_id: u64,
    pub special: Special,  // Note: Special has 2 u16 fields = 4 bytes total
    pub flag: u16,
    pub lower: u16,
    pub upper: u16,
    pub type_: u16,
    pub myself: PageId,
}
```

## Why This Matters

1. **Cross-platform consistency**: Different compilers/platforms add different padding
2. **Serialization/deserialization**: Manual reading/writing requires predictable layout
3. **Memory-mapped I/O**: Direct memory access expects exact byte positions
4. **Version compatibility**: Adding fields won't silently corrupt data

## Best Practices

1. **Use `#[repr(packed)]`** for ALL persistent structures
2. **Order fields by size** (descending) to minimize internal padding needs
3. **Group related fields** within size constraints
4. **Test serialization** to verify exact byte layout with `std::mem::size_of::<T>()`
5. **Document field offsets** for complex structures

## Page Structure

```rust
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
    pub checksum: u32,      // Offset 0  (4 bytes)
    pub glsn: u64,          // Offset 4  (8 bytes)
    pub plsn: u64,          // Offset 12 (8 bytes)
    pub wal_id: u64,        // Offset 20 (8 bytes)
    pub special: Special,   // Offset 28 (4 bytes)
    pub flag: u16,          // Offset 32 (2 bytes)
    pub lower: u16,          // Offset 34 (2 bytes)
    pub upper: u16,          // Offset 36 (2 bytes)
    pub type_: u16,          // Offset 38 (2 bytes)
    pub myself: PageId,      // Offset 40 (8 bytes)
}

/// Page - Main page structure containing packed header
#[derive(Debug, Clone, Copy)]
pub struct Page {
    pub header: PageHeader,
}
```

## Field Meanings

| Field | Type | Description |
|-------|------|-------------|
| checksum | u32 | 32-bit CRC checksum for data integrity |
| glsn | u64 | Global Log Sequence Number for global ordering |
| plsn | u64 | Previous LSN for this page (recovery) |
| wal_id | u64 | Write-Ahead Log ID for recovery tracking |
| special | Special | 14-bit offset + 2-bit reserve (metadata) |
| flag | u16 | Page flags (dirty, allocated, etc.) |
| lower | u16 | Slot area start offset |
| upper | u16 | Data area end offset |
| type_ | u16 | PageType (Data, Index, Directory, etc.) |
| myself | PageId | Page ID of this page |

## Size Verification

```rust
#[test]
fn test_page_header_size() {
    use std::mem::size_of;
    // PageHeader must be exactly 48 bytes with #[repr(packed)]
    assert_eq!(size_of::<PageHeader>(), 48);
}
```

## Special Struct (Bit Fields Helper)

```rust
/// Special page metadata containing offset and reserve bits
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(packed)]
pub struct Special {
    /// 14-bit offset value
    pub m_offset: u16,
    /// 2-bit reserve value
    pub m_reserve: u16,
}
```

## References

- [Rust Reference: Type representations](https://doc.rust-lang.org/reference/type-layout.html)
- [repr(packed) documentation](https://doc.rust-lang.org/nomicon/other-reprs.html#reprpacked)
