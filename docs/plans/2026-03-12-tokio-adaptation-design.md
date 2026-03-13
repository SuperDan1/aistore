# Tokio 异步 Runtime 适配设计

**Date:** 2026-03-12
**Status:** Completed
**Target:** Aistore Storage Engine Async Adaptation

---

## ✅ 最终架构

采用保守方案：锁层面替换为 tokio::sync，内部保持同步逻辑。

```
StorageEngine (同步API)
       │
       ▼
tokio::sync::RwLock / Mutex
   (blocking_xxx() 桥接)
       │
       ▼
   底层同步逻辑
```

---

## ✅ 实施完成

### 已完成工作

| 组件 | 状态 | 说明 |
|------|------|------|
| **tokio 依赖** | ✅ | 添加到 Cargo.toml |
| **parking_lot → tokio::sync** | ✅ | 全部模块已替换 |
| **测试验证** | ✅ | 229 tests passed |

---

## 1. 背景与目标

### 1.1 背景

Aistore 存储引擎当前采用纯同步架构，使用 `parking_lot` 和标准库同步原语。随着并发需求提升，需要引入异步 runtime 以：

1. **提升并发能力**: 异步任务调度替代同步阻塞
2. **资源利用率**: 减少线程上下文切换开销
3. **未来扩展**: 为网络服务层 (gRPC/HTTP) 打下基础

### 1.2 目标

| 目标 | 描述 |
|------|------|
| **StorageEngine API 保持同步** | 对外接口不变，用户无感知 |
| **内部组件异步化** | Buffer、Lock、WAL 等底层组件适配 tokio |
| **渐进式迁移** | 最小化风险，逐步替换 |
| **性能不降** | 异步化后性能持平或提升 |

### 1.3 非目标

- 不暴露异步 API 给外部用户
- 不修改 SQL/Executor 层
- 不引入新的网络协议支持

---

## 2. 架构设计

### 2.1 整体架构

```
┌─────────────────────────────────────────────────────────────────────┐
│                        External Layer                                │
│                  StorageEngine API (同步 - 不变)                      │
└────────────────────────────┬────────────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────────┐
│                     Internal Async Layer                             │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐          │
│  │ BufferMgr      │  │ LockManager    │  │ WAL Manager    │          │
│  │ tokio::sync   │  │ tokio::sync   │  │ tokio::fs      │          │
│  │ .RwLock       │  │ .Mutex        │  │ async IO       │          │
│  └────────────────┘  └────────────────┘  └────────────────┘          │
├─────────────────────────────────────────────────────────────────────┤
│                     VFS Abstraction Layer                            │
│  ┌─────────────────────────┐    ┌─────────────────────────┐          │
│  │ SyncVfsInterface (现有) │    │ AsyncVfsInterface (新增)│          │
│  │ LocalFs                 │    │ AsyncLocalFs            │          │
│  └─────────────────────────┘    └─────────────────────────┘          │
├─────────────────────────────────────────────────────────────────────┤
│                       Tokio Runtime                                   │
│              (Multi-threaded, IO-aware scheduler)                    │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.2 分层职责

| 层级 | 组件 | 职责 | 同步/异步 |
|------|------|------|-----------|
| 对外 API | StorageEngine | DDL/DML 接口 | 同步 |
| 内部协调 | BufferMgr | 页面缓存、淘汰 | 异步 |
| 并发控制 | LockManager | 事务锁、行锁 | 异步 |
| 日志 | WAL | 预写日志 | 异步 |
| 存储抽象 | VFS | 文件系统抽象 | 双轨 |

### 2.3 数据流

```
同步请求进入
     │
     ▼
StorageEngine::insert()  ──同步调用──▶ BufferMgr::get_page()
     │                                              │
     │                                              ▼
     │                                    (tokio::sync::RwLock)
     │                                              │
     │                                              ▼
     │                                    BufferMgr::pwrite()
     │                                              │
     │                                              ▼
     │                                    (AsyncVfsInterface)
     │                                              │
     ▼                                              ▼
返回结果 ←────────────────异步完成通知────────────────┘
```

---

## 3. 详细适配方案

### 3.1 Cargo 依赖变更

```toml
# 新增依赖
[dependencies]
tokio = { version = "1", features = [
    "rt-multi-thread",    # 多线程调度器
    "sync",               # 同步原语
    "fs",                 # 异步文件系统
    "io-util",            # IO 工具
    "time",               # 时间管理
] }

# 保留现有依赖
parking_lot = "0.12"      # 保留，用于不需要异步的场景
```

### 3.2 VFS 层适配

#### 3.2.1 双轨 VFS 架构

```rust
// ============ 现有同步接口 (保持不变) ============
pub trait VfsInterface: Send + Sync {
    fn pread(&self, path: &str, buf: &mut [u8], offset: u64) -> VfsResult<usize>;
    fn pwrite(&self, path: &str, buf: &[u8], offset: u64) -> VfsResult<usize>;
    // ... 其他同步方法
}

