# MVCC 实现详细设计

## 1. 整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│                      StorageEngine                              │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐ │
│  │   SQL        │  │   Lock       │  │   Transaction        │ │
│  │   Executor   │  │   Manager    │  │   Manager             │ │
│  └──────┬───────┘  └──────┬───────┘  └──────────┬───────────┘ │
│         │                  │                       │              │
│         │           ┌──────┴───────┐               │              │
│         │           │  BufferMgr   │◄──────────────┤              │
│         │           │ (LRU+Dirty)  │               │              │
│         │           └──────┬───────┘               │              │
│         │                  │                       │              │
│  ┌──────┴─────────────────┴───────────────────────┴───────────┐ │
│  │                    MVCC Layer                                 │ │
│  │  ┌────────────────┐  ┌────────────────┐  ┌───────────────┐  │ │
│  │  │ VersionChain   │  │  UndoManager   │  │ SnapshotMgr   │  │ │
│  │  │ Manager        │  │  (Undo Tblsp)  │  │               │  │ │
│  │  └───────┬────────┘  └───────┬────────┘  └───────┬───────┘  │ │
│  └──────────┴───────────────────┴─────────────────────┴──────────┘ │
│                        │                                           │
│         ┌──────────────┴──────────────┐                           │
│         │            WAL                │                           │
│         │  (Redo + Undo Log)           │                           │
│         └──────────────┬──────────────┘                           │
│                        ▼                                           │
│         ┌─────────────────────────────────┐                        │
│         │           VFS                   │                        │
│         └─────────────────────────────────┘                        │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. 核心数据结构

### 2.1 行头扩展（Row Header）

每行新增 MVCC 元数据，存储在行数据开头：

```rust
/// 行级 MVCC 元数据（存储在每行开头）
#[derive(Debug, Clone, Copy)]
pub struct RowMVCCHeader {
    /// 创建该版本的事务ID
    pub tx_id_created: TransactionId,
    /// 删除该版本的事务ID (0 表示未删除)
    pub tx_id_deleted: TransactionId,
    /// 指向 Undo Record 的指针 (页号 + 页内偏移)
    pub undo_ptr: UndoPtr,
    /// 行数据长度（用于快速计算）
    pub row_length: u16,
}

impl RowMVCCHeader {
    pub const SIZE: usize = 8 + 8 + 16 + 2; // 34 bytes
    
    pub fn new(tx_id: TransactionId, undo_ptr: UndoPtr, row_length: u16) -> Self {
        Self {
            tx_id_created: tx_id,
            tx_id_deleted: 0,
            undo_ptr,
            row_length,
        }
    }
}

/// Undo 指针
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UndoPtr {
    /// Undo 页面ID
    pub page_id: PageId,
    /// 页内偏移量
    pub offset: u16,
    /// Undo 记录的LSN（用于恢复）
    pub lsn: LSN,
}

impl UndoPtr {
    pub fn null() -> Self {
        Self {
            page_id: 0,
            offset: 0,
            lsn: 0,
        }
    }

    pub fn is_null(&self) -> bool {
        self.page_id == 0 && self.offset == 0
    }
}
```

### 2.2 Undo Record

Undo 记录存储在独立的 Undo Tablespace：

```rust
/// Undo 记录头
#[derive(Debug, Clone, Copy)]
pub struct UndoRecordHeader {
    /// 记录长度（包括 header）
    pub length: u32,
    /// 事务ID
    pub tx_id: TransactionId,
    /// 所属表ID
    pub table_id: TableId,
    /// 行ID (page_id + slot_idx)
    pub row_id: RowId,
    /// 前一个 Undo 记录的指针（形成链）
    pub prev_undo_ptr: UndoPtr,
    /// Undo 类型
    pub undo_type: UndoType,
    /// 校验和
    pub checksum: u32,
}

/// Undo 操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoType {
    /// 插入操作（回滚时删除）
    Insert,
    /// 更新操作（回滚时用 before_image 覆盖）
    Update,
    /// 删除操作（回滚时恢复）
    Delete,
}

/// 完整的 Undo 记录
pub struct UndoRecord {
    pub header: UndoRecordHeader,
    /// 变更前的行数据（对于 Insert 为空）
    pub before_image: Vec<u8>,
}

impl UndoRecord {
    /// 计算序列化后的大小
    pub fn serialized_size(&self) -> usize {
        std::mem::size_of::<UndoRecordHeader>() + self.before_image.len()
    }
}
```

### 2.3 Read Snapshot

