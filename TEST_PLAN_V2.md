# Aistore 测试计划 v2.0

**版本:** 2.0  
**日期:** 2026-03-11  
**目标:** 发现并发、内存、性能问题，80%+ 测试覆盖率

---

## 1. 现有测试状态分析

### 1.1 已识别测试文件

| 模块 | 测试文件 | 测试数 | 状态 |
|------|----------|--------|------|
| buffer | `src/buffer/tests.rs` | ~12 | ✅ |
| catalog | `src/catalog/tests.rs` | ~12 | ✅ |
| vfs | `src/vfs/tests.rs` | ~5 | ✅ |
| table | `src/table/tests.rs` | ~3 | ✅ |
| page | `src/page/tests.rs` | ~2 | ✅ |
| segment | `src/segment/tests.rs` | ~4 | ✅ |
| index | `src/index/tests.rs` | ~14 | ✅ |
| infrastructure | `hash/tests.rs`, `hash_table/tests.rs` | ~15 | ✅ |
| wal | `wal/tests.rs`, `lsn/tests.rs` | ~22 | ✅ |
| lock | `src/lock/tests.rs` | ~43 | ✅ |
| heap | `src/heap/tests.rs` | ~8 | ✅ |
| storage | MVCC + ACID + 集成 | ~26 | ✅ |

### 1.2 架构要点

```
StorageEngine
├── BufferMgr (RwLock<BufferMgr>)
│   ├── HashTable (buffer lookup)
│   ├── LRU Manager (hot/cold/free)
│   └── PageFlusher (background thread)
├── LockManager
│   ├── TransactionManager (TxId counter)
│   ├── RowLockManager
│   ├── TableLockManager
│   └── PageLockManager
├── HeapTable (per table)
├── IndexManager
├── UndoManager
├── Catalog
└── WalManager (background flush thread)
```

---

## 2. 潜在问题识别

### 2.1 并发问题 (Race Conditions)

| 位置 | 问题 | 风险 |
|------|------|------|
| `buffer/mod.rs` | 64-bit atomic state: DIRTY_BIT + PIN_COUNT 并发更新 | 高 |
| `buffer/mod.rs` | Hash table: buffer lookup/insert 无锁 | 高 |
| `lock/row_lock.rs` | Waiter queue timeout 处理 | 中 |
| `lock/page_lock.rs` | Waiter promotion 并发 | 中 |
| `wal/log_buffer.rs` | Append/flush 协调 | 中 |
| `storage.rs` | tables HashMap 非线程安全 | **极高** |

### 2.2 内存问题

| 位置 | 问题 | 风险 |
|------|------|------|
| `buffer/mod.rs` | Buffer eviction 时未同步 undo log | 高 |
| `heap/mod.rs` | MVCC header 泄漏 | 中 |
| `wal/` | Checkpoint 保留过多日志 | 中 |

### 2.3 性能问题

| 位置 | 问题 | 影响 |
|------|------|------|
| `buffer/mod.rs` | 每次 get_page 都 hash lookup | 中 |
| `lock/` | 死锁检测超时配置 | 中 |
| `wal/` | Group commit batch size | 低 |

---

## 3. 模块级单元测试

### 3.1 Buffer 模块 (buffer/)

