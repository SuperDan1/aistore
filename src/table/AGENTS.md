# table/ AGENTS.md

**Generated:** Table metadata, column definitions, system cache

## OVERVIEW
Table data model: Table struct, Column definitions, TableBuilder pattern, SysCache for quick lookups

## WHERE TO LOOK
| Task | File | Notes |
|------|------|-------|
| Table struct | table.rs | `Table`, `TableType` enums |
| Column def | column.rs | Column metadata, types |
| Builder | builder.rs | `TableBuilder` for construction |
| SysCache | syscache.rs | In-memory table lookup cache |

## MODULE STRUCTURE
```
table/
├── mod.rs           # Exports, re-exports
├── table.rs         # Table, TableType structs
├── builder.rs       # TableBuilder pattern
├── column.rs        # Column definition
├── syscache.rs      # SysCache for fast lookup
└── tests.rs         # Integration tests
```

## CONVENTIONS

### Builder Pattern
```rust
let table = TableBuilder::new(table_id, name)
    .segment_id(seg_id)
    .table_type(TableType::User)
    .columns(columns)
    .try_build()?;
```

### Column Definition
```rust
Column::new(name, column_type, nullable, ordinal)
```

## ANTI-PATTERNS
- NEVER use `TableType::Invalid` for valid tables
- NEVER skip column validation in builder

## COMMANDS
```bash
cargo test --lib table     # Table tests
cargo fmt                 # Required
```
