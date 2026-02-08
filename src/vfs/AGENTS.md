# vfs/ AGENTS.md

**Generated:** Virtual filesystem abstraction for cross-platform I/O

## OVERVIEW
Trait-based VFS interface, LocalFs implementation, posix-style operations

## WHERE TO LOOK
| Task | File | Notes |
|------|------|-------|
| Interface def | interface.rs | `VfsInterface` trait |
| Local impl | local_fs.rs | posix calls via libc |
| Error handling | error.rs | `VfsResult<T>` pattern |
| Tests | tests.rs | File operations, pread/pwrite |

## MODULE STRUCTURE
```
vfs/
├── interface.rs      # VfsInterface trait, trait impl guidelines
├── local_fs.rs       # LocalFs posix impl
├── error.rs         # Error types
├── tests.rs         # I/O patterns |
└── mod.rs           # Exports, re-exports
```

## CONVENTIONS (deviations from root)

### Trait Pattern
```rust
pub trait VfsInterface {
    fn pread(&self, path: &str, buf: &mut [u8], offset: u64) -> VfsResult<usize>;
    fn pwrite(&self, path: &self.str, buf: &[u8], offset: u64) -> VfsResult<usize>;
}
```

### Error Conversion
```rust
// libc errors → VfsError conversion required
```

## ANTI-PATTERNS
- NEVER skip error conversion
- NEVER bypass trait impl patterns
- NEVER mix posix with std::fs in same impl

## COMMANDS
```bash
cargo test vfs         # I/O patterns
cargo fmt             # Required
```
