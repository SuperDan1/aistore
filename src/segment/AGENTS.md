# segment/ AGENTS.md

**Generated:** 64MB segment storage with page mapping

## OVERVIEW
Segment-level storage: 64MB fixed-size segments, page-to-segment mapping, segment header management. BufferPool integration.

## WHERE TO LOOK
| Task | File | Notes |
|------|------|-------|
| Segment ops | segment.rs | `SegmentDirectory`, `SegmentHeader` |
| Page mapping | mod.rs | `SegmentManager` with BufferPool |
| Tests | tests.rs | File I/O, cleanup utilities |

## MODULE STRUCTURE
```
segment/
├── mod.rs           # SegmentManager, page operations
├── segment.rs       # SegmentDirectory, SegmentHeader
└── tests.rs        # Integration tests
```

## PAGE LAYOUT
```
Segment (64MB = 8192 pages × 8KB)
├── Page 0:     SegmentHeader (metadata)
├── Page 1-N:   Data pages
└── Free space tracking
```

## KEY STRUCTURES

### SegmentHeader
```rust
// First page of each segment
struct SegmentHeader {
    segment_id: u64,
    first_page: u64,
    page_count: u32,
    checksum: u32,
}
```

### SegmentDirectory
```rust
// Maps logical page → segment + offset
SegmentDirectory {
    entries: Vec<SegmentEntry>,
    free_pages: u64,
}
```

## ANTI-PATTERNS
- NEVER access segment without holding lock
- NEVER modify segment header directly
- NEVER skip checksum verification on read

## COMMANDS
```bash
cargo test --lib segment    # Segment tests
cargo fmt                 # Required
```
