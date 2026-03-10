# Aistore 测试计划

**版本:** 1.3  
**日期:** 2026-03-10  
**目标:** 80%+ 测试覆盖率, MVCC + ACID 验证

---

## 1. 当前测试状态

| 指标 | 当前值 | 目标值 |
|------|--------|--------|
| 总测试数 | **204** | 300+ |
| 模块测试文件 | 13 | 20+ |
| 并发测试 | 10 | 30+ |
| 集成测试 | 25 | 50+ |

### 已完成测试分布

| 模块 | 测试文件 | 测试数 | 状态 |
|------|----------|--------|------|
| buffer | buffer/mod.rs (inline) | 12 | ✅ |
| catalog | catalog/tests.rs | 12 | ✅ |
| vfs | vfs/tests.rs | 5 | ✅ |
| table | table/tests.rs | 3 | ✅ |
| page | page/tests.rs | 2 | ✅ |
| segment | segment/tests.rs | 4 | ✅ |
| index | index/tests.rs | **14** | ✅ 新增 |
| infrastructure | hash/tests.rs, hash_table/tests.rs | 15 | ✅ |
| wal | wal/tests.rs, lsn/tests.rs | 22 | ✅ |
| lock | lock/tests.rs | **43** | ✅ |
| heap | heap/tests.rs | 8 | ✅ |
| storage (MVCC) | storage.rs | 10 | ✅ |
| storage (ACID) | storage.rs | 8 | ✅ |
| storage (集成) | storage.rs | 8 | ✅ |

---

## 2. 测试计划

### 2.1 单元测试 (Unit Tests)

#### 模块: buffer (目标: +15 tests)

```rust
// src/buffer/tests.rs

// LRU 测试
#[test]
fn test_lru_eviction_order() { }
#[test]
fn test_lru_pin_prevents_eviction() { }
#[test]
fn test_lru_multiple_access() { }
#[test]
fn test_lru_resize() { }

// BufferDesc 状态测试
#[test]
fn test_buffer_desc_concurrent_pin() { }
#[test]
fn test_buffer_desc_dirty_pin_clear() { }
#[test]
fn test_buffer_desc_state_transitions() { }

// BufferMgr 功能测试
#[test]
fn test_buffer_mgr_get_nonexistent() { }
#[test]
fn test_buffer_mgr_pin_after_evict() { }
#[test]
fn test_buffer_mgr_stats() { }

// Flush 相关
#[test]
fn test_flush_single_page() { }
#[test]
fn test_flush_multiple_pages() { }
#[test]
fn test_double_write_basic() { }
#[test]
fn test_double_write_recovery() { }
#[test]
fn test_flusher_background() { }
```

#### 模块: wal (目标: +20 tests)

```rust
// src/wal/tests.rs (新建)

// LSN 测试
#[test]
fn test_lsn_max_value() { }
#[test]
fn test_lsn_wrap_around() { }

// LogRecord 测试
#[test]
fn test_log_record_serialization() { }
#[test]
fn test_log_record_deserialization() { }
#[test]
fn test_log_record_checksum() { }

// WAL Manager 测试
#[test]
fn test_wal_append_basic() { }
#[test]
fn test_wal_append_multiple() { }
#[test]
fn test_wal_commit_order() { }
#[test]
fn test_wal_group_commit_timeout() { }
#[test]
fn test_wal_group_commit_batch_size() { }

// Checkpoint 测试
#[test]
fn test_checkpoint_create() { }
#[test]
fn test_checkpoint_load() { }
#[test]
fn test_checkpoint_incremental() { }

// Recovery 测试
#[test]
fn test_recovery_basic() { }
#[test]
fn test_recovery_redo() { }
#[test]
fn test_recovery_undo() { }
#[test]
fn test_recovery_partial_commit() { }
#[test]
fn test_recovery_multiple_txs() { }
```

#### 模块: lock (目标: +25 tests)

