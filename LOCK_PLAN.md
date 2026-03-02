# 并发控制模块实现计划

## 目标
实现事务并发控制，支持 ACID 中的 I (Isolation)，防止脏读、脏写、丢失更新。

---

## 1. 设计决策

### 1.1 锁粒度
- **行级锁 (Row Lock)**: 细粒度，并发度高
- **表级锁 (Table Lock)**: 粗粒度，用于 DDL 操作

### 1.2 锁模式
| 模式 | 简称 | 允许 | 禁止 |
|------|------|------|------|
| Shared (S) | 共享锁 | 其他 S 锁 | X 锁 |
| Exclusive (X) | 排他锁 | 任何锁 | 任何锁 |

### 1.3 隔离级别
- **Read Committed (RC)**: 每次读取最新已提交的数据 (默认)

### 1.4 锁协议
采用 **2PL (Two-Phase Locking)**:
- **Growing Phase**: 可以获取锁，不能释放锁
- **Shrinking Phase**: 可以释放锁，不能获取锁

### 1.5 超时机制
- 锁等待超时: 默认 30 秒，可配置
- 超时后返回 `Error::LockTimeout`
- 避免无限等待

---

## 2. 核心数据结构

```rust
// 事务
pub struct Transaction {
    pub tx_id: TransactionId,        // 事务ID (全局递增)
    pub status: TxStatus,          // 事务状态
    pub start_lsn: LSN,            // 事务开始时的 LSN
    pub locks: Vec<LockRequest>,   // 已持有的锁
}

pub type TransactionId = u64;

pub enum TxStatus {
    Active,     // 运行中
    Committed, // 已提交
    Aborted,   // 已回滚
}

// 锁请求
pub struct LockRequest {
    pub resource: Resource,
    pub mode: LockMode,
    pub granted: bool,
}

// 锁模式
pub enum LockMode {
    Shared,     // S 锁 - 读
    Exclusive, // X 锁 - 写
}
```

---

## 3. 模块结构

```
src/lock/
├── mod.rs          # LockManager 入口
├── transaction.rs  # 事务管理
├── row_lock.rs    # 行级锁实现
├── table_lock.rs  # 表级锁实现
└── dead_lock.rs   # 死锁检测
```

---

## 4. 接口设计

### 4.1 修改 StorageEngine 接口

```rust
// 新增: 事务管理
pub fn begin_transaction(&mut self) -> TransactionId;
pub fn commit(&mut self, tx_id: TransactionId) -> Result<()>;
pub fn abort(&mut self, tx_id: TransactionId) -> Result<()>;

// 修改: 插入 - 增加 tx_id 参数
pub fn insert(&mut self, tx_id: TransactionId, table: &str, values: Vec<Value>) -> Result<RowId>;

// 修改: 扫描 - 增加 tx_id 参数  
pub fn scan(&mut self, tx_id: TransactionId, table: &str, filter: Option<Filter>) -> Result<Vec<Tuple>>;

// 修改: 更新
pub fn update(&mut self, tx_id: TransactionId, table: &str, row_id: RowId, values: Vec<Value>) -> Result<()>;

// 修改: 删除
pub fn delete(&mut self, tx_id: TransactionId, table: &str, row_id: RowId) -> Result<()>;
```

### 4.2 LockManager 内部接口

```rust
pub trait LockManager: Send + Sync {
    fn lock_row(&self, tx_id: TransactionId, row_id: RowId, mode: LockMode) -> Result<()>;
    fn unlock_row(&self, tx_id: TransactionId, row_id: RowId);
    fn lock_table(&self, tx_id: TransactionId, table_id: TableId, mode: LockMode) -> Result<()>;
    fn unlock_table(&self, tx_id: TransactionId, table_id: TableId);
    fn commit(&self, tx_id: TransactionId) -> Result<()>;
    fn abort(&self, tx_id: TransactionId) -> Result<()>;
    fn set_timeout(&self, duration: Duration);
}
```

---

## 5. 实现步骤

| 步骤 | 内容 | 预计工作量 |
|------|------|-----------|
| 1 | 创建 lock/ 模块骨架 | 0.5天 |
| 2 | 实现 Transaction 和事务管理 | 1天 |
| 3 | 实现 LockTable 锁表数据结构 | 1天 |
| 4 | 实现行级 S 锁 (读) | 1天 |
| 5 | 实现行级 X 锁 (写) + 超时 | 1.5天 |
| 6 | 实现表级锁 | 0.5天 |
| 7 | 实现 2PL 协议 + 死锁检测 | 1.5天 |
| 8 | 修改 StorageEngine 接口 | 1天 |
| 9 | 集成测试 | 2天 |

**小计: 10天**

---

## 6. 错误类型

```rust
#[derive(Debug)]
pub enum LockError {
    Timeout,           // 锁等待超时
    Deadlock,          // 检测到死锁
    TransactionNotFound,
    ResourceNotFound,
    Conflict,          // 锁模式冲突
}
```

---

## 确认

设计已更新：
- ✅ 只支持 Read Committed
- ✅ 等待超时机制 (默认30秒)
- ✅ 修改现有接口 (增加 tx_id 参数)
- ⏳ MVCC 暂时不做，等待后续讨论

**是否确认此计划开始实现？**
