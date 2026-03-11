# 研究发现

## 2026-03-07

### 发现 1: BufferMgr 页面管理机制

待研究...

### 发现 2: RecoveryManager replay 逻辑

在 `wal/recovery.rs` 中：
- `replay_from_lsn` 从指定 LSN 开始读取日志记录
- 对于 `PageRedo` 类型，调用回调写入页面
- 对于 `TxCommit`，只计数不执行操作

### 发现 3: Checkpoint 结构

待研究...

### 发现 4: 当前恢复调用链

```
StorageEngine::new()
  └─> WalManager::new()
        └─> RecoveryManager::new()  // 创建了但从未调用
```

## 待确认问题

1. BufferMgr 如何管理脏页？恢复的页是否需要走相同流程？
2. checkpoint 保存了哪些信息？
3. 未提交事务如何回滚？