```rust
// src/lock/tests.rs (新建)

// Transaction 测试
#[test]
fn test_transaction_begin() { }
#[test]
fn test_transaction_commit() { }
#[test]
fn test_transaction_abort() { }
#[test]
fn test_transaction_id_increment() { }

// RowLock 测试
#[test]
fn test_row_lock_exclusive() { }
#[test]
fn test_row_lock_shared() { }
#[test]
fn test_row_lock_upgrade() { }
#[test]
fn test_row_lock_reentrant() { }
#[test]
fn test_row_lock_release() { }

// TableLock 测试
#[test]
fn test_table_lock_exclusive() { }
#[test]
fn test_table_lock_shared() { }
#[test]
fn test_table_lock_intention() { }

// 2PL 协议测试
#[test]
fn test_2pl_growing_phase() { }
#[test]
fn test_2pl_shrinking_phase() { }
#[test]
fn test_2pl_violation_detection() { }

// 死锁测试
#[test]
fn test_deadlock_detection_cycle() { }
#[test]
fn test_deadlock_detection_timeout() { }
#[test]
fn test_deadlock_resolution() { }
#[test]
fn test_no_deadlock_wait_graph() { }
```

#### 模块: mvcc (目标: +15 tests)

```rust
// src/mvcc/tests.rs (新建)

// 可见性测试 (RC)
#[test]
fn test_mvcc_rc_insert_visible() { }
#[test]
fn test_mvcc_rc_update_uncommitted_visible() { }
#[test]
fn test_mvrc_rc_update_committed_visible() { }
#[test]
fn test_mvcc_rc_delete_uncommitted() { }
#[test]
fn test_mvcc_rc_delete_committed() { }
#[test]
fn test_mvcc_rc_rollback_visible() { }

// Undo 链测试
#[test]
fn test_undo_chain_single() { }
#[test]
fn test_undo_chain_multiple() { }
#[test]
fn test_undo_chain_update() { }
#[test]
fn test_undo_chain_delete() { }

// Undo Manager 测试
#[test]
fn test_undo_manager_append() { }
#[test]
fn test_undo_manager_get() { }
#[test]
fn test_undo_manager_persistence() { }
```

#### 模块: index (目标: +10 tests)

```rust
// src/index/tests.rs (新建)

// B+Tree 测试
#[test]
fn test_btree_insert_basic() { }
#[test]
fn test_btree_insert_many() { }
#[test]
fn test_btree_split() { }
#[test]
fn test_btree_search() { }
#[test]
fn test_btree_range_search() { }
#[test]
fn test_btree_delete() { }
#[test]
fn test_btree_delete_merge() { }
#[test]
fn test_btree_persistence() { }
#[test]
fn test_index_lookup() { }
#[test]
fn test_index_range_scan() { }
```

#### 模块: heap (目标: +10 tests)

```rust
// src/heap/tests.rs (新建)

// HeapTable 测试
#[test]
fn test_heap_insert_basic() { }
#[test]
fn test_heap_insert_many() { }
#[test]
fn test_heap_scan() { }
#[test]
fn test_heap_get() { }
#[test]
fn test_heap_update() { }
#[test]
fn test_heap_delete() { }
#[test]
fn test_heap_persistence() { }
#[test]
fn test_heap_mvcc_header() { }
#[test]
fn test_heap_undo_ptr() { }
#[test]
fn test_heap_iter_visible() { }
```

#### 模块: storage (目标: +20 tests)

```rust
// src/storage/tests.rs (新建)

// CRUD 测试
#[test]
fn test_storage_create_table() { }
#[test]
fn test_storage_drop_table() { }
#[test]
fn test_storage_insert() { }
#[test]
fn test_storage_scan() { }
#[test]
fn test_storage_update() { }
#[test]
fn test_storage_delete() { }
#[test]
fn test_storage_filter() { }

// 事务测试
#[test]
fn test_storage_transaction_basic() { }
#[test]
fn test_storage_transaction_commit() { }
#[test]
fn test_storage_transaction_abort() { }
#[test]
fn test_storage_transaction_isolation() { }
```

