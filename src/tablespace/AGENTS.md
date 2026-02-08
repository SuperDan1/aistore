# tablespace/ AGENTS.md

**Generated:** Segment-page storage module

## OVERVIEW
InnoDB-style segment-page storage with free extent list management, multi-file tablespaces, CRC32 checksums

## WHERE TO LOOK
| Task | File | Notes |
|------|------|-------|
| Tablespace creation | mod.rs | `TablespaceManager::create_tablespace()` |
| Segment ops | segment.rs | `SegmentDirectory` patterns |
| Page allocation | mod.rs | `allocate_extent()`, free list lookup |
| Error handling | mod.rs | `TablespaceResult<T>` pattern |

## MODULE STRUCTURE
```
tablespace/
├── mod.rs           # TablespaceManager, FreeExtentList, headers
├── segment.rs        # SegmentDirectory, SegmentHeader
├── buffered.rs      # BufferPool integration
└── DESIGN.md       # Architecture docs
```

## CONVENTIONS (deviations from root)

### Error Propagation
- Use `TablespaceResult<T>` not standard `Result`
- Convert VFS errors: `TablespaceError::Io(e) → TablespaceError::NotFound(0)`

### Free List Management
```rust
// FreeExtentList tracks available extents for fast allocation
free_lists: RwLock<HashMap<u64, Arc<RwLock<FreeExtentList>>>
```

### Checksum Pattern
```rust
header.compute_checksum()  // Exclude checksum field
header.verify_checksum()   // Allow zero for uninitialized
```

## ANTI-PATTERNS
- NEVER skip checksum verification on disk reads
- NEVER mix `TablespaceResult` with `Result` in same function
- NEVER leak raw pointers from extent allocation

## COMMANDS
```bash
cargo test --lib tablespace  # Module tests
cargo bench hash_bench    # Hash performance
cargo fmt              # Required before commit
```