```rust
/// Read 快照（事务可见性判断用）
#[derive(Debug, Clone)]
pub struct ReadSnapshot {
    /// 事务ID
    pub tx_id: TransactionId,
    /// 快照创建时间（事务开始时的 LSN）
    pub snapshot_lsn: LSN,
    /// 快照创建时的最大已提交事务ID
    pub max_committed_tx: TransactionId,
    /// 快照创建时所有活跃的事务ID集合
    pub active_txns: Vec<TransactionId>,
    /// 隔离级别
    pub isolation: IsolationLevel,
    /// 快照创建时间（用于超时判断）
    pub created_at: Instant,
}

impl ReadSnapshot {
    /// 判断一个版本对当前快照是否可见
    pub fn is_visible(&self, version: &RowVersion) -> bool {
        let created = version.tx_id_created;
        let deleted = version.tx_id_deleted;

        match self.isolation {
            IsolationLevel::ReadCommitted => {
                // RC: 读取最新已提交的版本
                // 可见条件：创建事务已提交
                let created_committed = created <= self.max_committed_tx 
                    || !self.is_active(created);
                
                // 未删除 或 删除事务未提交
                let not_deleted = deleted == 0 
                    || deleted > self.max_committed_tx 
                    || self.is_active(deleted);
                
                created_committed && not_deleted
            }
            IsolationLevel::RepeatableRead => {
                // RR: 读取事务开始时的快照
                // 可见条件：创建事务在快照时已提交
                let created_committed = created < self.tx_id 
                    && !self.was_active(created);
                
                // 未删除 且 删除事务在快照时未开始 或 已提交
                let not_deleted = deleted == 0 
                    || (deleted >= self.tx_id && !self.was_active(deleted));
                
                created_committed && not_deleted
            }
        }
    }

    /// 判断事务是否当前活跃
    fn is_active(&self, tx_id: TransactionId) -> bool {
        self.active_txns.contains(&tx_id)
    }

    /// 判断事务在快照创建时是否活跃（RR 用）
    fn was_active(&self, tx_id: TransactionId) -> bool {
        // RR: 在 tx_id < snapshot.tx_id 时，如果 tx_id 活跃，则认为历史活跃
        tx_id < self.tx_id && self.is_active(tx_id)
    }
}

/// 行版本信息（从页面读取）
#[derive(Debug, Clone)]
pub struct RowVersion {
    pub tx_id_created: TransactionId,
    pub tx_id_deleted: TransactionId,
    pub undo_ptr: UndoPtr,
    pub is_current: bool,
}
```

### 2.4 事务状态表

```rust
/// 事务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxStatus {
    Active,
    Committed,
    Aborted,
}

/// 事务状态记录
#[derive(Debug, Clone, Copy)]
pub struct TxState {
    pub tx_id: TransactionId,
    pub status: TxStatus,
    /// 事务开始时的 LSN
    pub start_lsn: LSN,
    /// 事务结束时的 LSN（提交/回滚）
    pub end_lsn: LSN,
    /// 开始时间（用于 GC 判断）
    pub start_time: Instant,
}

/// 事务状态管理器
pub struct TxStateManager {
    /// 事务状态表
    states: RwLock<HashMap<TransactionId, TxState>>,
    /// 活跃事务集合
    active_txns: RwLock<HashSet<TransactionId>>,
    /// 已提交事务的最大ID
    max_committed_tx: AtomicU64,
    /// 已分配的最大事务ID
    max_tx_id: AtomicU64,
}

impl TxStateManager {
    /// 分配新事务ID
    pub fn allocate_tx(&self) -> TransactionId {
        self.max_tx_id.fetch_add(1, Ordering::SeqCst)
    }

    /// 事务开始
    pub fn tx_start(&self, tx_id: TransactionId, start_lsn: LSN) {
        let state = TxState {
            tx_id,
            status: TxStatus::Active,
            start_lsn,
            end_lsn: 0,
            start_time: Instant::now(),
        };
        self.states.write().insert(tx_id, state);
        self.active_txns.write().insert(tx_id);
    }

    /// 事务提交
    pub fn tx_commit(&self, tx_id: TransactionId, commit_lsn: LSN) {
        let mut states = self.states.write();
        if let Some(state) = states.get_mut(&tx_id) {
            state.status = TxStatus::Committed;
            state.end_lsn = commit_lsn;
        }
        self.active_txns.write().remove(&tx_id);
        
        // 更新最大已提交事务ID
        let mut current_max = self.max_committed_tx.load(Ordering::SeqCst);
        while tx_id > current_max {
            if self.max_committed_tx.compare_exchange(
                current_max, tx_id, Ordering::SeqCst, Ordering::SeqCst
            ).is_ok() {
                break;
            }
            current_max = self.max_committed_tx.load(Ordering::SeqCst);
        }
    }

    /// 事务回滚
    pub fn tx_abort(&self, tx_id: TransactionId) {
        let mut states = self.states.write();
        if let Some(state) = states.get_mut(&tx_id) {
            state.status = TxStatus::Aborted;
        }
        self.active_txns.write().remove(&tx_id);
    }

    /// 获取活跃事务列表
    pub fn get_active_txns(&self) -> Vec<TransactionId> {
        self.active_txns.read().iter().copied().collect()
    }

    /// 获取最大已提交事务ID
    pub fn get_max_committed_tx(&self) -> TransactionId {
        self.max_committed_tx.load(Ordering::SeqCst)
    }
}
```

---

## 3. Undo Tablespace 设计

### 3.1 表空间结构

```
undo_tablespace/
├── undo_0000.seg    # 第一个 Undo 段 (64MB)
├── undo_0001.seg    # 第二个 Undo 段
└── ...
```

### 3.2 页面格式