```rust
// src/buffer/tests.rs - 目标: 发现并发/内存问题

// === 并发问题测试 ===

// 问题: test_buffer_state_race 检测 atomic state race
// 64-bit 布局: DIRTY_BIT(bit 0) + PIN_COUNT(bits 8-63)
#[test]
fn test_buffer_state_race() {
    // 启动多个线程同时 pin/unpin 同一 buffer
    // 检测: dirty bit 和 pin count 是否一致
    let buffer = Arc::new(BufferDesc::new(tag));
    let barrier = Arc::new(Barrier::new(10));
    
    let handles: Vec<_> = (0..10).map(|_| {
        let buf = Arc::clone(&buffer);
        let b = Arc::clone(&barrier);
        thread::spawn(move || {
            b.wait();
            for _ in 0..1000 {
                buf.acquire_pin();
                buf.release_pin();
            }
        })
    }).collect();
    
    // 检测 pin count 是否为 0 (可能有泄漏)
    assert_eq!(buffer.get_pin_count(), 0, "PIN count leak detected!");
}

// 问题: test_buffer_dirty_pin_concurrent 检测 dirty + pin 竞态
#[test]
fn test_buffer_dirty_pin_concurrent() {
    // 并发: 线程A pin + dirty, 线程B release_pin
    // 期望: dirty bit 在 pin 释放后正确反映
}

// 问题: test_hash_table_concurrent_lookup_insert 检测 hash 竞态
// 注意: 代码中使用 raw pointer: buf_hash_table: *mut *mut HashEntry
#[test]
fn test_hash_table_concurrent_lookup_insert() {
    // 并发 lookup + insert 到同一 bucket
    // 检测: 是否有 use-after-free
}

// === 内存泄漏测试 ===

// 问题: test_buffer_evict_undo_association 检测 eviction 后 undo 指针有效性
#[test]
fn test_buffer_evict_undo_association() {
    // 插入数据 → 修改 → evict → crash recovery
    // 检测: undo ptr 是否指向有效版本
}

// 问题: test_lru_memory_leak 检测 LRU 内存泄漏
#[test]
fn test_lru_memory_leak() {
    // 大量随机访问模式
    // 检测: RSS 增长是否正常
}

// === 边界条件测试 ===

// 问题: test_buffer_max_pin_overflow 检测 pin count 溢出
#[test]
fn test_buffer_max_pin_overflow() {
    // PIN_COUNT_SHIFT = 8, 最大 pin 数 = 2^56 - 1
    // 溢出行为测试
}

// 问题: test_dirty_bit_concurrent_set_clear 检测 dirty 标志竞态
#[test]
fn test_dirty_bit_concurrent_set_clear() {
    // 并发 set_dirty + clear_dirty
    // 检测: dirty 状态最终一致性
}
```

### 3.2 Lock 模块 (lock/)

```rust
// src/lock/tests.rs - 目标: 死锁/活锁检测

// === 死锁测试 ===

// 问题: test_deadlock_cycle_detection 检测死锁检测器
#[test]
fn test_deadlock_cycle_detection() {
    // T1: lock(A) → lock(B)
    // T2: lock(B) → lock(A)
    // 期望: 其中一个超时/死锁检测
}

// 问题: test_deadlock_timeout 配置验证
#[test]
fn test_deadlock_timeout() {
    // 设置超时时间
    // 验证: 超时后返回 LockError::Timeout
}

// === 活锁测试 ===

// 问题: test_livelock_starvation 检测活锁
#[test]
fn test_livelock_starvation() {
    // 多个 reader 持有 shared lock
    // writer 一直等待
    // 检测: writer 是否最终获得锁
}

// === 锁升级测试 ===

// 问题: test_row_lock_upgrade 检测 S → X 升级
#[test]
fn test_row_lock_upgrade() {
    // Shared lock → 尝试升级为 Exclusive
    // 检测: 升级是否成功/阻塞
}

// === 超时边界测试 ===

// 问题: test_lock_waiter_timeout_precision 检测超时精度
#[test]
fn test_lock_waiter_timeout_precision() {
    // 设置 100ms 超时
    // 检测: 实际等待时间 ±10ms
}
```

### 3.3 WAL 模块 (wal/)

```rust
// src/wal/tests.rs - 目标: 持久性/恢复问题

// === 并发 Append 测试 ===

// 问题: test_concurrent_wal_append 检测并发写入
#[test]
fn test_concurrent_wal_append() {
    // 多线程并发 append
    // 检测: LSN 顺序、记录完整性
}

// 问题: test_wal_group_commit 检测 group commit
#[test]
fn test_wal_group_commit() {
    // 配置 batch_size=100, timeout=10ms
    // 快速提交多笔小事务
    // 检测: 是否正确 batch
}

// === 恢复测试 ===

// 问题: test_wal_recovery_partial_write 检测 partial write 恢复
#[test]
fn test_wal_recovery_partial_write() {
    // 写入过程中 crash
    // 恢复后检测: 无脏数据
}

// 问题: test_wal_recovery_log_integrity 检测日志完整性
#[test]
fn test_wal_recovery_log_integrity() {
    // 模拟磁盘损坏 (checksum 错误)
    // 检测: 跳过损坏记录继续恢复
}

// === Checkpoint 测试 ===

// 问题: test_checkpoint_concurrent_with_append 检测 checkpoint 并发
#[test]
fn test_checkpoint_concurrent_with_append() {
    // Checkpoint 过程中并发 append
    // 检测: checkpoint 完整性
}

// 问题: test_checkpoint_space_reclaim 检测空间回收
#[test]
fn test_checkpoint_space_reclaim() {
    // 多次 checkpoint
    // 检测: WAL 文件大小是否合理
}
```

### 3.4 Heap 模块 (heap/)