---

### 2.2 并发测试 (Concurrency Tests)

```rust
// src/storage/concurrency_tests.rs (新建)

// Buffer Pool 并发
#[test]
fn test_concurrent_buffer_pin() { }
#[test]
fn test_concurrent_buffer_evict() { }
#[test]
fn test_concurrent_buffer_dirty() { }
#[test]
fn test_concurrent_flusher() { }

// Lock 并发
#[test]
fn test_concurrent_row_lock_contention() { }
#[test]
fn test_concurrent_table_lock_contention() { }
#[test]
fn test_concurrent_mixed_lock() { }
#[test]
fn test_concurrent_deadlock_prevention() { }

// WAL 并发
#[test]
fn test_concurrent_wal_append() { }
#[test]
fn test_concurrent_wal_commit() { }
#[test]
fn test_concurrent_group_commit() { }

// Transaction 并发
#[test]
fn test_concurrent_transactions_read() { }
#[test]
fn test_concurrent_transactions_write() { }
#[test]
fn test_concurrent_read_write_conflict() { }
#[test]
fn test_concurrent_write_write_conflict() { }
#[test]
fn test_concurrent_long_short_tx() { }
```

---

### 2.3 集成测试 (Integration Tests)

```rust
// src/storage/integration_tests.rs

// StorageApi 完整流程
#[test]
fn test_integration_full_workflow() { }
#[test]
fn test_integration_multiple_tables() { }
#[test]
fn test_integration_recovery() { }

// CRUD 集成
#[test]
fn test_integration_crud_all_types() { }
#[test]
fn test_integration_null_values() { }
#[test]
fn test_integration_large_values() { }

// 索引集成
#[test]
fn test_integration_with_index() { }
#[test]
fn test_integration_index_lookup() { }
#[test]
fn test_integration_index_update() { }

// 扫描集成
#[test]
fn test_integration_scan_empty() { }
#[test]
fn test_integration_scan_with_filter() { }
#[test]
fn test_integration_scan_range() { }
#[test]
fn test_integration_scan_pagination() { }
```

---

### 2.4 MVCC 验证测试

```rust
// src/storage/mvcc_tests.rs (扩展)

// RC 隔离级别测试
#[test]
fn test_mvcc_rc_insert_visible_self() { }
#[test]
fn test_mvcc_rc_insert_invisible_other_uncommitted() { }
#[test]
fn test_mvcc_rc_insert_visible_other_committed() { }
#[test]
fn test_mvcc_rc_update_own_uncommitted() { }
#[test]
fn test_mvcc_rc_update_own_committed() { }
#[test]
fn test_mvcc_rc_update_other_uncommitted_see_old() { }
#[test]
fn test_mvcc_rc_update_other_committed_see_new() { }
#[test]
fn test_mvcc_rc_delete_uncommitted() { }
#[test]
fn test_mvcc_rc_delete_committed() { }
#[test]
fn test_mvcc_rc_phantom_read() { }

// 快照测试
#[test]
fn test_mvcc_snapshot_isolation() { }
#[test]
fn test_mvcc_snapshot_consistency() { }
#[test]
fn test_mvcc_snapshot_multiple_readers() { }

// Undo 链验证
#[test]
fn test_mvcc_undo_chain_integrity() { }
#[test]
fn test_mvcc_undo_chain_recovery() { }
```

---

### 2.5 ACID 验证测试