```rust
/// Undo 页面头
#[derive(Debug, Clone, Copy)]
pub struct UndoPageHeader {
    /// 页面ID
    pub page_id: PageId,
    /// 页面类型
    pub page_type: PageType,
    /// 最后一个有效记录的偏移量
    pub last_record_offset: u16,
    /// 第一个空闲字节偏移量
    pub free_start: u16,
    /// 页面空闲空间
    pub free_space: u16,
    /// 事务ID（该页面内记录所属的第一个事务）
    pub tx_id: TransactionId,
}

impl UndoPageHeader {
    pub const SIZE: usize = 8 + 2 + 2 + 2 + 2 + 8; // 24 bytes
    pub const DATA_SIZE: usize = PAGE_SIZE - Self::SIZE;
}

/// Undo 段
pub struct UndoSegment {
    /// 段ID
    pub seg_id: u32,
    /// 文件路径
    pub file_path: PathBuf,
    /// 当前页面
    pub current_page: Option<PageId>,
    /// 页面内的可用偏移
    pub offset_in_page: u16,
}

impl UndoSegment {
    pub const SEGMENT_SIZE: usize = 64 * 1024 * 1024;
    pub const PAGE_SIZE: usize = 8192;
}
```

### 3.3 Undo Manager 接口

```rust
/// Undo Manager
pub struct UndoManager {
    /// Undo 表空间路径
    tablespace_path: PathBuf,
    /// 段管理器
    segments: RwLock<Vec<UndoSegment>>,
    /// 当前活跃段
    current_segment: RwLock<Option<u32>>,
    /// Buffer Pool（复用已有的）
    buffer_mgr: Arc<BufferMgr>,
    /// 锁（单写者）
    write_lock: Mutex<()>,
}

impl UndoManager {
    /// 追加 Undo 记录
    pub fn append(&self, record: &UndoRecord) -> Result<UndoPtr> {
        let _guard = self.write_lock.lock().unwrap();
        
        // 序列化
        let mut data = Vec::with_capacity(record.serialized_size());
        // ... 序列化 record ...
        
        // 写入页面
        let (page_id, offset) = self.write_to_page(&data)?;
        
        Ok(UndoPtr {
            page_id,
            offset,
            lsn: 0, // 由 WAL 填充
        })
    }

    /// 读取 Undo 记录
    pub fn get(&self, ptr: &UndoPtr) -> Result<UndoRecord> {
        // 从页面读取并反序列化
        todo!()
    }

    /// 获取事务的所有 Undo 记录（用于回滚）
    pub fn get_tx_undo_chain(&self, tx_id: TransactionId, first_ptr: UndoPtr) -> Result<Vec<UndoRecord>> {
        let mut records = Vec::new();
        let mut ptr = first_ptr;
        
        while !ptr.is_null() {
            let record = self.get(&ptr)?;
            if record.header.tx_id != tx_id {
                break;
            }
            records.push(record);
            ptr = record.header.prev_undo_ptr;
        }
        
        Ok(records)
    }

    /// 清理已提交事务的 Undo 记录（GC）
    pub fn purge_completed_tx(&self, oldest_active_tx: TransactionId) -> Result<usize> {
        // 清理 tx_id < oldest_active_tx 的已提交事务的 Undo
        todo!()
    }
}
```

---

## 4. 读流程（可见性判断）

### 4.1 获取可见版本

```rust
impl MVCC {
    /// 获取行的可见版本
    pub fn get_visible_version(
        &self,
        table_id: TableId,
        row_id: &RowId,
        snapshot: &ReadSnapshot,
    ) -> Result<Option<VersionedRow>> {
        // 1. 从页面读取当前版本
        let page = self.buffer_mgr.get_page(row_id.page_id)?;
        let slot = page.get_slot(row_id.slot_idx)?;
        
        // 2. 读取行头
        let header = RowMVCCHeader::from_bytes(slot)?;
        
        // 3. 检查当前版本是否可见
        let version = RowVersion {
            tx_id_created: header.tx_id_created,
            tx_id_deleted: header.tx_id_deleted,
            undo_ptr: header.undo_ptr,
            is_current: true,
        };
        
        if snapshot.is_visible(&version) {
            return Ok(Some(VersionedRow {
                header,
                data: slot[RowMVCCHeader::SIZE..].to_vec(),
                is_visible: true,
            }));
        }
        
        // 4. 当前版本不可见，沿 Undo 链回溯
        let mut undo_ptr = header.undo_ptr;
        while !undo_ptr.is_null() {
            let undo_record = self.undo_manager.get(&undo_ptr)?;
            
            // 检查这个 Undo 版本是否可见
            let undo_version = RowVersion {
                tx_id_created: undo_record.header.tx_id,
                tx_id_deleted: 0, // Undo 记录本身不是删除
                undo_ptr: undo_record.header.prev_undo_ptr,
                is_current: false,
            };
            
            if snapshot.is_visible(&undo_version) {
                // 找到了可见的历史版本
                return Ok(Some(VersionedRow {
                    header: RowMVCCHeader::new(
                        undo_record.header.tx_id,
                        UndoPtr::null(),
                        undo_record.before_image.len() as u16,
                    ),
                    data: undo_record.before_image,
                    is_visible: true,
                }));
            }
            
            undo_ptr = undo_record.header.prev_undo_ptr;
        }
        
        // 没有找到可见版本（可能被删除或未提交）
        Ok(None)
    }
}

/// 带版本信息的行
pub struct VersionedRow {
    pub header: RowMVCCHeader,
    pub data: Vec<u8>,
    pub is_visible: bool,
}
```

### 4.2 索引读取流程

