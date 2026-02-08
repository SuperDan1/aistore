# infrastructure/ AGENTS.md

**Generated:** Low-level primitives - hashing, locks, concurrent data structures

## OVERVIEW
Performance-critical primitives: FNV-1a/XXH64/CRC32 hashing, lightweight locks, concurrent hash tables. All with inline benchmarks.

## WHERE TO LOOK
| Task | Subdir | Notes |
|------|--------|-------|
| Hash algos | `hash/` | FNV-1a (fast), XXH64, CRC32, CityHash benchmarks |
| Lock impls | `lwlock/` | Custom lightweight lock implementations |
| Hash tables | `hash_table/` | Concurrent hash map wrappers |

## MODULE STRUCTURE
```
infrastructure/
├── mod.rs           # Module exports
├── hash/           # 5 hash algos, benchmarks
│   ├── mod.rs
│   ├── tests.rs
│   └── bench.rs
├── hash_table/     # Concurrent hash map, benchmarks
│   ├── mod.rs
│   ├── tests.rs
│   └── bench.rs
└── lwlock/         # Lightweight locks, benchmarks
    ├── mod.rs
    └── bench.rs
```

## BENCHMARKS
```bash
cargo bench --bench hash_bench        # Hash algorithms
cargo bench --bench hash_table_bench  # Concurrent hash maps
cargo bench --bench lwlock_bench      # Lock performance
```

## CONVENTIONS

### Hash Selection
```rust
// FNV-1a: Best speed/distribution for general use
// XXH64: 64-bit hash for large datasets
// CRC32: Checksumming, not cryptographic
// CityHash64: Google-optimized for short strings
```

### Lock Patterns
```rust
// parking_lot::RwLock for read-heavy workloads
// Custom lwlock for specialized scenarios
// Atomic operations for simple state
```

## ANTI-PATTERNS
- NEVER use cryptographic hashes (SHA for non-security-256) use
- NEVER block in benchmarks (use proper synchronization)
- NEVER skip benchmark comparison against baseline

## KEY BENCHMARKS
- `hash_bench`: FNV-1a vs XXH64 vs CRC32 vs CityHash64
- `hash_table_bench`: Concurrent access patterns
- `lwlock_bench`: Lock contention scenarios
