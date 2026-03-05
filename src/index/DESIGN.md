# 索引模块设计文档

## 1. 核心决策

| 需求 | 方案 |
|------|------|
| 存储位置 | 同 tablespace，不同 segment (SegmentType::Index) |
| 键类型 | 所有 ColumnType，第一版 Int64 |
| 多列索引 | 联合键 (concat serialization) |
| 唯一约束 | 插入前检查，冲突返回错误 |
| NULL 值 | 允许索引 NULL 值 |
| 键大小 | 最大 1024 字节 |
| 填充因子 | 默认 0.8 |

## 2. 模块结构

```
src/index/
├── mod.rs              # IndexManager 主模块
├── btree.rs            # B+Tree 实现
├── key.rs              # 键序列化
└── meta.rs             # 索引元数据
```

## 3. 索引元数据

```rust
pub struct IndexMeta {
    pub id: u64,
    pub name: String,
    pub table_id: u64,
    pub columns: Vec<String>,
    pub is_unique: bool,
    pub root_page_id: PageId,
    pub segment_id: u64,
    pub fill_factor: f32,  // 默认 0.8
    pub max_key_size: usize, // 默认 1024
}
```

## 4. 键序列化

```rust
// Int64 序列化 (big-endian for unsigned comparison)
fn serialize_int64(v: i64) -> Vec<u8>

// 多列联合键
fn serialize_key(values: &[Value], columns: &[Column]) -> Vec<u8>
```

## 5. B+Tree

- 内部节点: [key1, child1, key2, child2, ...]
- 叶子节点: [key1, rid1, key2, rid2, ...] + 双向链表
- 页面大小: 64KB (PAGE_SIZE)
- 填充因子: 0.8

## 6. IndexManager API

```rust
impl IndexManager {
    pub fn create_index(&mut self, table_id: u64, name: String, columns: Vec<String>, is_unique: bool) -> IndexResult<u64>;
    pub fn drop_index(&mut self, index_id: u64) -> IndexResult<()>;
    pub fn insert(&self, index_id: u64, key: &[u8], rid: RowId) -> IndexResult<()>;
    pub fn delete(&self, index_id: u64, key: &[u8], rid: RowId) -> IndexResult<()>;
    pub fn lookup(&self, index_id: u64, key: &[u8]) -> IndexResult<Vec<RowId>>;
}
```

## 7. 实现顺序

1. KeyCodec - 键序列化
2. B+Tree 搜索
3. B+Tree 插入/删除
4. IndexManager + 持久化
5. 与 StorageEngine 集成
6. CREATE/DROP INDEX