```rust
impl MVCC {
    /// 通过索引读取（只返回可见行）
    pub fn index_read(
        &self,
        table_id: TableId,
        index_id: IndexId,
        key: &[u8],
        snapshot: &ReadSnapshot,
    ) -> Result<Option<Tuple>> {
        // 1. 从索引获取行指针列表
        let row_ptrs = self.index_manager.search(index_id, key)?;
        
        // 2. 逐个检查可见性
        for ptr in row_ptrs {
            if let Some(row) = self.get_visible_version(table_id, &ptr.row_id, snapshot)? {
                if row.is_visible {
                    return Ok(Some(self.decode_row(&row)?));
                }
            }
        }
        
        Ok(None)
    }
}
```

---

## 5. 写流程（Copy-on-Write + Undo）

### 5.1 Insert 流程

```rust
impl MVCC {
    /// 插入行（带 MVCC）
    pub fn insert(
        &self,
        tx: &mut Transaction,
        table_id: TableId,
        values: Vec<Value>,
    ) -> Result<RowId> {
        // 1. 编码行数据
        let row_data = self.encode_row(table_id, &values)?;
        
        // 2. 分配行ID（page_id + slot_idx）
        let row_id = self.heap_manager.allocate_row(table_id)?;
        
        // 3. 创建 Undo 记录（用于回滚）
        // Insert 的 Undo 只需要记录 row_id，回滚时删除即可
        let undo_record = UndoRecord {
            header: UndoRecordHeader {
                length: std::mem::size_of::<UndoRecordHeader>() as u32,
                tx_id: tx.tx_id,
                table_id,
                row_id,
                prev_undo_ptr: tx.last_undo_ptr,
                undo_type: UndoType::Insert,
                checksum: 0,
            },
            before_image: Vec::new(), // Insert 无 before_image
        };
        
        // 4. 写入 WAL（Redo + Undo）
        let wal_lsn = self.wal.write_log(&LogRecord {
            tx_id: tx.tx_id,
            op: LogOp::Insert,
            table_id,
            row_id,
            undo_record: Some(undo_record.clone()),
            before_image: None,
            after_image: Some(row_data.clone()),
        })?;
        
        // 5. 更新 Undo 指针
        let undo_ptr = self.undo_manager.append(&undo_record)?;
        tx.last_undo_ptr = undo_ptr;
        
        // 6. 构建行头
        let header = RowMVCCHeader {
            tx_id_created: tx.tx_id,
            tx_id_deleted: 0,
            undo_ptr,
            row_length: row_data.len() as u16,
        };
        
        // 7. 写入页面
        let page = self.buffer_mgr.get_page_mut(row_id.page_id)?;
        page.write_slot(row_id.slot_idx, header, &row_data)?;
        
        // 8. 标记页面为脏
        self.buffer_mgr.mark_dirty(row_id.page_id, wal_lsn)?;
        
        Ok(row_id)
    }
}
```

### 5.2 Update 流程

```rust
impl MVCC {
    /// 更新行（带 MVCC）
    pub fn update(
        &self,
        tx: &mut Transaction,
        table_id: TableId,
        row_id: &RowId,
        new_values: Vec<Value>,
    ) -> Result<()> {
        // 1. 获取行（验证存在且可见）
        let page = self.buffer_mgr.get_page(row_id.page_id)?;
        let slot = page.get_slot(row_id.slot_idx)?;
        let old_header = RowMVCCHeader::from_bytes(slot)?;
        let old_data = slot[RowMVCCHeader::SIZE..].to_vec();
        
        // 2. 检查是否有其他事务持有排他锁
        self.lock_manager.lock_row(tx.tx_id, row_id.clone(), LockMode::Exclusive)?;
        
        // 3. 检查当前版本是否对自己可见（防止更新已删除的行）
        let old_version = RowVersion {
            tx_id_created: old_header.tx_id_created,
            tx_id_deleted: old_header.tx_id_deleted,
            undo_ptr: old_header.undo_ptr,
            is_current: true,
        };
        
        let snapshot = tx.get_snapshot()?;
        if !snapshot.is_visible(&old_version) {
            return Err(MVCCError::RowNotVisible);
        }
        
        // 4. 编码新行数据
        let new_data = self.encode_row_with_values(table_id, &old_data, new_values)?;
        
        // 5. 创建 Undo 记录（记录修改前的完整数据）
        let undo_record = UndoRecord {
            header: UndoRecordHeader {
                length: std::mem::size_of::<UndoRecordHeader>() as u32 + old_data.len() as u32,
                tx_id: tx.tx_id,
                table_id,
                row_id: row_id.clone(),
                prev_undo_ptr: tx.last_undo_ptr,
                undo_type: UndoType::Update,
                checksum: 0,
            },
            before_image: old_data.clone(),
        };
        
        // 6. 写入 WAL
        let wal_lsn = self.wal.write_log(&LogRecord {
            tx_id: tx.tx_id,
            op: LogOp::Update,
            table_id,
            row_id: row_id.clone(),
            undo_record: Some(undo_record.clone()),
            before_image: Some(old_data),
            after_image: Some(new_data.clone()),
        })?;
        
        // 7. 更新 Undo 链
        let undo_ptr = self.undo_manager.append(&undo_record)?;
        tx.last_undo_ptr = undo_ptr;
        
        // 8. 更新行头（指向新版本）
        // 注意：物理上覆盖原位置，这是 Copy-on-Write 的简化版本
        let new_header = RowMVCCHeader {
            tx_id_created: tx.tx_id,
            tx_id_deleted: 0,
            undo_ptr,
            row_length: new_data.len() as u16,
        };
        
        // 9. 写入页面
        let page = self.buffer_mgr.get_page_mut(row_id.page_id)?;
        page.write_slot(row_id.slot_idx, new_header, &new_data)?;
        
        // 10. 标记脏页
        self.buffer_mgr.mark_dirty(row_id.page_id, wal_lsn)?;
        
        Ok(())
    }
}
```