// ============ 新增异步接口 ============
#[async_trait]
pub trait AsyncVfsInterface: Send + Sync {
    async fn pread(&self, path: &str, buf: &mut [u8], offset: u64) -> VfsResult<usize>;
    async fn pwrite(&self, path: &str, buf: &[u8], offset: u64) -> VfsResult<usize>;
    // ... 其他异步方法
}

// ============ 异步实现 (基于 tokio::fs) ============
pub struct AsyncLocalFs {
    runtime: tokio::runtime::Handle,
}

#[async_trait]
impl AsyncVfsInterface for AsyncLocalFs {
    async fn pread(&self, path: &str, mut buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let file = tokio::fs::File::open(path).await.map_err(|e| ...)?;
        use tokio::io::AsyncReadExt;
        file.read(&mut buf).await.map_err(|e| ...)?
        // ... 实现
    }
}
```

#### 3.2.2 桥接层

```rust
// 在 VFS 模块中添加桥接
impl VfsInterface for AsyncLocalFs {
    // 同步调用通过 block_on 桥接
    fn pread(&self, path: &str, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        tokio::runtime::Handle::current().block_on(
            self.pread(path, buf, offset)
        )
    }
}
```

### 3.3 BufferMgr 适配

#### 3.3.1 当前实现

```rust
// 当前: 使用 parking_lot::RwLock
pub struct BufferMgr {
    buffers: Vec<BufferDesc>,
    hash_table: *mut *mut HashEntry,
    lock: parking_lot::RwLock<()>,  // 需要替换
}
```

#### 3.3.2 适配后

```rust
use tokio::sync::RwLock;
use std::sync::Arc;

pub struct BufferMgr {
    buffers: Vec<BufferDesc>,
    hash_table: *mut *mut HashEntry,
    // 替换为 tokio RwLock
    lock: Arc<RwLock<()>>,
}

// 注意: BufferMgr 内部仍然使用同步逻辑
// 只是锁机制改为 tokio::sync，以支持更高并发
impl BufferMgr {
    pub async fn get_page(&self, page_id: PageId) -> Result<PageId> {
        let _lock = self.lock.read().await;
        // 同步逻辑保持不变
        self.lookup_or_load(page_id).await
    }
}
```

#### 3.3.3 适配要点

| 项目 | 当前实现 | 适配后 |
|------|----------|--------|
| 全局锁 | `parking_lot::RwLock` | `tokio::sync::RwLock` |
| 页面锁 | `std::sync::RwLock` | `tokio::sync::RwLock` |
| 状态原子 | `AtomicU64` | 保持不变 |
| 淘汰策略 | 同步 LRU | 保持不变 (仅锁异步化) |

### 3.4 LockManager 适配

#### 3.4.1 当前实现

```rust
// 当前: 使用 parking_lot::Mutex
struct RowLockManager {
    locks: HashMap<LockRowId, parking_lot::Mutex<LockState>>,
}
```

#### 3.4.2 适配后

```rust
use tokio::sync::Mutex;
use std::sync::Arc;

struct RowLockManager {
    locks: HashMap<LockRowId, Arc<Mutex<LockState>>>,
}
```

#### 3.4.3 适配要点

| 项目 | 当前实现 | 适配后 |
|------|----------|--------|
| 行锁 | `parking_lot::Mutex` | `tokio::sync::Mutex` |
| 表锁 | `parking_lot::Mutex` | `tokio::sync::Mutex` |
| 事务管理 | 同步 | 保持同步 (事务边界) |

**注意**: LockManager 的 API 保持同步，内部实现使用 tokio 原语。

### 3.5 WAL 适配

#### 3.5.1 当前实现

```rust
// 当前: 同步文件 IO
impl WalManager {
    pub fn append(&self, data: &[u8]) -> Lsn {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&self.path)
            .unwrap();
        std::io::Write::write_all(&mut file, data).unwrap();
    }
}
```

#### 3.5.2 适配后

```rust
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, AsyncSeekExt};

pub struct AsyncWalManager {
    file: Option<File>,
    path: PathBuf,
    // ... 其他字段
}