```rust
// src/heap/tests.rs - 目标: MVCC/undo 问题

// === MVCC 可见性测试 ===

// 问题: test_mvcc_version_chain 检测版本链完整性
#[test]
fn test_mvcc_version_chain() {
    // Insert → Update → Update → Delete
    // 检测: 版本链是否正确
}

// 问题: test_mvcc_undo_ptr_validity 检测 undo 指针有效性
#[test]
fn test_mvcc_undo_ptr_validity() {
    // 大量更新后检查 undo ptr
    // 检测: 指针是否指向有效 page/slot
}

// === 内存问题测试 ===

// 问题: test_heap_tuple_overwrite_memory 检测内存覆盖
#[test]
fn test_heap_tuple_overwrite_memory() {
    // Update 覆盖大值 → 再次覆盖小值
    // 检测: 旧数据是否被正确覆盖
}

// 问题: test_heap_null_padding_memory 检测 null 填充
#[test]
fn test_heap_null_padding_memory() {
    // 插入大量含 null 的行
    // 检测: 存储空间使用
}

// === 并发测试 ===

// 问题: test_heap_concurrent_insert_delete 检测并发 DDL
#[test]
fn test_heap_concurrent_insert_delete() {
    // 并发 insert + delete 同一 table
    // 检测: row_id 分配是否正确
}
```

---

## 4. StorageApi 集成测试

### 4.1 CRUD 完整性测试

```rust
// src/storage/integration_tests.rs

// === 基础 CRUD 测试 ===

#[test]
fn test_storage_api_create_drop_table() {
    // Create table → exists → drop → exists (false)
    let mut storage = StorageEngine::new(temp_dir()).unwrap();
    
    storage.create_table("users", vec![
        Column::new("id".into(), ColumnType::Int64, true, 0),
    ]).unwrap();
    
    assert!(storage.table_exists("users"));
    
    storage.drop_table("users").unwrap();
    
    assert!(!storage.table_exists("users"));
}

#[test]
fn test_storage_api_insert_scan() {
    let mut storage = StorageEngine::new(temp_dir()).unwrap();
    storage.create_table("test", columns()).unwrap();
    
    // Insert 1000 rows
    for i in 0..1000 {
        storage.insert("test", values(i)).unwrap();
    }
    
    // Scan all
    let results = storage.scan("test", None).unwrap();
    assert_eq!(results.len(), 1000);
}

#[test]
fn test_storage_api_update_delete() {
    let mut storage = StorageEngine::new(temp_dir()).unwrap();
    storage.create_table("test", columns()).unwrap();
    
    let id = storage.insert("test", values(1)).unwrap();
    
    // Update
    storage.update("test", id, new_values()).unwrap();
    
    // Delete
    storage.delete("test", id).unwrap();
    
    let results = storage.scan("test", Some(filter(id))).unwrap();
    assert!(results.is_empty());
}

// === Filter 边界测试 ===

#[test]
fn test_storage_api_filter_all_types() {
    // 测试所有 ColumnType 的 filter
    let types = vec![
        ColumnType::Int64,
        ColumnType::Varchar(100),
        ColumnType::Boolean,
        ColumnType::Float64,
    ];
    
    for ct in types {
        // 对每种类型测试 filter
    }
}

// === 大数据测试 ===

#[test]
fn test_storage_api_large_value() {
    // 测试 varchar(max), blob 等大值
    let large_value = Value::Varchar("x".repeat(1_000_000));
}

#[test]
fn test_storage_api_many_columns() {
    // 100+ 列的表
    let columns: Vec<_> = (0..100)
        .map(|i| Column::new(format!("col_{i}"), ColumnType::Int64, false, i))
        .collect();
}
```

### 4.2 事务并发测试 (关键)