### 5.3 Delete 流程

```rust
impl MVCC {
    /// 删除行（逻辑删除）
    pub fn delete(
        &self,
        tx: &mut Transaction,
        table_id: TableId,
        row_id: &RowId,
    ) -> Result<()> {
        // 1. 获取行
        let page = self.buffer_mgr.get_page(row_id.page_id)?;
        let slot = page.get_slot(row_id.slot_idx)?;
        let header = RowMVCCHeader::from_bytes(slot)?;
        
        // 2. 获取排他锁
        self.lock_manager.lock_row(tx.tx_id, row_id.clone(), LockMode::Exclusive)?;
        
        // 3. 检查可见性
        let version = RowVersion {
            tx_id_created: header.tx_id_created,
            tx_id_deleted: header.tx_id_deleted,
            undo_ptr: header.undo_ptr,
            is_current: true,
        };
        
        let snapshot = tx.get_snapshot()?;
        if !snapshot.is_visible(&version) {
            return Err(MVCCError::RowNotVisible);
        }
        
        // 4. 已删除？
        if header.tx_id_deleted != 0 {
            return Err(MVCCError::RowAlreadyDeleted);
        }
        
        // 5. 创建 Undo 记录
        let old_data = slot[RowMVCCHeader::SIZE..].to_vec();
        let undo_record = UndoRecord {
            header: UndoRecordHeader {
                length: std::mem::size_of::<UndoRecordHeader>() as u32 + old_data.len() as u32,
                tx_id: tx.tx_id,
                table_id,
                row_id: row_id.clone(),
                prev_undo_ptr: tx.last_undo_ptr,
                undo_type: UndoType::Delete,
                checksum: 0,
            },
            before_image: old_data,
        };
        
        // 6. 写入 WAL
        let wal_lsn = self.wal.write_log(&LogRecord {
            tx_id: tx.tx_id,
            op: LogOp::Delete,
            table_id,
            row_id: row_id.clone(),
            undo_record: Some(undo_record.clone()),
            before_image: None,
            after_image: None,
        })?;
        
        // 7. 更新 Undo 链
        let undo_ptr = self.undo_manager.append(&undo_record)?;
        tx.last_undo_ptr = undo_ptr;
        
        // 8. 标记删除（更新行头）
        // 注意：只标记 tx_id_deleted，不实际删除数据
        let mut header_mut = *header;
        header_mut.tx_id_deleted = tx.tx_id;
        
        let page = self.buffer_mgr.get_page_mut(row_id.page_id)?;
        page.update_slot_header(row_id.slot_idx, header_mut)?;
        
        // 9. 标记脏页
        self.buffer_mgr.mark_dirty(row_id.page_id, wal_lsn)?;
        
        Ok(())
    }
}
```

---

## 6. 事务回滚

### 6.1 回滚流程

```rust
impl MVCC {
    /// 回滚事务
    pub fn rollback(&self, tx: &mut Transaction) -> Result<()> {
        // 1. 沿 Undo 链回溯
        let mut undo_ptr = tx.last_undo_ptr;
        
        while !undo_ptr.is_null() {
            // 读取 Undo 记录
            let undo_record = self.undo_manager.get(&undo_ptr)?;
            
            // 只处理当前事务的记录
            if undo_record.header.tx_id != tx.tx_id {
                break;
            }
            
            // 根据 Undo 类型回滚
            match undo_record.header.undo_type {
                UndoType::Insert => {
                    // 回滚插入：删除插入的元组
                    self.rollback_insert(&undo_record.header.row_id)?;
                }
                UndoType::Update => {
                    // 回滚更新：用 before_image 覆盖
                    self.rollback_update(
                        &undo_record.header.row_id,
                        &undo_record.before_image,
                    )?;
                }
                UndoType::Delete => {
                    // 回滚删除：清除删除标记
                    self.rollback_delete(&undo_record.header.row_id)?;
                }
            }
            
            // 继续处理更早的 Undo
            undo_ptr = undo_record.header.prev_undo_ptr;
        }
        
        // 2. 释放锁
        self.lock_manager.rollback(tx.tx_id)?;
        
        // 3. 更新事务状态
        self.tx_state_manager.tx_abort(tx.tx_id)?;
        
        // 4. 记录 Abort 日志
        self.wal.write_abort(tx.tx_id)?;
        
        Ok(())
    }

    fn rollback_insert(&self, row_id: &RowId) -> Result<()> {
        // 标记为已删除（由其他事务创建，所以创建者ID保留）
        let page = self.buffer_mgr.get_page_mut(row_id.page_id)?;
        let mut header = page.get_slot_header(row_id.slot_idx)?;
        header.tx_id_deleted = 1; // 标记为已删除
        page.update_slot_header(row_id.slot_idx, header)?;
        
        self.buffer_mgr.mark_dirty(row_id.page_id, 0)?;
        Ok(())
    }

    fn rollback_update(&self, row_id: &RowId, before_image: &[u8]) -> Result<()> {
        // 用 before_image 覆盖当前数据
        let page = self.buffer_mgr.get_page_mut(row_id.page_id)?;
        page.write_slot_data(row_id.slot_idx, before_image)?;
        
        self.buffer_mgr.mark_dirty(row_id.page_id, 0)?;
        Ok(())
    }

    fn rollback_delete(&self, row_id: &RowId) -> Result<()> {
        // 清除删除标记
        let page = self.buffer_mgr.get_page_mut(row_id.page_id)?;
        let mut header = page.get_slot_header(row_id.slot_idx)?;
        header.tx_id_deleted = 0;
        page.update_slot_header(row_id.slot_idx, header)?;
        
        self.buffer_mgr.mark_dirty(row_id.page_id, 0)?;
        Ok(())
    }
}
```

