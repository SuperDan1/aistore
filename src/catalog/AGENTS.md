# catalog/ AGENTS.md

**Generated:** System catalog for table metadata persistence

## OVERVIEW
System catalog: table registry, metadata persistence, name/ID caching. Manages table lifecycle (create, load, drop).

## WHERE TO LOOK
| Task | File | Notes |
|------|------|-------|
| Catalog struct | mod.rs | `Catalog`, `TableEntry` |
| Error types | error.rs | `CatalogError`, `CatalogResult` |
| Table ops | mod.rs | `create_table()`, `get_table()`, `drop_table()` |

## MODULE STRUCTURE
```
catalog/
├── mod.rs           # Catalog, TableEntry, persistence
├── error.rs         # CatalogError, CatalogResult
└── tests.rs         # Integration tests
```

## KEY PATTERNS

### Catalog Creation/Load
```rust
// New catalog
let catalog = Catalog::new(data_dir)?;

// Load existing
let catalog = Catalog::load(data_dir)?;
```

### Table Operations
```rust
catalog.create_table(name, segment_id, columns)?;
catalog.get_table(name)?;
catalog.drop_table(name)?;
```

### Persistence
- Tables stored in `system/` dir as `.tbl` files
- Columns stored in `system/columns.dat`

## ERROR HANDLING
- Use `CatalogResult<T>` not standard `Result`
- Convert errors: `CatalogError::TableAlreadyExists`, `CatalogError::TableNotFound`

## ANTI-PATTERNS
- NEVER mix `CatalogResult` with standard `Result` in same function
- NEVER skip table validation on load

## COMMANDS
```bash
cargo test --lib catalog   # Catalog tests
cargo fmt                  # Required
```
