# page/ AGENTS.md

**Generated:** 8KB page structure and page type definitions

## OVERVIEW
Storage unit (8KB pages): PageHeader, PageType enum, special page types for metadata. Shared between BufferPool and segment storage.

## WHERE TO LOOK
| Task | File | Notes |
|------|------|-------|
| Page types | mod.rs | `Page`, `PageType`, `Special` enums |
| Page ops | mod.rs | `PageHeader` manipulation |

## MODULE STRUCTURE
```
page/
├── mod.rs           # Page, PageType, PageHeader, Special
└── tests.rs        # Page operations
```

## PAGE LAYOUT (8KB)
```
Page (8192 bytes)
├── PageHeader (64 bytes)
│   ├── page_id: u64
│   ├── page_type: u8
│   ├── checksum: u32
│   └── ...
└── PageBody (8128 bytes)
    └── Type-specific data
```

## PAGE TYPES
```rust
pub enum PageType {
    Invalid,      // Uninitialized
    Data,         // Regular B-tree node or heap row
    Internal,     // B-tree internal node
    Leaf,         // B-tree leaf node
    Special,      // Special purpose (see Special enum)
}
```

## ANTI-PATTERNS
- NEVER modify page without acquiring buffer lock
- NEVER skip checksum verification on disk read
- NEVER use `PageType::Invalid` for valid pages

## COMMANDS
```bash
cargo test --lib page     # Page tests
cargo fmt               # Required
```