---

## 7. GC 机制

### 7.1 后台 GC

```rust
/// GC Worker
pub struct GCWorker {
    /// 唤醒间隔
    interval: Duration,
    /// 最老活跃事务的最小 tx_id
    oldest_active_tx: TransactionId,
    /// 是否运行
    running: AtomicBool,
}

impl GCWorker {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            oldest_active_tx: 0,
            running: AtomicBool::new(false),
        }
    }

    pub fn start(&self, undo_manager: Arc<UndoManager>, tx_state: Arc<TxStateManager>) {
        self.running.store(true, Ordering::SeqCst);
        
        std::thread::spawn(move || {
            while self.running.load(Ordering::SeqCst) {
                // 1. 获取最老活跃事务ID
                let active = tx_state.get_active_txns();
                let oldest = active.iter().min().copied().unwrap_or(u64::MAX);
                
                // 2. 清理已提交事务的 Undo 记录
                if let Err(e) = undo_manager.purge_completed_tx(oldest) {
                    error!("GC purge failed: {}", e);
                }
                
                // 3. 清理不可见的物理版本（如果有独立的版本存储）
                // self.purge_invisible_versions(oldest)?;
                
                std::thread::sleep(self.interval);
            }
        });
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}
```

### 7.2 手动 GC（VACUUM）

```rust
impl MVCC {
    /// 手动 GC
    pub fn vacuum(&self, table_id: Option<TableId>) -> Result<GCStats> {
        let mut stats = GCStats::default();
        
        // 1. 获取最老活跃事务
        let oldest_active = self.tx_state_manager.get_active_txns()
            .iter()
            .min()
            .copied()
            .unwrap_or(u64::MAX);
        
        // 2. 清理 Undo
        stats.undo_purged = self.undo_manager.purge_committed(oldest_active)?;
        
        // 3. 如果有独立版本存储，清理物理版本
        if let Some(tid) = table_id {
            stats.rows_purged = self.purge_invisible_rows(tid, oldest_active)?;
        }
        
        Ok(stats)
    }
}

#[derive(Debug, Default)]
pub struct GCStats {
    pub undo_purged: usize,
    pub rows_purged: usize,
}
```

---

## 8. 与现有模块集成

### 8.1 Transaction 扩展

```rust
/// 扩展 Transaction 结构
pub struct Transaction {
    pub tx_id: TransactionId,
    pub status: TxStatus,
    pub snapshot: Option<ReadSnapshot>,
    pub last_undo_ptr: UndoPtr,
    pub start_time: Instant,
    // ... 现有的锁信息
}

impl Transaction {
    pub fn get_snapshot(&mut self) -> Result<&ReadSnapshot> {
        if self.snapshot.is_none() {
            // RC: 每次读取创建新快照
            self.snapshot = Some(self.create_snapshot()?);
        }
        Ok(self.snapshot.as_ref().unwrap())
    }

    fn create_snapshot(&self) -> Result<ReadSnapshot> {
        let max_committed = self.tx_state_manager.get_max_committed_tx();
        let active = self.tx_state_manager.get_active_txns();
        
        Ok(ReadSnapshot {
            tx_id: self.tx_id,
            snapshot_lsn: 0, // TODO: 获取当前 LSN
            max_committed_tx: max_committed,
            active_txns: active,
            isolation: IsolationLevel::ReadCommitted,
            created_at: Instant::now(),
        })
    }

    /// 刷新快照（RC 用）
    pub fn refresh_snapshot(&mut self) -> Result<()> {
        self.snapshot = Some(self.create_snapshot()?);
        Ok(())
    }
}
```

### 8.2 StorageEngine 接口

```rust
impl StorageEngine {
    /// 开始事务
    pub fn begin_transaction(&mut self) -> TransactionId {
        let tx_id = self.tx_manager.begin();
        // 记录开始 LSN
        let start_lsn = self.wal.get_current_lsn();
        self.tx_state_manager.tx_start(tx_id, start_lsn);
        tx_id
    }

    /// 提交事务
    pub fn commit(&mut self, tx_id: TransactionId) -> Result<()> {
        // 1. 释放锁
        self.lock_manager.commit(tx_id)?;
        
        // 2. 更新事务状态
        let commit_lsn = self.wal.get_current_lsn();
        self.tx_state_manager.tx_commit(tx_id, commit_lsn)?;
        
        // 3. 写提交日志
        self.wal.write_commit(tx_id, commit_lsn)?;
        
        // 4. 刷新日志（确保持久化）
        self.wal.flush()?;
        
        Ok(())
    }

    /// 回滚事务
    pub fn rollback(&mut self, tx_id: TransactionId) -> Result<()> {
        // 1. MVCC 回滚
        self.mvcc.rollback(tx_id)?;
        
        // 2. 锁回滚
        self.lock_manager.rollback(tx_id)?;
        
        // 3. 状态更新
        self.tx_state_manager.tx_abort(tx_id)?;
        
        // 4. 写 Abort 日志
        self.wal.write_abort(tx_id)?;
        
        Ok(())
    }
}
```

