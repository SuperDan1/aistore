# P0 实现计划: 核心持久化与并发控制

## 目标
实现高并发低时延存储引擎的核心功能，支持 crash recovery 和事务并发。

---

## P0-1: WAL (Write-Ahead Log)

### 1.1 设计目标
- 支持 crash recovery，事务 ACID 中的 D (Durability)
- 顺序写入，优化写入性能
- 支持组提交 (Group Commit) 减少 I/O

### 1.2 核心数据结构

```rust
// Log Sequence Number
pub type LSN = u64;

// WAL Record 结构
pub struct LogRecord {
    pub lsn: LSN,                    // 日志序列号
    pub tx_id: TransactionId,        // 事务ID
    pub op: LogOp,                   // 操作类型
    pub page_id: PageId,             // 关联页面
    pub before: Option<Vec<u8>>,     // 修改前镜像 (undo)
    pub after: Option<Vec<u8>>,      // 修改后镜像 (redo)
    pub timestamp: u64,              // 时间戳
}

pub enum LogOp {
    Insert,
    Update,
    Delete,
    PageWrite,   // 页面完整写入
    Commit,
    Abort,
}
```

### 1.3 模块结构

```
src/wal/
├── mod.rs          # WAL 入口
├── segment.rs      # WAL segment 管理 (64MB/segment)
├── log_writer.rs   # 顺序写入器
├── log_reader.rs   # 日志读取/恢复
├── checkpoint.rs    # Checkpoint 机制
└── recovery.rs     # 恢复流程
```

### 1.4 关键接口

```rust
pub trait WAL {
    // 写入日志 (同步)
    fn append(&self, record: &LogRecord) -> Result<LSN>;
    
    // 写入日志 (异步，组提交)
    fn append_async(&self, record: &LogRecord) -> Result<LSN>;
    
    // 提交事务
    fn commit(&self, tx_id: TransactionId) -> Result<LSN>;
    
    // 回滚事务
    fn abort(&self, tx_id: TransactionId) -> Result<()>;
    
    // 获取 checkpoint 点
    fn get_checkpoint(&self) -> Option<LSN>;
    
    // 恢复
    fn recover(&self) -> Vec<Transaction>;
}
```

### 1.5 实现步骤

| 步骤 | 内容 | 预计工作量 |
|------|------|-----------|
| 1 | 创建 wal/ 模块骨架 | 0.5天 |
| 2 | 实现 LogRecord 和序列化 | 1天 |
| 3 | 实现 WALSegment 循环写入 | 2天 |
| 4 | 实现组提交 (Group Commit) | 2天 |
| 5 | 实现 Checkpoint 机制 | 1天 |
| 6 | 实现 Recovery 恢复流程 | 2天 |
| 7 | 集成到 StorageEngine | 1天 |

**小计: 9.5天**

---

## P0-2: BufferPool 持久化

### 2.1 设计目标
- 将内存中的 dirty page 刷到磁盘
- 支持 crash recovery
- 减少随机 I/O，优化写入

### 2.2 核心数据结构

```rust
// 页面状态 (已有 atomic)
const DIRTY_BIT: u64 = 1 << 0;
const PIN_COUNT_SHIFT: u8 = 8;

// Flush 策略
pub enum FlushPolicy {
    // 定时刷新
    Interval { ms: u64 },
    // 达到阈值刷新
    Threshold { dirty_ratio: f64 },
    // 后台线程刷新
    Background { threads: usize },
}

// PageWriter 输出
pub struct PageWrite {
    pub page_id: PageId,
    pub lsn: LSN,
    pub checksum: u32,
}
```

### 2.3 模块结构

```
src/buffer/
├── mod.rs           # 已有 BufferMgr
├── flush.rs        # 页面刷新策略
├── double_write.rs  # Double Write Buffer (防止 partial write)
└── evictor.rs      # 淘汰策略
```

### 2.4 关键接口 (扩展现有 BufferMgr)

```rust
// 扩展 BufferMgr
impl BufferMgr {
    // 标记页面为 dirty
    fn mark_dirty(&self, tag: BufferTag);
    
    // 获取需要 flush 的页面
    fn get_dirty_pages(&self) -> Vec<PageId>;
    
    // 刷脏页
    fn flush_page(&self, page_id: PageId) -> Result<()>;
    
    // 刷所有脏页
    fn flush_all(&self) -> Result<()>;
    
    // 后台 flush 线程
    fn start_flusher(&self, policy: FlushPolicy);
}
```

### 2.5 Double Write Buffer

```
目的: 防止 partial write (断电导致半页写入)

设计:
1. 先写入 doublewrite buffer (顺序)
2. 再写入目标位置 (随机)
3. 成功后从 buffer 中清除
4. Recovery 时从 buffer 恢复
```

### 2.6 实现步骤

