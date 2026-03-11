# Crash Recovery 集成计划

## 目标
在 StorageEngine 启动时集成 WAL Crash Recovery，实现 ARIES 恢复算法的完整流程。

## 问题陈述

| 组件 | 问题 |
|------|------|
| StorageEngine | 启动时不调用 wal.recover() |
| WalManager | recover 传入空回调 `|_, _| Ok(())` |
| BufferMgr | 没有接收恢复页面的公开接口 |

## 阶段规划

### Phase 1: 研究现有代码结构 ✅
- [x] 分析 BufferMgr 内部结构，了解页面管理机制
- [x] 分析 RecoveryManager 的 replay 逻辑
- [x] 确定页面写入 BufferMgr 的正确方式
- [x] 分析 checkpoint 格式，理解恢复起点

### Phase 2: 设计集成方案 🔄
- [x] 设计 BufferMgr 恢复接口
- [x] 设计 StorageEngine 启动恢复流程
- [ ] 设计事务回滚逻辑

---

## 详细设计

### 设计 1: BufferMgr 恢复接口

```rust
// src/buffer/mod.rs

impl BufferMgr {
    /// 从恢复加载页面到 buffer pool（不经过 LRU 淘汰）
    /// 仅在恢复时使用
    pub fn recover_page(&mut self, page_id: PageId, data: &[u8]) -> Result<(), BufferError> {
        // 1. 检查页面是否已在 buffer 中
        if let Some(buffer_idx) = self.lookup(page_id) {
            // 已在 buffer 中，直接写入数据
            self.page_data[buffer_idx].copy_from_slice(data);
            return Ok(());
        }

        // 2. 分配新 buffer
        let buffer_idx = self.allocate_buffer(page_id)?;
        
        // 3. 写入数据
        self.page_data[buffer_idx].copy_from_slice(data);
        
        // 4. 注册 hash entry
        self.insert_hash_entry(page_id, buffer_idx);
        
        // 5. 加入 LRU（可选，不加也没关系）
        self.lru.add(buffer_idx);
        
        // 6. 标记为 clean（恢复的页面是从 WAL 重放的，已是最新）
        let buffer = unsafe { &*self.buffers.add(buffer_idx) };
        buffer.clear_dirty();
        
        Ok(())
    }
}
```

### 设计 2: StorageEngine 启动恢复流程

```rust
// src/storage.rs

impl StorageEngine {
    /// 从崩溃中恢复
    pub fn recover(&mut self) -> Result<RecoveryReport, StorageError> {
        let wal = self.wal.as_ref()
            .ok_or_else(|| StorageError::Other("WAL not initialized".to_string()))?;

        // 1. 获取恢复报告（包含 checkpoint_lsn 和 active_transactions）
        let result = wal.recover_with_callback(|page_id, data| {
            self.buffer_mgr
                .write()
                .recover_page(page_id, data)
                .map_err(|e| e.to_string())
        })?;

        // 2. 回滚未提交的事务
        for tx_id in &result.rolled_back_transactions {
            self.rollback_transaction(*tx_id)?;
        }

        Ok(RecoveryReport {
            replayed_records: result.replayed_records,
            rolled_back_transactions: result.rolled_back_transactions.len(),
        })
    }

    fn rollback_transaction(&mut self, tx_id: TransactionId) -> StorageResult<()> {
        // TODO: 实现事务回滚逻辑
        // 1. 从 LockManager 获取事务持有的锁
        // 2. 遍历 undo log
        // 3. 应用 before_image
        // 4. 释放锁
        Ok(())
    }
}
```

### 设计 3: WalManager 恢复接口

```rust
// src/wal/mod.rs

impl WalManager {
    /// 带回调的恢复方法
    pub fn recover_with_callback<F>(&self, write_page: F) -> RecoveryResult
    where
        F: Fn(PageId, &[u8]) -> Result<(), String>,
    {
        if let Some(ref mgr) = *self.recovery_mgr.read() {
            mgr.recover(write_page)
        } else {
            RecoveryResult {
                checkpoint_lsn: LSN::invalid(),
                replayed_records: 0,
                rolled_back_transactions: Vec::new(),
            }
        }
    }
}
```

---

## 实施任务清单

| 优先级 | 任务 | 涉及文件 |
|--------|------|----------|
| P0 | 添加 `BufferMgr::recover_page()` | src/buffer/mod.rs |
| P0 | 修复 `WalManager.recover()` 回调 | src/wal/mod.rs |
| P0 | 在 `StorageEngine::new()` 调用 recover | src/storage.rs |
| P1 | 实现事务回滚逻辑 | src/storage.rs |
| P1 | 添加恢复相关错误类型 | src/storage.rs |
| P2 | 添加单元测试 | src/wal/recovery.rs |

### Phase 3: 实施
- [ ] 为 BufferMgr 添加恢复页面接口
- [ ] 修复 WalManager.recover 回调
- [ ] 在 StorageEngine 启动时调用 recover
- [ ] 实现未提交事务回滚

### Phase 4: 测试
- [ ] 单元测试：恢复流程
- [ ] 集成测试：崩溃恢复场景

## 当前状态
- **Phase**: 1 (研究中)
- **开始时间**: 2026-03-07

## 关键文件
- `src/storage.rs` - StorageEngine
- `src/wal/mod.rs` - WalManager
- `src/wal/recovery.rs` - RecoveryManager
- `src/buffer/` - BufferMgr