---

## 9. 页面格式扩展

### 9.1 数据页格式

```
┌─────────────────────────────────────────────────────────────┐
│                      Page Header (24 bytes)                 │
├─────────────────────────────────────────────────────────────┤
│  Slot Directory                                             │
│  ┌──────────┬──────────┬──────────┬──────────┐            │
│  │ Slot 0   │ Slot 1   │  ...     │ Slot N   │            │
│  │ offset   │ offset   │          │ offset   │            │
│  │ length   │ length   │          │ length   │            │
│  └──────────┴──────────┴──────────┴──────────┘            │
├─────────────────────────────────────────────────────────────┤
│  Row Data Area                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ Row 0: [MVCC Header(34B)] [Row Data...]              │  │
│  ├──────────────────────────────────────────────────────┤  │
│  │ Row 1: [MVCC Header(34B)] [Row Data...]              │  │
│  ├──────────────────────────────────────────────────────┤  │
│  │ ...                                                   │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## 10. 实现步骤

| 步骤 | 内容 | 预计工作量 |
|------|------|-----------|
| 1 | 扩展 types.rs 添加 MVCC 相关类型 | 0.5天 |
| 2 | 创建 undo_tablespace 模块 | 2天 |
| 3 | 实现 UndoManager（写入/读取） | 2天 |
| 4 | 实现 TxStateManager（事务状态表） | 1天 |
| 5 | 实现 ReadSnapshot 和可见性判断 | 2天 |
| 6 | 扩展 StorageEngine 接口（begin/commit/rollback） | 1天 |
| 7 | 实现 Insert + Undo | 1.5天 |
| 8 | 实现 Update + Undo | 1.5天 |
| 9 | 实现 Delete + Undo | 1天 |
| 10 | 实现事务回滚（沿 Undo 链） | 1.5天 |
| 11 | 实现 GC Worker（后台自动） | 1.5天 |
| 12 | 实现手动 vacuum 命令 | 1天 |
| 13 | 集成测试（RC 隔离级别） | 3天 |

**小计: ~19 天**

---

## 11. 索引与 MVCC 集成（已确认）

### 11.1 设计原则

- **索引始终指向最新版本**：索引叶子节点存储当前最新的行位置
- **LRU 不感知 MVCC**：BufferPool 正常运作，读取/写入的都是最新页面
- **MVCC 判断在索引和 Heap 模块进行**：索引扫描和 Heap 读取时进行可见性判断

### 11.2 索引叶子节点扩展

```rust
/// 索引叶子节点条目（扩展）
pub struct IndexEntry {
    /// 索引键值
    pub key: Vec<u8>,
    /// 行物理位置
    pub row_ptr: RowPtr,
    /// 指向 Undo 记录的指针（用于索引列更新时的回滚）
    pub undo_ptr: UndoPtr,
    /// 该索引条目的 tx_id（用于可见性判断）
    pub tx_id_created: TransactionId,
}

