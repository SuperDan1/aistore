# WAL 设计文档

## 1. 模块结构

```
src/wal/                          # WAL模块
├── mod.rs                        # WalManager主模块
├── config.rs                     # 配置
├── lsn.rs                       # LSN定义
├── log_record.rs                # 日志记录格式
├── log_file.rs                  # 日志文件管理
├── log_buffer.rs                # 组提交缓冲区
├── checkpoint.rs                # 增量Checkpoint
└── recovery.rs                  # 崩溃恢复

src/mvcc/                        # MVCC模块（独立）
├── mod.rs                       # MvccManager
├── version_store.rs             # 版本存储（B1独立表空间）
├── visibility.rs                # 可见性判断
└── gc.rs                        # 定时GC

src/binlog/                      # 后续独立模块（当前不实现）
```

## 2. 需求规格

### 2.1 事务模型
- 支持MVCC（多版本并发控制）
- 暂不支持分布式事务（单节点）

### 2.2 持久化策略
- 异步写入 + 组提交
- 事务提交时等待对应日志刷盘，不允许丢失
- RTO目标：秒级

### 2.3 日志内容
- 物理日志（Redo日志）
- 逻辑日志（Binlog）后续独立实现

### 2.4 Checkpoint
- 增量Checkpoint
- 触发条件：时间间隔（默认60秒）

### 2.5 性能目标
- TPS：越高越好
- 日志文件大小：默认1GB，支持轮转

## 3. 核心设计

### 3.1 LSN (Log Sequence Number)

```rust
/// Log Sequence Number
/// [文件号(16bit)][文件内偏移(48bit)] = 64bit
pub struct LSN(u64);

impl LSN {
    pub fn new(file_id: u16, offset: u64) -> LSN;
    pub fn file_id(&self) -> u16;
    pub fn offset(&self) -> u64;
    pub fn invalid() -> LSN;
    pub fn is_valid(&self) -> bool;
}
```

### 3.2 日志记录格式

```rust
/// 日志记录头部（定长32字节）
struct LogRecordHeader {
    lsn: LSN,                    // 日志序列号
    tx_id: TransactionId,        // 事务ID
    prev_lsn: LSN,               // 事务上一条日志LSN
    log_type: LogType,           // 日志类型
    payload_len: u32,            // 数据长度
    checksum: u32,                // CRC32校验
}

/// 日志类型
enum LogType {
    // 物理Redo
    PageRedo { page_id: PageId, offset: u32, data: Vec<u8> },
    
    // 事务边界
    TxBegin { tx_id: TransactionId },
    TxCommit { tx_id: TransactionId },
    TxAbort  { tx_id: TransactionId },
    
    // Checkpoint
    Checkpoint { checkpoint_id: u64, lsn: LSN, dirty_pages: Vec<PageId> },
}
```

### 3.3 WAL Manager

```rust
pub struct WalManager {
    config: WalConfig,
    log_files: RwLock<Vec<LogFile>>,    // 日志文件列表
    active_log: RwLock<LogFile>,        // 当前活跃日志文件
    log_buffer: Arc<LogBuffer>,        // 组提交缓冲区
    checkpoint_mgr: CheckpointManager, // Checkpoint管理
    flush_lsn: AtomicU64,              // 已刷盘的最大LSN
}

impl WalManager {
    /// 追加日志（异步，返回LSN）
    pub fn append(&self, tx_id: TransactionId, record: LogRecord) -> LSN;
    
    /// 事务提交（等待对应LSN刷盘）
    pub fn commit(&self, tx_id: TransactionId) -> Result<LSN, WalError>;
    
    /// 获取当前活跃LSN
    pub fn get_active_lsn(&self) -> LSN;
    
    /// 强制刷盘
    pub fn flush(&self) -> Result<(), WalError>;
    
    /// 增量Checkpoint
    pub fn checkpoint(&self) -> Result<LSN, WalError>;
    
    /// 崩溃恢复
    pub fn recover(&self) -> RecoveryResult;
}
```

### 3.4 组提交

- 内存缓冲区：8MB
- 组提交条件：batch >= 4 或 timeout >= 10ms
- 事务提交时等待自己对应的LSN刷盘

```rust
pub struct LogBuffer {
    buffer: RingBuffer,          // 环形缓冲区
    pending: Mutex<Vec<PendingRecord>>,
    group_commit: GroupCommit,
    flusher: Flusher,
}
```

### 3.5 增量Checkpoint

- 触发：定时60秒
- 记录：脏页列表 + 活跃事务列表 + 目录快照

```rust
struct CheckpointRecord {
    checkpoint_id: u64,
    begin_lsn: LSN,
    end_lsn: LSN,
    dirty_pages: Vec<PageId>,
    active_transactions: Vec<TransactionId>,
    catalog_snapshot: Vec<u8>,
}
```

### 3.6 MVCC（B1独立表空间）

- 版本存放在独立表空间 `version_space`
- 使用tx_id作为偏序判断可见性
- 后台定时GC线程

```rust
/// 版本记录
struct VersionRecord {
    row_key: RowKey,              // 行键（table_id + row_id）
    version_id: u64,              // 版本号
    created_by_tx: TransactionId, // 创建事务ID
    deleted_by_tx: Option<TransactionId>, // 删除事务ID
    data: Vec<u8>,                // 行数据
}
```

### 3.7 崩溃恢复流程

1. 加载最新Checkpoint
2. 从Checkpoint LSN重做Redo日志
3. 回滚未提交事务（依赖MVCC的标记）
4. 恢复完成

## 4. 配置参数

```rust
pub struct WalConfig {
    pub log_dir: PathBuf,              // 日志目录
    pub max_file_size: u64,            // 单文件大小（1GB）
    pub buffer_size: usize,            // 内存缓冲（8MB）
    pub group_commit_batch: usize,     // 组提交batch（4）
    pub group_commit_timeout_ms: u64,  // 组提交超时（10ms）
    pub checkpoint_interval_sec: u64,  // Checkpoint间隔（60s）
    pub enabled: bool,                // 是否启用WAL
}
```

## 5. 文件格式

### 5.1 WAL日志文件

```
+----------------------------------+
| Magic Number (4B): 0x57414C31    |
| Version (4B): 0x00000001         |
+----------------------------------+
| Log Record 1                     |
| Log Record 2                     |
| ...                               |
| Log Record N                     |
+----------------------------------+
| Trailer: LSN of last record      |
+----------------------------------+
```

### 5.2 Checkpoint文件

```
+----------------------------------+
| Checkpoint ID                    |
| Begin LSN                        |
| End LSN                          |
| Dirty Pages Count                |
| Dirty Pages[PageId]             |
| Active Transactions              |
| Catalog Snapshot                 |
+----------------------------------+
```

## 6. 实现顺序

1. WAL基础：LSN + 日志格式 + 文件管理
2. 组提交：LogBuffer + 刷盘线程
3. Checkpoint：增量Checkpoint
4. 恢复：崩溃恢复流程
5. 集成：StorageEngine集成WAL
6. MVCC基础：版本存储 + 可见性判断
7. GC：后台GC线程