| 步骤 | 内容 | 预计工作量 |
|------|------|-----------|
| 1 | 扩展 BufferMgr 增加 dirty tracking | 1天 |
| 2 | 实现 FlushPolicy 策略 | 1天 |
| 3 | 实现 Double Write Buffer | 2天 |
| 4 | 实现后台 flusher 线程 | 2天 |
| 5 | 实现 partial write 检测 | 1天 |
| 6 | 集成 WAL + BufferPool | 2天 |

**小计: 9天**

---

## P0-3: 并发控制

### 3.1 设计目标
- 支持事务并发
- 提供隔离级别 (RC, RR)
- 防止脏读、脏写

### 3.2 核心数据结构

```rust
// 事务
pub struct Transaction {
    pub tx_id: TransactionId,
    pub status: TxStatus,
    pub start_lsn: LSN,
    pub isolation: IsolationLevel,
}

pub enum TxStatus {
    Active,
    Committed,
    Aborted,
}

pub enum IsolationLevel {
    ReadCommitted,  // RC - 每次读取最新已提交
    RepeatableRead, // RR - 事务内读取一致
}

// 行锁
pub struct RowLock {
    pub row_id: RowId,
    pub tx_id: TransactionId,
    pub lock_mode: LockMode,
    pub granted: bool,
}

pub enum LockMode {
    Shared,    // S 锁 (读)
    Exclusive, // X 锁 (写)
}
```

### 3.3 模块结构

```
src/lock/
├── mod.rs          # LockManager 入口
├── row_lock.rs     # 行级锁
├── table_lock.rs   # 表级锁
├── wait_graph.rs   # 死锁检测
└── mvcc.rs        # MVCC (可选)
```

### 3.4 关键接口

```rust
pub trait LockManager {
    // 获取行锁
    fn lock_row(&self, tx_id: TransactionId, row_id: RowId, mode: LockMode) -> Result<()>;
    
    // 释放行锁
    fn unlock_row(&self, tx_id: TransactionId, row_id: RowId);
    
    // 获取表锁
    fn lock_table(&self, tx_id: TransactionId, table_id: TableId, mode: LockMode) -> Result<()>;
    
    // 提交事务，释放所有锁
    fn commit(&self, tx_id: TransactionId) -> Result<()>;
    
    // 回滚事务，释放所有锁
    fn rollback(&self, tx_id: TransactionId) -> Result<()>;
}
```

### 3.5 2PL (Two-Phase Locking)

```
阶段1: Growing - 可以获取锁，不能释放锁
阶段2: Shrinking - 可以释放锁，不能获取锁

防止: 脏写, 丢失更新
```

### 3.6 实现步骤

| 步骤 | 内容 | 预计工作量 |
|------|------|-----------|
| 1 | 创建 lock/ 模块骨架 | 0.5天 |
| 2 | 实现 Transaction 和事务管理 | 1天 |
| 3 | 实现行级锁 (排他锁) | 2天 |
| 4 | 实现 2PL 协议 | 2天 |
| 5 | 实现死锁检测 | 2天 |
| 6 | 集成到 StorageEngine | 2天 |

**小计: 9.5天**

---

## P0 总结

| 模块 | 工作量 | 依赖 |
|------|--------|------|
| WAL | 9.5天 | 无 |
| BufferPool 持久化 | 9天 | WAL |
| 并发控制 | 9.5天 | 无 |

**总计: ~28天 (约 6 周)**

---

## 集成架构

```
┌─────────────────────────────────────────────────────────┐
│                    StorageEngine                        │
├─────────────────────────────────────────────────────────┤
│  ┌─────────┐  ┌──────────┐  ┌─────────────────────┐  │
│  │  SQL    │  │  Lock    │  │   Transaction       │  │
│  │ Executor │  │ Manager  │  │   Manager           │  │
│  └────┬────┘  └────┬─────┘  └──────────┬──────────┘  │
│       │            │                     │             │
│       │     ┌──────┴──────┐            │             │
│       │     │  BufferMgr   │            │             │
│       │     │ (LRU+Dirty)  │◄───────────┤             │
│       │     └──────┬───────┘            │             │
│       │            │                     │             │
│  ┌────┴────────────┴─────────────────────┴─────────┐   │
│  │              WAL (Write-Ahead Log)             │   │
│  │  - Log Record                                  │   │
│  │  - Group Commit                                │   │
│  │  - Checkpoint                                  │   │
│  └─────────────────────┬───────────────────────────┘   │
│                        │                               │
│                        ▼                               │
│  ┌───────────────────────────────────────────────────┐ │
│  │              VFS (File System)                    │ │
│  └───────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

---

## 验证计划

| 阶段 | 测试 |
|------|------|
| WAL | 1. 写日志后 crash recovery<br>2. 组提交性能测试 |
| BufferPool | 1. 脏页刷新正确性<br>2. Double write 恢复测试 |
| Lock | 1. 并发事务隔离测试<br>2. 死锁检测测试 |
| 集成 | 1. Sysbench 9 场景回归<br>2. 断电恢复测试 |

---

**请确认以上计划，确认后开始实现。**
