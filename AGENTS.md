# AISTORE KNOWLEDGE BASE

**Generated:** 2026-02-08
**Project:** High-performance storage engine (Rust 2024)

## OVERVIEW
Aistore is a Rust storage engine with segment-page storage, buffer pool caching, and multi-hash algorithm support. Optimized for performance with jemalloc and parking_lot.

## STRUCTURE
```
aistore/
├── src/
│   ├── infrastructure/    # Hash algos, hash tables, lwlock (PRIMITIVES)
│   ├── buffer/            # LRU buffer pool (CACHING)
│   ├── vfs/               # Virtual filesystem (ABSTRACTION)
│   ├── page/              # Page structure (STORAGE UNIT)
│   ├── segment/           # 64MB segments (STORAGE LAYOUT)
│   ├── tablespace/        # Tablespace management (ORG)
│   ├── heap/              # Heap file organization
│   ├── index/             # B-tree indexes
│   ├── table/             # Table metadata, columns (DATA MODEL)
│   ├── catalog/           # System catalog, table persistence
│   └── storage/           # Storage Engine API (bench interface)
├── bench/                 # Benchmark tool
└── AGENTS.md
```

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| Buffer pool | `buffer/` | LRU-K, atomic pin counting, dirty tracking |
| Hashing | `infrastructure/hash/` | FNV-1a, XXH64, CRC32, CityHash benchmarks |
| VFS abstraction | `vfs/` | Trait-based, posix operations |
| Storage layout | `tablespace/`, `segment/` | InnoDB-style, free extent lists |
| Heap storage | `heap/` | Heap table implementation |
| Storage API | `storage.rs` | Table-oriented API for benchmarks |
| Concurrency | `infrastructure/lwlock/` | Custom lightweight locks |

## CONVENTIONS (Deviations from Standard Rust)

### Imports
```rust
use std::sync::Arc;
use parking_lot::RwLock;
use crate::types::BlockId;        // Always crate::
use crate::buffer::BufferMgr;     // Never super::
```

### Naming
| Type | Pattern | Example |
|------|---------|---------|
| Constants | `SCREAMING_SNAKE_CASE` | `BLOCK_SIZE`, `CACHELINE_SIZE` |
| Traits | `PascalCase` + `Interface` | `RwLockInterface<T>` |
| Wrappers | `<Type>Wrapper` | `StdRwLockWrapper<T>` |
| Booleans | `is_`, `has_`, `can_` | `is_dirty()`, `has_pin()` |

### Error Handling
```rust
// Custom Result alias per module
pub type VfsResult<T> = Result<T, VfsError>;

// From conversions required
impl From<std::io::Error> for VfsError { ... }

// NEVER .unwrap() - always ?
```

### Atomic State (Buffer Pool Pattern)
```rust
// 64-bit layout: DIRTY_BIT at bit 0, PIN_COUNT at bits 8-63
const DIRTY_BIT: u64 = 1 << 0;
const PIN_COUNT_SHIFT: u8 = 8;
```

### Memory & Concurrency
- **Global allocator**: jemalloc via `#[global_allocator]`
- **Cache line alignment**: `#[repr(align(64))]` on x86_64
- **Prefer parking_lot** over std::sync

### Non-Standard Patterns (Deviations)
- **Rust 2024 Edition**: `edition = "2024"` in Cargo.toml (bleeding-edge, not 2021)
- **Minimal CI**: No clippy, fmt check, or caching in `.github/workflows/rust.yml`
- **Global allocator**: jemalloc via `#[global_allocator]`
- **Cache line alignment**: `#[repr(align(64))]` on x86_64
- **Prefer parking_lot** over std::sync

## ANTI-PATTERNS (THIS PROJECT)

### CRITICAL - Never Do
- Never use `.unwrap()` in production code
- Never modify buffer state without atomic operations
- Never skip checksum verification on disk reads
- Never mix module-specific `Result` types with standard `Result`

### WARNING
- Never bypass LRU for hot paths
- Never leak raw pointers from extent allocation

## CODE QUALITY (Pre-Commit)

```bash
cargo fmt                              # Required before commit
cargo clippy --lib --tests --benches  # Fix all warnings
cargo test --lib                       # All library tests pass
cargo bench                            # Profile if performance impact
```

## PERFORMANCE

| Component | Pattern | Notes |
|-----------|---------|-------|
| Hashing | FNV-1a | Best speed/distribution balance |
| Locking | parking_lot::RwLock | Read-heavy workloads |
| Alignment | 64B x86_64, 128B ARM | Critical structures |
| Allocator | jemalloc | Global, configured |

## BUILD & TEST

```bash
cargo build --release                  # Optimized build
cargo test --lib                       # Library tests
cargo test -- --test-threads=1         # Single-threaded tests
cargo bench --bench hash_bench        # Hash performance
```

## MODULE-SPECIFIC GUIDES/AGENTS.md

- [buffer](src/buffer/AGENTS.md) - Buffer pool patterns
- [vfs/AGENTS.md](src/vfs/AGENTS.md) - VFS interface patterns
- [tablespace/AGENTS.md](src/tablespace/AGENTS.md) - Segment-page storage
- [table/AGENTS.md](src/table/AGENTS.md) - Table metadata & columns
- [catalog/AGENTS.md](src/catalog/AGENTS.md) - System catalog

## STORAGE ENGINE API

The `storage.rs` module provides a simple table-oriented API:

```rust
use aistore::{StorageEngine, Filter, Value};

// Create storage engine
let mut storage = StorageEngine::new("./data").unwrap();

// Create table
storage.create_table("users", vec![
    Column::new("id".to_string(), ColumnType::Int64, false, 0),
    Column::new("name".to_string(), ColumnType::Varchar(100), false, 1),
]).unwrap();

// Insert
storage.insert("users", vec![
    Value::Int64(1),
    Value::Varchar("Alice".to_string()),
]).unwrap();

// Scan with filter
let filter = Filter {
    column: "id".to_string(),
    Value::Int64(1),
};
let results = storage.scan("users", Some(filter)).unwrap();
```

## BENCHMARK TOOL

Run benchmarks with `bench/` directory:

```bash
cd bench
cargo run --release -- \
    -t 4          # threads \
    -d 10         # duration (seconds) \
    -s read_only  # scenario \
    --rows 10000  # initial rows
```

Available scenarios:
- `read_only` - Full table scan
- `point_select` - Point query by primary key
- `insert` - Single row insert
- `update_index` - Update indexed column
- `bulk_insert` - Batch insert
