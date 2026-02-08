# buffer/ AGENTS.md

**Generated:** Buffer Pool with LRU replacement, dirty page tracking, VFS I/O

## OVERVIEW
In-memory page caching with hash-table lookup, LRU-K style replacement policy, atomic state management

## WHERE TO LOOK
| Task | File | Notes |
|------|------|-------|
| Buffer allocation | mod.rs | `get_page()`, `allocate_buffer()` |
| LRU integration | lru.rs | Hot/Cold/Free list management |
| Dirty tracking | mod.rs | Atomic state bit 0 = dirty flag |
| Pin counting | mod.rs | Atomic state bits 8-63 = pin count |

## MODULE STRUCTURE
```
buffer/
├── mod.rs           # BufferMgr, BufferDesc, BufferTag
├── lru.rs          # LruManager<Node<T>>, 3-tier LRU |
├── DESIGN.md       # Architecture docs
└── tests.rs        # Integration tests
```

## CONVENTIONS (deviations from root)

### Atomic State Pattern
```rust
// State layout (64-bit AtomicU64)
const DIRTY_BIT: u64 = 1 << 0;
const PIN_COUNT_SHIFT: u8 = 8;
buffer.state.fetch_or(DIRTY_BIT, Ordering::Relaxed);
```

### Hash Chain Lookup
```rust
// Chained hash table for buffer lookup
buf_hash_table: *mut *mut HashEntry  // Raw pointers for FFI-style
```

### LRU Integration
```rust
// 3-tier LRU (hot/cold/free)
lru.add(buffer_idx);   // New buffer access
lru.access(&buffer_idx); // Existing buffer access
```

## ANTI-PATTERNS
- NEVER modify buffer state without atomic operations
- NEVER skip dirty bit sync on flush
- NEVER bypass LRU for hot paths

## COMMANDS
```bash
cargo test --lib buffer    # BufferPool tests
cargo test --lib lru.rs  # LRU patterns
cargo fmt                # Required before commit
```