```rust
// src/storage/concurrency_tests.rs

// === 事务隔离级别测试 ===

// 问题: test_isolation_dirty_read 检测脏读
#[test]
fn test_isolation_dirty_read() {
    // T1: begin → insert (不 commit)
    // T2: 读取 → 应该看不到
    // RC 隔离级别应防止脏读
}

// 问题: test_isolation_non_repeatable_read 检测不可重复读
#[test]
fn test_isolation_non_repeatable_read() {
    // T1: begin → read row
    // T2: begin → update row → commit
    // T1: read again → 应该看到新值 (RC) / 旧值 (RR)
}

// 问题: test_isolation_phantom_read 检测幻读
#[test]
fn test_isolation_phantom_read() {
    // T1: SELECT WHERE id > 100
    // T2: INSERT new row WHERE id > 100 → commit
    // T1: SELECT again → 是否看到新行
}

// === 并发写入测试 ===

// 问题: test_concurrent_write_conflict 检测写-写冲突
#[test]
fn test_concurrent_write_conflict() {
    let storage = Arc::new(Mutex::new(StorageEngine::new(temp_dir()).unwrap()));
    
    let h1 = {
        let s = Arc::clone(&storage);
        thread::spawn(move || {
            let mut s = s.lock().unwrap();
            s.insert("test", values(1)).unwrap()
        })
    };
    
    let h2 = {
        let s = Arc::clone(&storage);
        thread::spawn(move || {
            let mut s = s.lock().unwrap();
            s.insert("test", values(1)).unwrap()
        })
    };
    
    // 检测: 是否有死锁/超时
}

// 问题: test_concurrent_update_same_row 检测同一行更新
#[test]
fn test_concurrent_update_same_row() {
    // T1: UPDATE users SET age=30 WHERE id=1
    // T2: UPDATE users SET age=31 WHERE id=1
    // 检测: 最终结果一致性
}

// === 长短事务测试 ===

// 问题: test_long_transaction_block_short 检测长事务阻塞
#[test]
fn test_long_transaction_block_short() {
    // T1: begin (持有锁长时间不释放)
    // T2: 尝试短操作 → 超时
}

// 问题: test_short_transaction_priority 检测短事务优先级
#[test]
fn test_short_transaction_priority() {
    // 多个长事务 + 1 个短事务
    // 检测: 短事务是否被优先处理
}

// === 死锁检测测试 ===

// 问题: test_deadlock_detection_and_recovery 检测死锁恢复
#[test]
fn test_deadlock_detection_and_recovery() {
    // T1: lock(A) → lock(B)
    // T2: lock(B) → lock(A)
    // 期望: 一个事务回滚，另一个继续
}
```

### 4.3 恢复/持久性测试

```rust
// src/storage/recovery_tests.rs

// === Crash Recovery 测试 ===

// 问题: test_crash_recovery_insert 检测插入后 crash 恢复
#[test]
fn test_crash_recovery_insert() {
    let mut storage = StorageEngine::new(temp_dir()).unwrap();
    storage.create_table("test", columns()).unwrap();
    
    // Insert + commit
    storage.insert("test", values(1)).unwrap();
    
    // Simulate crash: drop storage, recreate
    drop(storage);
    let storage = StorageEngine::new(temp_dir()).unwrap();
    
    // 应该能恢复
    let results = storage.scan("test", None).unwrap();
    assert_eq!(results.len(), 1);
}

// 问题: test_crash_recovery_partial_commit 检测部分提交
#[test]
fn test_crash_recovery_partial_commit() {
    // T1: begin → insert → commit (wal 写入但未 flush)
    // crash → 恢复
    // 检测: 是否恢复到 commit 后的状态
}

// 问题: test_crash_recovery_rollback 检测回滚恢复
#[test]
fn test_crash_recovery_rollback() {
    // T1: begin → insert → abort
    // crash → 恢复
    // 检测: 插入的数据是否被回滚
}

// === WAL 完整性测试 ===

// 问题: test_wal_corruption_recovery 检测日志损坏恢复
#[test]
fn test_wal_corruption_recovery() {
    // 模拟 WAL 文件损坏 (随机字节)
    // 恢复后检测: 能跳过损坏部分继续恢复
}

// 问题: test_checkpoint_recovery 检测检查点恢复
#[test]
fn test_checkpoint_recovery() {
    // 创建 checkpoint
    // 大量操作
    // crash → 从 checkpoint 恢复
}
```

### 4.4 内存/资源压力测试

```rust
// src/storage/stress_tests.rs

// === 内存泄漏测试 ===

// 问题: test_memory_leak_large_dataset 检测大数据集内存泄漏
#[test]
fn test_memory_leak_large_dataset() {
    let mut storage = StorageEngine::new(temp_dir()).unwrap();
    
    // 插入 1M rows
    for i in 0..1_000_000 {
        storage.insert("test", values(i)).unwrap();
    }
    
    // 扫描多次
    for _ in 0..100 {
        storage.scan("test", None).unwrap();
    }
    
    // 检测: 内存是否正常释放
    // 注意: 需要外部监控工具
}

// 问题: test_memory_leak_many_transactions 检测事务内存泄漏
#[test]
fn test_memory_leak_many_transactions() {
    // 大量短事务
    for _ in 0..10_000 {
        let mut storage = StorageEngine::new(temp_dir()).unwrap();
        storage.insert("test", values(1)).unwrap();
    }
}

// === 资源耗尽测试 ===

// 问题: test_buffer_pool_exhaustion 检测缓冲区池耗尽
#[test]
fn test_buffer_pool_exhaustion() {
    // 大量并发读取不同 page
    // 检测: LRU eviction 是否正常工作
}

// 问题: test_lock_manager_exhaustion 检测锁表耗尽
#[test]
fn test_lock_manager_exhaustion() {
    // 大量并发事务持有不同锁
    // 检测: 锁表是否正常
}

// 问题: test_disk_space_exhaustion 检测磁盘空间耗尽
#[test]
fn test_disk_space_exhaustion() {
    // 模拟磁盘满
    // 检测: 错误处理是否正确
}
```