```rust
// src/storage/acid_tests.rs

// Atomicity (原子性)
#[test]
fn test_acid_atomic_insert() { }
#[test]
fn test_acid_atomic_update() { }
#[test]
fn test_acid_atomic_delete() { }
#[test]
fn test_acid_atomic_multi_operation() { }
#[test]
fn test_acid_atomic_partial_failure() { }

// Consistency (一致性)
#[test]
fn test_acid_consistency_constraints() { }
#[test]
fn test_acid_consistency_index() { }
#[test]
fn test_acid_consistency_mvcc() { }
#[test]
fn test_acid_consistency_after_recovery() { }

// Isolation (隔离性) - 已覆盖在 MVCC 测试中
#[test]
fn test_acid_isolation_dirty_read() { }
#[test]
fn test_acid_isolation_non_repeatable_read() { }
#[test]
fn test_acid_isolation_phantom_read() { }
#[test]
fn test_acid_isolation_serializable() { }

// Durability (持久性)
#[test]
fn test_acid_durability_commit_persisted() { }
#[test]
fn test_acid_durability_after_crash() { }
#[test]
fn test_acid_durability_wal_integrity() { }
#[test]
fn test_acid_durability_checkpoint_recovery() { }
```

---

## 3. 测试覆盖率目标

### 按模块覆盖率目标

| 模块 | 当前 | 目标 | 新增测试 |
|------|------|------|----------|
| buffer | 60% | 85% | +25 |
| wal | 40% | 85% | +35 |
| lock | 0% | 85% | +30 |
| mvcc | 30% | 90% | +25 |
| index | 30% | 85% | +20 |
| heap | 20% | 85% | +20 |
| storage | 20% | 80% | +60 |
| vfs | 80% | 90% | +5 |
| catalog | 70% | 85% | +10 |
| page | 40% | 70% | +10 |
| segment | 50% | 70% | +10 |
| **总计** | ~35% | **85%** | **~250** |

---

## 4. 执行计划

### Phase 1: 核心模块单元测试 (Week 1)
- [ ] buffer 模块 +15 tests
- [ ] wal 模块 +20 tests
- [ ] lock 模块 +25 tests

### Phase 2: MVCC 和存储引擎 (Week 2)
- [ ] mvcc 模块 +15 tests
- [ ] index 模块 +10 tests
- [ ] heap 模块 +10 tests

### Phase 3: 集成和 ACID (Week 3)
- [ ] storage 模块 +20 tests
- [ ] 并发测试 +15 tests
- [ ] MVCC 验证测试 +15 tests
- [ ] ACID 测试 +15 tests

### Phase 4: 覆盖率验证 (Week 4)
- [ ] 安装 tarpaulin
- [ ] 运行覆盖率分析
- [ ] 补充缺失测试
- [ ] 达到 80%+ 目标

---

## 5. 测试运行命令

```bash
# 运行所有测试
cargo test --lib

# 运行特定模块测试
cargo test --lib buffer
cargo test --lib wal
cargo test --lib lock

# 运行并发测试
cargo test --lib concurrency

# 运行集成测试
cargo test --lib integration

# 运行 MVCC 测试
cargo test --lib mvcc

# 运行 ACID 测试
cargo test --lib acid

# 运行覆盖率
cargo install cargo-tarpaulin
cargo tarpaulin --out Html --output-dir coverage

# 运行特定测试文件
cargo test --lib --test '*'
```

---

## 6. 验收标准

- [ ] 总测试数 >= 300
- [ ] 覆盖率 >= 80%
- [ ] 所有模块单元测试 >= 80%
- [ ] MVCC RC 隔离级别测试完整
- [ ] ACID 四个属性测试完整
- [ ] 并发测试覆盖主要场景
- [ ] cargo test --lib 全部通过

---

## 7. 风险与缓解

| 风险 | 缓解措施 |
|------|----------|
| 并发测试不稳定 | 使用适当的 sleep/mutex |
| 覆盖率工具缺失 | 手动统计 + tarpaulin |
| 测试执行时间过长 | 分离快速/慢速测试 |
| 内存占用过高 | 使用 tempfile + 自动清理 |