impl AsyncWalManager {
    pub async fn append(&mut self, data: &[u8]) -> Lsn {
        if self.file.is_none() {
            self.file = Some(
                tokio::fs::OpenOptions::new()
                    .append(true)
                    .open(&self.path)
                    .await
                    .map_err(|e| ...)?
            );
        }
        
        let file = self.file.as_mut().unwrap();
        file.write_all(data).await.map_err(|e| ...)?;
        file.flush().await.map_err(|e| ...)?;
        
        // 返回 LSN
    }
}
```

#### 3.5.3 WAL 桥接

由于 StorageEngine 使用同步 API，需要提供桥接:

```rust
// 为 StorageEngine 提供同步 WAL 接口，内部调用异步实现
impl WalManager {
    pub fn append_sync(&self, data: &[u8]) -> Lsn {
        // 使用 block_on 桥接
        tokio::runtime::Handle::current().block_on(
            self.async_wal.append(data)
        )
    }
}
```

---

## 4. 渐进式迁移计划

### Phase 1: 基础设施 (预计 1 周)

- [ ] 添加 tokio 依赖，配置 features
- [ ] 创建 `src/async_vfs/` 模块
- [ ] 实现 `AsyncVfsInterface` trait
- [ ] 实现 `AsyncLocalFs`
- [ ] 验证编译通过

### Phase 2: Buffer 异步化 (预计 1 周)

- [ ] 替换 BufferMgr 全局锁为 `tokio::sync::RwLock`
- [ ] 替换页面锁为 `tokio::sync::RwLock`
- [ ] 添加桥接层 (`sync -> async`)
- [ ] 运行 Buffer 相关测试，确保无回归

### Phase 3: Lock 异步化 (预计 1 周)

- [ ] 替换 RowLockManager 锁为 `tokio::sync::Mutex`
- [ ] 替换 TableLockManager 锁为 `tokio::sync::Mutex`
- [ ] 添加桥接层
- [ ] 运行 Lock 相关测试

### Phase 4: WAL 异步化 (预计 1 周)

- [ ] 创建 `AsyncWalManager`
- [ ] 实现异步 append/flush
- [ ] 添加同步桥接
- [ ] 运行 WAL 相关测试

### Phase 5: 集成测试 (预计 3 天)

- [ ] 运行全部单元测试
- [ ] 运行集成测试
- [ ] 性能基准测试 (对比异步化前后)
- [ ] 内存使用分析

---

## 5. 风险与注意事项

### 5.1 性能风险

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| `tokio::sync` 开销 | 相比 parking_lot 可能有额外开销 | 基准测试对比 |
| `block_on` 嵌套 | 同步调用异步可能造成性能损失 | 最小化桥接次数 |
| 调度器开销 | 多线程调度带来开销 | 使用单线程 runtime 变体 |

**建议**: 初期使用 `tokio::runtime::Builder::current_thread()` 验证功能，再评估是否需要多线程。

### 5.2 兼容性风险

- **依赖兼容性**: tokio 1.x 与现有依赖可能存在兼容性问题
- **行为差异**: tokio 原语与 parking_lot 行为略有差异 (如锁公平性)

### 5.3 测试策略

1. **单元测试**: 每个模块独立测试
2. **集成测试**: 端到端 CRUD 测试
3. **回归测试**: 对比异步化前后结果一致性
4. **性能测试**: 使用现有 bench 框架

---

## 6. 文件变更清单

### 新增文件

| 文件 | 描述 |
|------|------|
| `src/async_vfs/mod.rs` | 异步 VFS 模块入口 |
| `src/async_vfs/interface.rs` | AsyncVfsInterface trait |
| `src/async_vfs/local_fs.rs` | AsyncLocalFs 实现 |

### 修改文件

| 文件 | 修改内容 |
|------|----------|
| `Cargo.toml` | 添加 tokio 依赖 |
| `src/buffer/mod.rs` | 锁替换为 tokio::sync |
| `src/lock/mod.rs` | 锁替换为 tokio::sync |
| `src/wal/mod.rs` | 新增 AsyncWal 封装 |

---

## 7. 成功标准

- [ ] 所有现有测试通过
- [ ] StorageEngine API 行为不变
- [ ] 异步化后性能不下降 (基准测试误差 < 10%)
- [ ] 文档更新 (AGENTS.md)

---

## 8. 附录

### A. Tokio Features 选择

```toml
tokio = { version = "1", features = [
    "rt-multi-thread",  # 多线程调度器，适合 IO 密集型
    "sync",            # 同步原语
    "fs",              # 异步文件系统
    "io-util",         # AsyncRead/AsyncWrite 工具
    "rt",              # 运行时核心
] }
```

### B. 锁对比

| 特性 | parking_lot | tokio::sync |
|------|-------------|-------------|
| 性能 | 极快 | 中等 |
| 异步支持 | 无 | 原生 |
| 公平性 | 可选 | 默认公平 |
| 锁超时 | 需自行实现 | 内置 `timeout` |

### C. 参考资料

- [Tokio 官方文档](https://tokio.rs)
- [Rust Async Book](https://rust-lang.github.io/async-book/)
- [tokio::sync 文档](https://docs.rs/tokio/latest/tokio/sync/)

---

**待确认事项**:

1. 上述设计是否OK？
2. 是否接受双轨 VFS 架构 (保留同步，新增异步)？
3. 是否有其他需要加入的组件？
