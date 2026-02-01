# AGENTS.md

## Build Commands

### Build Project
```bash
cargo build --release    # Optimized release build
cargo build              # Debug build
```

### Run Project
```bash
cargo run --release
```

### Run Tests
```bash
cargo test                            # Run all tests
cargo test <TESTNAME>                  # Run specific test (e.g., cargo test fnv1a_hash_consistency)
cargo test --lib                       # Run library tests only
cargo test -- --test-threads=1         # Single thread
cargo test -- --nocapture              # Show println output
```

### Run Benchmarks
```bash
cargo bench                           # Run all benchmarks
cargo bench --bench <BENCH_NAME>      # Run specific benchmark (e.g., cargo bench --bench hash_bench)
```

### Code Quality
```bash
cargo fmt                             # Format code (required before committing)
cargo clippy                          # Run linter
```

---

## Code Style Guidelines

### Rust Edition
- Uses **Rust 2024** edition
- Toolchain: cargo 1.92.0+

### Imports
- Imports sorted alphabetically by rustfmt (default behavior)
- Group related imports together
- Always use `use crate::...` for internal modules (never `super::...`)

Example:
```rust
use std::collections::LinkedList;
use std::sync::Arc;
use parking_lot::RwLock;
use crate::types::BlockId;
use crate::buffer::BufferMgr;
```

### Formatting
- **MUST** run `cargo fmt` before committing
- rustfmt 1.8.0-stable rules
- Line length: 100 characters (default)

### Naming Conventions
- **Types**: `PascalCase` (e.g., `BlockId`, `BufferMgr`, `VfsResult<T>`)
- **Enums**: `PascalCase` variants (e.g., `BlockType::Data`, `NodeType::Hot`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `BLOCK_SIZE`, `CACHELINE_SIZE`)
- **Functions**: `snake_case` (e.g., `fnv1a_hash`, `create_dir`)
- **Traits**: `PascalCase` + `Interface` suffix (e.g., `RwLockInterface<T>`)
- **Wrappers**: `<Type>Wrapper` suffix (e.g., `StdRwLockWrapper<T>`)
- **Booleans**: `is_`, `has_`, `can_` prefixes
- **Mutators**: `set_`, `insert_`, `add_` prefixes

### Error Handling
- Custom errors: `#[derive(Debug)]` enum with `Display` and `std::error::Error`
- Module-specific `Result<T>` alias (e.g., `VfsResult<T> = Result<T, VfsError>`)
- Implement `From` for conversions (e.g., `From<std::io::Error>`)
- **NEVER** use `.unwrap()` - always propagate with `?`

Example:
```rust
#[derive(Debug)]
pub enum VfsError {
    NotFound(String),
    IoError(std::io::Error),
}

impl std::fmt::Display for VfsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VfsError::NotFound(path) => write!(f, "File not found: {}", path),
            VfsError::IoError(err) => write!(f, "I/O error: {}", err),
        }
    }
}

impl std::error::Error for VfsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VfsError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for VfsError {
    fn from(err: std::io::Error) -> Self {
        VfsError::IoError(err)
    }
}

pub type VfsResult<T> = Result<T, VfsError>;
```

### Documentation
- Module docs: `//!` at file top
- Public API docs: `///`
- Document purpose, behavior, parameters, returns
- Document unsafe blocks with safety invariants

### Module Structure
- `mod.rs` as module root
- Submodules: `pub mod <name>;` in parent
- Re-export common types: `pub use crate::vfs::error::{VfsError, VfsResult};`
- Tests: `#[cfg(test)] mod tests { include!("tests.rs"); }`

### Memory & Concurrency
- **Global allocator**: jemalloc via `#[global_allocator]`
- Prefer `parking_lot` over `std::sync` for performance
- Use `RwLock<T>` for read-heavy workloads
- Align critical structures to cache lines: `#[repr(align(64))]` on x86_64
- Use `unsafe` sparingly, document invariants

### Testing
- Unit tests: module `tests.rs` files
- Integration tests: `/tests/` directory
- Descriptive names: `test_fnv1a_hash_consistency`
- Use `assert_eq!`, `assert_ne!` for clarity

### Benchmarking
- Use `criterion`
- Benchmarks: `src/<module>/bench.rs`
- Configure in `Cargo.toml`: `[[bench]] name = "<bench_name>" harness = false`
- Use `black_box()` to prevent optimizations
- Group: `c.benchmark_group("ShortStrings")`

### Dependencies
- Core: `jemallocator`, `rand`, `xxhash-rust`, `simplehash`, `crc32fast`, `crc-fast`
- Concurrency: `parking_lot`
- Data structures: `linked-hash-map`
- System: `libc`
- Dev: `criterion`

### Code Organization
```
src/
├── main.rs           # Binary entry
├── lib.rs           # Library entry, re-exports
├── types.rs         # Global types/constants
├── buffer/          # Buffer management
├── heap/            # Heap storage
├── index/           # Index structures
├── vfs/             # Virtual filesystem
├── infrastructure/   # Low-level utilities
│   ├── hash/       # Hash algorithms
│   ├── lwlock/     # Lock implementations
│   └── hash_table/ # Hash map wrappers
├── tablespace/      # Tablespace management
├── segment/         # Segment storage
├── controlfile/     # Control file
└── lock/            # Lock management
```

### Performance
- Prefer FNV-1a hash (best speed/distribution balance)
- Use parking_lot locks for performance-critical code
- Align data structures to cache lines (64B x86_64, 128B ARM)
- Profile with `cargo bench` before optimizing

### Before Contributing
1. `cargo fmt` (required)
2. `cargo clippy` and fix warnings
3. `cargo test` and ensure all pass
4. Add tests for new functionality
5. Update docs for public APIs
6. Benchmark if performance impact expected