impl IndexEntry {
    pub fn new(key: Vec<u8>, row_ptr: RowPtr, tx_id: TransactionId) -> Self {
        Self {
            key,
            row_ptr,
            undo_ptr: UndoPtr::null(),
            tx_id_created: tx_id,
        }
    }
}
```

### 11.3 索引更新策略

#### Insert（插入）
```rust
impl MVCC {
    pub fn insert_with_index(
        &self,
        tx: &mut Transaction,
        table_id: TableId,
        values: Vec<Value>,
    ) -> Result<RowId> {
        // 1. 插入数据行（已有逻辑）
        let row_id = self.insert(tx, table_id, values.clone())?;
        
        // 2. 插入索引
        for (index_id, indexed_value) in self.get_indexed_values(table_id, &values)? {
            let entry = IndexEntry::new(
                indexed_value.encode()?,
                RowPtr::from(row_id),
                tx.tx_id,
            );
            self.index_manager.insert(index_id, entry)?;
        }
        
        Ok(row_id)
    }
}
```

#### Update（更新）- 索引列变化
```rust
impl MVCC {
    pub fn update_with_index(
        &self,
        tx: &mut Transaction,
        table_id: TableId,
        row_id: &RowId,
        new_values: Vec<Value>,
    ) -> Result<()> {
        // 1. 读取旧行数据
        let old_row = self.heap_manager.get_row(row_id)?;
        
        // 2. 更新数据行（已有逻辑）
        self.update(tx, table_id, row_id, new_values.clone())?;
        
        // 3. 处理索引变化
        let old_indexed = self.get_indexed_values_from_row(table_id, &old_row)?;
        let new_indexed = self.get_indexed_values(table_id, &new_values)?;
        
        // 4. 对比并更新索引
        for (index_id, (old_val, new_val)) in old_indexed.iter().zip(new_indexed.iter()).enumerate() {
            if old_val != new_val {
                // 索引值变化：删除旧索引，插入新索引
                let index_id = self.get_index_id_for_column(table_id, index_id)?;
                
                // 删除旧索引（记录 Undo）
                let old_entry = IndexEntry::new(old_val.encode()?, RowPtr::from(row_id), old_row.header.tx_id_created);
                self.index_manager.delete(index_id, &old_entry)?;
                
                // 插入新索引
                let new_entry = IndexEntry::new(new_val.encode()?, RowPtr::from(row_id), tx.tx_id);
                self.index_manager.insert(index_id, new_entry)?;
            }
        }
        
        Ok(())
    }
}
```

#### Delete（删除）
```rust
impl MVCC {
    pub fn delete_with_index(
        &self,
        tx: &mut Transaction,
        table_id: TableId,
        row_id: &RowId,
    ) -> Result<()> {
        // 1. 读取行获取索引值
        let row = self.heap_manager.get_row(row_id)?;
        let indexed_values = self.get_indexed_values_from_row(table_id, &row)?;
        
        // 2. 删除数据行（标记删除，已有逻辑）
        self.delete(tx, table_id, row_id)?;
        
        // 3. 删除索引
        for (index_id, indexed_value) in indexed_values.into_iter().enumerate() {
            let entry = IndexEntry::new(
                indexed_value.encode()?,
                RowPtr::from(row_id),
                row.header.tx_id_created,
            );
            self.index_manager.delete(index_id, &entry)?;
        }
        
        Ok(())
    }
}
```

### 11.4 索引读取流程

```rust
impl IndexManager {
    /// 索引扫描（返回候选行，由 MVCC 过滤可见性）
    pub fn scan(
        &self,
        index_id: IndexId,
        key_range: KeyRange,
    ) -> Result<Vec<IndexEntry>> {
        // 1. B-tree 扫描获取候选索引条目
        let entries = self.btree_scan(index_id, key_range)?;
        
        // 2. 返回所有条目，由上层 MVCC 判断可见性
        Ok(entries)
    }
}

impl MVCC {
    /// 索引扫描 + 可见性过滤
    pub fn index_scan(
        &self,
        table_id: TableId,
        index_id: IndexId,
        key_range: KeyRange,
        snapshot: &ReadSnapshot,
    ) -> Result<Vec<Tuple>> {
        // 1. 索引扫描获取候选
        let candidates = self.index_manager.scan(index_id, key_range)?;
        
        // 2. 逐个检查可见性
        let mut results = Vec::new();
        for entry in candidates {
            // 读取行数据
            if let Some(row) = self.get_visible_version(table_id, &entry.row_ptr.row_id, snapshot)? {
                if row.is_visible {
                    results.push(self.decode_row(&row)?);
                }
            }
        }
        
        Ok(results)
    }
}
```

### 11.5 索引回滚

```rust
impl MVCC {
    /// 回滚时恢复索引
    fn rollback_index(&self, undo_type: UndoType, row_id: &RowId, before_image: &[u8]) -> Result<()> {
        match undo_type {
            UndoType::Insert => {
                // 回滚插入：删除插入时的索引
                // 从 Undo 记录中获取插入时的索引值
                todo!()
            }
            UndoType::Update => {
                // 回滚更新：恢复旧索引值
                // 1. 删除新索引
                // 2. 恢复旧索引
                todo!()
            }
            UndoType::Delete => {
                // 回滚删除：恢复被删除的索引
                // 从 Undo 记录中获取删除前的索引值并恢复
                todo!()
            }
        }
    }
}
```

### 11.6 LRU-K 与 MVCC 关系

```
┌─────────────────────────────────────────────────────────────┐
│                        BufferPool                          │
│  ┌─────────────────────────────────────────────────────┐  │
│  │  LRU-K: 正常淘汰冷页面                            │  │
│  │  - 不感知 MVCC                                    │  │
│  │  - 读取/写入的都是最新页面                        │  │
│  └─────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                    MVCC Layer                             │
│  - 索引扫描: 返回候选 → MVCC 可见性判断 → 返回结果       │
│  - Heap 读取: 读取页面 → MVCC 可见性判断 → 返回结果      │
│  - 页面换出: LRU-K 正常换出，无需特殊处理                │
│  - 页面加载: 加载后由 MVCC 判断可见性                    │
└─────────────────────────────────────────────────────────────┘
```

**结论**：
- BufferPool/LRU-K 保持原有逻辑不变
- MVCC 可见性判断在索引读取和 Heap 读取时进行
- 回滚时由 MVCC 层处理索引恢复

---

## 12. 待讨论问题

以上设计已确认，开始实现前确认以下问题：

1. **WAL 和 Undo 的关系** ✅ 已确认：同时写 WAL 和 Undo Tablespace

2. **索引更新策略** ✅ 已确认：
   - 索引始终指向最新版本
   - 删除/更新索引列时索引要变化
   - 叶子记录 Undo 指针

3. **LRU-K 集成** ✅ 已确认：
   - LRU 不感知 MVCC
   - 读取写入的都是最新页面
   - 在索引和 Heap 模块进行 MVCC 判断和回滚

4. **恢复流程**（待实现时确认）：
   - 崩溃恢复时，如何恢复 Undo 链？
   - 需要和 WAL Recovery 集成

确认后开始实现？
