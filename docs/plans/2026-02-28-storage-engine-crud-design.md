# Storage Engine CRUD Design

**Date:** 2026-02-28
**Status:** Approved
**Target:** Minimal viable storage engine with SQL interface

---

## Overview

Implement CRUD operations (Create, Read, Update, Delete) for the Aistore storage engine with SQL interface and simplified heap storage.

### Design Goals

1. **SQL Interface**: SQLite-like embedded SQL (INSERT, SELECT, UPDATE, DELETE, CREATE TABLE)
2. **Heap Storage**: Simplified heap table with full table scan
3. **Minimal Viable**: YAGNI - defer JOIN, subqueries, indexes to future phases

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                   SQL Interface                      │
│  Parser → Binder → Optimizer → Executor             │
└─────────────────┬───────────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────────┐
│              Storage Layer                           │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
│  │ Catalog  │  │  Heap    │  │  Buffer  │          │
│  └──────────┘  └──────────┘  └──────────┘          │
└─────────────────┬───────────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────────┐
│              VFS / Segment / Page                   │
└─────────────────────────────────────────────────────┘
```

---

## Data Layout

### Heap Page (8KB)

```
┌────────────────────────────────────────┐
│           Page Header (64B)            │
│  - page_id: u64                        │
│  - page_type: u8                       │
│  - checksum: u32                        │
│  - free_space: u32                      │
│  - slot_count: u32                     │
│  - reserved                            │
├────────────────────────────────────────┤
│           Slot Array                   │
│  [slot 0] offset: i32, length: u32    │
│  [slot 1] offset: i32, length: u32    │
│  ...                                   │
├────────────────────────────────────────┤
│           Free Space                   │
│                                        │
├────────────────────────────────────────┤
│           Tuple Data                   │
│  ┌────────────────────────────────┐   │
│  │ Tuple: [null bitmap][col1]...  │   │
│  └────────────────────────────────┘   │
└────────────────────────────────────────┘
```

### Tuple Format

```
┌────────────────────────────────────────┐
│  Null Bitmap (variable)                 │
│  - 1 bit per column                    │
│  - padded to byte boundary             │
├────────────────────────────────────────┤
│  Column 1 Data                         │
├────────────────────────────────────────┤
│  Column 2 Data                         │
├────────────────────────────────────────┤
│  ...                                   │
└────────────────────────────────────────┘
```

---

## Module Design

### 1. Heap Module (`src/heap/`)

| Component | Responsibility |
|-----------|----------------|
| `HeapTable` | Manages heap pages for a table |
| `HeapPage` | Page-level tuple operations |
| `Tuple` | In-memory tuple representation |
| `RowSerializer` | Serialize/deserialize tuples |

**Key APIs:**
```rust
impl HeapTable {
    pub fn insert(&mut self, values: &[Value]) -> Result<RowId>;
    pub fn scan(&self, filter: Option<&Predicate>) -> Result<Vec<Tuple>>;
    pub fn update(&mut self, row_id: RowId, values: &[Value]) -> Result<()>;
    pub fn delete(&mut self, row_id: RowId) -> Result<()>;
}
```

### 2. SQL Parser (`src/sql/`)

| Component | Responsibility |
|-----------|----------------|
| `Parser` | Tokenize and parse SQL |
| `AST` | Abstract syntax tree nodes |
| `Binder` | Resolve table/column names |

**Supported Syntax:**
```sql
CREATE TABLE name (col1 TYPE, col2 TYPE, ...)

INSERT INTO name VALUES (val1, val2, ...)

SELECT col1, col2 FROM name [WHERE condition]

UPDATE name SET col1=val1 [WHERE condition]

DELETE FROM name [WHERE condition]
```

### 3. Query Executor (`src/executor/`)

| Component | Responsibility |
|-----------|----------------|
| `Executor` | Execute query plan |
| `ProjectExecutor` | SELECT projection |
| `FilterExecutor` | WHERE clause evaluation |
| `InsertExecutor` | INSERT execution |
| `UpdateExecutor` | UPDATE execution |
| `DeleteExecutor` | DELETE execution |

---

## Implementation Phases

### Phase 1: Row Storage (Heap)
- [ ] Implement `Tuple` struct with serialization
- [ ] Implement `HeapPage` with slot array
- [ ] Implement `HeapTable` for page management
- [ ] Connect to BufferPool

### Phase 2: SQL Parser
- [ ] Implement tokenizer (lexer)
- [ ] Implement AST nodes
- [ ] Implement CREATE TABLE parser
- [ ] Implement INSERT/SELECT/UPDATE/DELETE parser

### Phase 3: Query Executor
- [ ] Implement Binder (resolve names)
- [ ] Implement simple executor
- [ ] Implement WHERE evaluation
- [ ] Integrate with Catalog

### Phase 4: Integration
- [ ] Add SQL entry point to `lib.rs`
- [ ] Add CLI for testing
- [ ] End-to-end tests

---

## Error Handling

- Use module-specific Result types (`HeapResult`, `SqlResult`, `ExecutorResult`)
- Convert errors up the stack with `?` operator
- Return user-friendly error messages for SQL parse errors

---

## Testing Strategy

1. **Unit Tests**: Each module has corresponding tests
2. **Integration Tests**: End-to-end SQL tests
3. **Benchmark Tests**: Basic performance smoke tests

---

## Future Phases (Out of Scope)

- B-tree indexes
- JOIN operations
- Transaction isolation (MVCC)
- SQL aggregates (GROUP BY, ORDER BY, LIMIT)
- Prepared statements

---

## References

- [SQLite Architecture](https://www.sqlite.org/arch.html)
- [InnoDB Row Format](https://dev.mysql.com/doc/refman/8.0/en/innodb-row-format.html)
- Existing modules: `buffer/`, `page/`, `segment/`, `tablespace/`, `catalog/`