---

## 5. 问题发现矩阵

| 测试名称 | 目标问题 | 风险级别 | 模块 | 预期结果 |
|-----------|----------|----------|------|----------|
| `test_buffer_state_race` | Atomic state race | 高 | buffer | 发现 pin count 泄漏 |
| `test_hash_table_concurrent_lookup_insert` | Hash 竞态 | 高 | buffer | 发现 use-after-free |
| `test_buffer_dirty_pin_concurrent` | Dirty flag 竞态 | 中 | buffer | 发现状态不一致 |
| `test_storage_tables_not_thread_safe` | tables HashMap 非线程安全 | **极高** | storage | 发现数据竞争 |
| `test_concurrent_write_conflict` | 死锁/超时 | 高 | lock | 验证死锁检测 |
| `test_isolation_dirty_read` | 脏读 | 高 | storage | 验证 RC 隔离 |
| `test_crash_recovery_partial_commit` | 部分提交丢失 | 高 | storage | 发现 WAL 丢失 |
| `test_memory_leak_large_dataset` | 内存泄漏 | 高 | storage/buffer | 发现 RSS 增长 |
| `test_wal_group_commit` | Group commit 不工作 | 中 | wal | 发现 batch 问题 |
| `test_livelock_starvation` | 活锁 | 中 | lock | 发现 writer 饥饿 |

---

## 6. 执行计划

### Phase 1: 基础测试补充 (Day 1-2)
- [ ] 补充 buffer 模块并发测试 (6 tests)
- [ ] 补充 lock 模块死锁测试 (5 tests)
- [ ] 补充 wal 模块恢复测试 (5 tests)

### Phase 2: StorageApi 集成测试 (Day 3-4)
- [ ] CRUD 完整性测试 (8 tests)
- [ ] 事务并发测试 (10 tests)
- [ ] 恢复/持久性测试 (6 tests)

### Phase 3: 压力测试 (Day 5)
- [ ] 内存泄漏测试 (3 tests)
- [ ] 资源耗尽测试 (3 tests)
- [ ] 长时间运行测试

### Phase 4: 问题验证 (Day 6+)
- [ ] 运行所有新测试
- [ ] 分析失败测试
- [ ] 修复/记录发现的问题

---

## 7. 运行命令

```bash
# 运行所有测试
cargo test --lib

# 运行特定模块测试
cargo test --lib buffer
cargo test --lib lock
cargo test --lib wal

# 运行并发测试
cargo test concurrency

# 运行集成测试
cargo test integration

# 运行恢复测试
cargo test recovery

# 运行压力测试
cargo test stress

# 使用 Miri 检测未定义行为 (UCG)
cargo +nightly miri test

# 内存泄漏检测
cargo install cargo-malloc
cargo malloc test

# 性能分析
cargo bench
```

---

## 8. 预期发现的问题

基于代码分析，预计会发现以下问题:

1. **StorageEngine.tables 非线程安全** - `HashMap<String, HeapTable>` 无同步
2. **Buffer atomic state 潜在竞态** - dirty bit 和 pin count 的读写顺序
3. **WAL group commit 配置** - batch_size 和 timeout 可能未生效
4. **Lock waiter promotion 并发** - 超时时的队列操作
5. **MVCC undo pointer 有效性** - page eviction 后指针可能失效

---

## 9. 风险与缓解

| 风险 | 缓解措施 |
|------|----------|
| 并发测试不稳定 | 使用适当的 barrier/sleep，使用 thread::spawn |
| 内存泄漏检测困难 | 使用外部监控 (top, /proc/self/status) |
| 恢复测试复杂 | 使用 tempfile + 模拟 crash |
| 测试执行时间过长 | 分离快速/慢速测试，使用 #[ignore] |
