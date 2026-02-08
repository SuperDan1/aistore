# 段页式存储设计文档

## 1. 概述

### 1.1 设计目标
在Segment模块实现段页式存储机制，通过调用BufferMgr接口实现页面读写，继承现有VFS文件系统能力。

### 1.2 核心约束
- **Page Size**: 8KB (复用 `BLOCK_SIZE = PAGE_SIZE`)
- **Extent Size**: 1MB = 128 pages
- 页面在extent内连续分配
- 通过file_offset直接定位页面

---

## 2. 存储层次结构

```
File
├── File Header (固定位置，文件起始处)
│   ├── magic number
│   ├── file_version
│   ├── file_size
│   ├── segment_count
│   └── checksum
│
├── Segment 1
│   ├── Segment Header (第一个extent的第一个page)
│   │   ├── segment_id
│   │   ├── segment_type
│   │   ├── next_extent_ptr (下一个extent的header位置)
│   │   ├── total_pages
│   │   └── checksum
│   │
│   ├── Extent 1 (1MB)
│   │   ├── Extent Header (Page 0)
│   │   │   └── next_extent_ptr
│   │   ├── Page 1 (可用)
│   │   ├── Page 2 (可用)
│   │   └── ...
│   │       └── Page 127 (可用)
│   │
│   ├── Extent 2 (1MB)
│   │   ├── Extent Header (Page 0)
│   │   │   └── next_extent_ptr
│   │   ├── Page 1 ~ Page 127 (可用)
│   │   └── Page 128 (可用)
│   │
│   └── Extent N ...
│
├── Segment 2 ...
└── ...
```

### 2.1 关键特性
- **Extent Header**: 只占用一个page，存储 `next_extent_ptr`
- **Segment Header**: 占用第一个extent的第一个page（Page 0）
- **可用页面**: 每个extent的Page 1 ~ Page 127（共127个可用页面）

---

## 3. 数据结构定义

### 3.1 File Header

**位置**: 文件起始偏移 0

```rust
/// 文件头部
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct FileHeader {
    /// 魔数，标识有效文件
    pub magic: u32,
    /// 文件版本号
    pub version: u32,
    /// 当前文件大小（字节）
    pub file_size: u64,
    /// Segment数量
    pub segment_count: u32,
    /// 文件头部校验和
    pub checksum: u32,
}

impl FileHeader {
    /// 文件魔数
    pub const MAGIC: u32 = 0x41535452; // "ASTR" in little endian

    /// 当前版本
    pub const VERSION: u32 = 1;

    /// 头部大小
    pub const SIZE: usize = 24; // 4 + 4 + 8 + 4 + 4 bytes

    /// 校验文件有效性
    pub fn is_valid(&self) -> bool {
        self.magic == Self::MAGIC
    }

    /// 计算校验和（不包括checksum字段本身）
    pub fn compute_checksum(&self) -> u32 {
        let mut header = *self;
        header.checksum = 0;
        crc32fast::hash(to_bytes(&header))
    }

    /// 验证校验和
    pub fn verify_checksum(&self) -> bool {
        self.compute_checksum() == self.checksum
    }
}
```

### 3.2 Extent Header

**位置**: 每个extent的第一个page（Page 0）

```rust
/// Extent头部
/// 存储在extent的第一个page中（不计入可用页面）
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ExtentHeader {
    /// 下一个extent的header位置（0表示无）
    pub next_extent_ptr: u64,
}

impl ExtentHeader {
    /// Extent大小（字节）
    pub const SIZE: usize = 1 << 20; // 1MB
    /// Extent内页面数
    pub const PAGE_COUNT: u32 = 128;
    /// 可用页面数（除去header page）
    pub const USABLE_PAGES: u32 = Self::PAGE_COUNT - 1; // 127
}
```

### 3.3 Segment Header

**位置**: Segment第一个extent的第一个page（Segment的Page 0）

```rust
/// Segment类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentType {
    /// 通用Segment（当前实现）
    Generic,
    /// 数据Segment（后续实现）
    Data,
    /// 索引Segment（后续实现）
    Index,
    /// 元数据Segment（后续实现）
    Metadata,
}

/// Segment头部
/// 存储在segment第一个extent的第一个page中
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct SegmentHeader {
    /// Segment ID
    pub segment_id: u64,
    /// Segment类型
    pub segment_type: SegmentType,
    /// 下一个extent的header位置（0表示无）
    pub next_extent_ptr: u64,
    /// 总页面数
    pub total_pages: u64,
    /// Segment头部校验和
    pub checksum: u32,
}

impl SegmentHeader {
    /// Segment头部大小
    pub const SIZE: usize = 24; // 8 + 1 + 7(padding) + 8 + 8 + 4 bytes

    /// 计算校验和
    pub fn compute_checksum(&self) -> u32 {
        let mut header = *self;
        header.checksum = 0;
        crc32fast::hash(to_bytes(&header))
    }

    /// 验证校验和
    pub fn verify_checksum(&self) -> bool {
        self.compute_checksum() == self.checksum
    }
}
```

---

## 4. 核心接口设计

### 4.1 SegmentManager

```rust
/// Segment管理器
pub struct SegmentManager {
    /// 虚拟文件系统接口
    vfs: Arc<dyn VfsInterface>,
    /// 文件路径
    file_path: String,
    /// 文件句柄
    file: RwLock<Box<dyn FileHandle>>,
    /// File Header缓存
    file_header: RwLock<FileHeader>,
    /// 全局Extent分配锁
    extent_alloc_lock: RwLock<()>,
}

impl SegmentManager {
    /// 创建新的Segment
    pub fn create_segment(
        &self,
        segment_type: SegmentType,
    ) -> AistoreResult<SegmentId> {
        // 1. 获取分配锁
        let _guard = self.extent_alloc_lock.write();

        // 2. 创建新的extent
        let extent_ptr = self.allocate_extent()?;

        // 3. 初始化segment header
        let header = SegmentHeader {
            segment_id: self.generate_segment_id()?,
            segment_type,
            next_extent_ptr: 0,
            total_pages: 0,
            checksum: 0,
        };
        header.checksum = header.compute_checksum();

        // 4. 写入segment header到extent第一个page
        let page_data = serialize_to_bytes(&header);
        self.write_page(extent_ptr, 0, &page_data)?;

        // 5. 更新file header
        let mut fh = self.file_header.write();
        fh.segment_count += 1;
        fh.file_size = extent_ptr + ExtentHeader::SIZE as u64;
        fh.checksum = fh.compute_checksum();

        // 6. 刷写file header
        self.file.write().pread(to_bytes(&*fh), 0)?;

        Ok(header.segment_id)
    }

    /// 分配新的extent
    fn allocate_extent(&self) -> AistoreResult<u64> {
        // 从file header获取当前文件大小作为新extent的起始位置
        let fh = self.file_header.read();
        let extent_ptr = fh.file_size;
        drop(fh);

        // 扩展文件大小
        let new_size = extent_ptr + ExtentHeader::SIZE as u64;
        self.file.write().truncate(new_size)?;

        // 更新file header
        let mut fh = self.file_header.write();
        fh.file_size = new_size;
        fh.checksum = fh.compute_checksum();

        Ok(extent_ptr)
    }

    /// 获取Segment页面
    pub fn get_page(
        &self,
        segment_id: SegmentId,
        page_idx: PageId,
    ) -> AistoreResult<BufferDesc> {
        // 1. 定位segment header位置
        let segment_offset = self.locate_segment(segment_id)?;

        // 2. 根据page_idx计算文件偏移
        let file_offset = self.page_to_file_offset(segment_offset, page_idx)?;

        // 3. 调用BufferMgr读取页面
        let tag = BufferTag {
            file_id: self.get_file_id(),
            block_id: (file_offset / PAGE_SIZE as u64) as u32,
        };

        let buffer = self.buffer_mgr.read(tag);
        if buffer.is_null() {
            // Buffer未命中，从文件加载
            self.load_page_to_buffer(tag, file_offset)?;
        }

        Ok(buffer)
    }

    /// 分配新页面
    pub fn allocate_page(&self, segment_id: SegmentId) -> AistoreResult<PageId> {
        // 1. 获取Segment Header
        let segment_offset = self.locate_segment(segment_id)?;
        let header = self.read_segment_header(segment_offset)?;

        // 2. 检查当前extent是否已满
        let page_idx = header.total_pages;
        let extent_idx = page_idx / ExtentHeader::USABLE_PAGES;
        let page_in_extent = page_idx % ExtentHeader::USABLE_PAGES;

        // 3. 如果当前extent已满，分配新extent
        if page_in_extent == 0 && page_idx > 0 {
            let new_extent_ptr = self.allocate_extent()?;

            // 将新extent链接到当前extent
            let current_extent_ptr = self.extent_ptr_from_page(segment_offset, page_idx - 1)?;
            self.link_extent(current_extent_ptr, new_extent_ptr)?;

            // 更新segment header
            let mut header = self.read_segment_header(segment_offset)?;
            header.next_extent_ptr = new_extent_ptr;
            header.checksum = header.compute_checksum();
            self.write_segment_header(segment_offset, &header)?;
        }

        // 4. 更新segment header的total_pages
        let mut header = self.read_segment_header(segment_offset)?;
        header.total_pages += 1;
        header.checksum = header.compute_checksum();
        self.write_segment_header(segment_offset, &header)?;

        Ok(page_idx)
    }

    /// 页面偏移转换为文件偏移
    fn page_to_file_offset(
        &self,
        segment_offset: u64,
        page_idx: PageId,
    ) -> AistoreResult<u64> {
        let extent_idx = page_idx / ExtentHeader::USABLE_PAGES;
        let page_in_extent = page_idx % ExtentHeader::USABLE_PAGES;

        // 遍历extent链表
        let mut current_extent_ptr = segment_offset;
        for _ in 0..extent_idx {
            let header = self.read_extent_header(current_extent_ptr)?;
            if header.next_extent_ptr == 0 {
                return Err(AistoreError::NotFound);
            }
            current_extent_ptr = header.next_extent_ptr;
        }

        // 计算文件偏移：extent起始 + header page + page_idx_in_extent
        Ok(current_extent_ptr + (page_in_extent + 1) as u64 * PAGE_SIZE as u64)
    }

    /// 从segment offset定位extent指针
    fn extent_ptr_from_page(
        &self,
        segment_offset: u64,
        page_idx: PageId,
    ) -> AistoreResult<u64> {
        if page_idx == 0 {
            return Ok(segment_offset);
        }
        let extent_idx = (page_idx - 1) / ExtentHeader::USABLE_PAGES;
        self.page_to_file_offset(segment_offset, extent_idx * ExtentHeader::USABLE_PAGES)
    }

    /// 链接两个extent
    fn link_extent(&self, from_ptr: u64, to_ptr: u64) -> AistoreResult<()> {
        let mut header = self.read_extent_header(from_ptr)?;
        header.next_extent_ptr = to_ptr;
        self.write_extent_header(from_ptr, &header)
    }
}
```

### 4.2 Buffer集成

```rust
/// BufferMgr扩展接口
impl BufferMgr {
    /// 从文件加载页面到Buffer
    fn load_page_to_buffer(&self, tag: BufferTag, file_offset: u64) -> AistoreResult<*mut BufferDesc> {
        // 1. 检查Buffer是否有空间
        // 2. 从文件读取page_size字节到Buffer
        // 3. 建立BufferTag到BufferDesc的映射

        // 具体实现后续在buffer模块完善
        unimplemented!()
    }

    /// 刷写Buffer到文件
    fn flush_page(&self, tag: &BufferTag) -> AistoreResult<()> {
        // 具体实现后续在buffer模块完善
        unimplemented!()
    }
}
```

---

## 5. 并发控制

### 5.1 锁策略
- **全局Extent分配锁** (`extent_alloc_lock`): 控制extent分配操作
- **BufferMgr content_lock**: 控制页面读写并发（复用现有机制）
- **File Header更新锁**: 保护file header一致性

### 5.2 页面级并发
页面读写通过BufferMgr的 `content_lock` 控制，与现有设计一致。

---

## 6. 文件布局示例

### 6.1 初始状态（空文件）

```
Offset 0      +-------------------+
              | File Header       | (24 bytes)
              +-------------------+
24            |                   |
              |    (空闲空间)      |
              |                   |
              +-------------------+
```

### 6.2 创建第一个Segment后

```
Offset 0      +-------------------+
              | File Header       |
              +-------------------+
24            +-------------------+
              | Segment 1 Header  | <-- Extent 1, Page 0
              | (SegmentHeader)   |
              +-------------------+
8KB           |                   |
              |  Extent 1 Page 1  | (可用)
              |                   |
              +-------------------+
16KB          |                   |
              |  Extent 1 Page 2  | (可用)
              |                   |
              +-------------------+
...           |                   |
              |                   |
1MB           +-------------------+
              | Extent 2 Header   | (尚未分配)
              +-------------------+
```

### 6.3 Segment扩展后

```
Offset 0      +-------------------+
              | File Header       |
              +-------------------+
              | Segment 1 Header  |
              +-------------------+
8KB           | Extent 1 Page 1   |
              |                   |
              +-------------------+
...           |                   |
              |                   |
1MB           +-------------------+
              | Extent 1 Header   | <-- next_extent_ptr -> Extent 2
              +-------------------+
1MB + 8KB     | Extent 2 Page 1   |
              |                   |
              +-------------------+
...           |                   |
2MB           +-------------------+
              | Extent 2 Header   |
              +-------------------+
```

---

## 7. 实现优先级

### Phase 1: 基础结构
- [ ] File Header读写
- [ ] Segment Header读写
- [ ] Extent Header读写
- [ ] 页面分配（allocate_page）
- [ ] 页面读取（get_page）

### Phase 2: Buffer集成
- [ ] BufferMgr页面加载
- [ ] Buffer未命中处理
- [ ] 页面置换策略（后续）

### Phase 3: 扩展功能
- [ ] 多Segment支持
- [ ] Segment类型扩展（Data/Index/Metadata）
- [ ] 页面回收（可选）

---

## 8. 关键常量定义

```rust
/// 文件魔数
pub const FILE_MAGIC: u32 = 0x41535452;

/// 文件版本
pub const FILE_VERSION: u32 = 1;

/// Extent大小（1MB）
pub const EXTENT_SIZE: usize = 1 << 20;

/// Extent内页面数
pub const EXTENT_PAGE_COUNT: u32 = 128;

/// Extent可用页面数
pub const EXTENT_USABLE_PAGES: u32 = EXTENT_PAGE_COUNT - 1;

/// Segment头部大小
pub const SEGMENT_HEADER_SIZE: usize = 24;

/// 文件头部大小
pub const FILE_HEADER_SIZE: usize = 24;
```

---

## 9. 错误类型

```rust
/// Segment相关错误
#[derive(Debug, thiserror::Error)]
pub enum SegmentError {
    #[error("Segment not found: {0}")]
    NotFound(SegmentId),

    #[error("Extent not found at offset: {0}")]
    ExtentNotFound(u64),

    #[error("Page out of bounds: {page_idx} in segment {segment_id}")]
    PageOutOfBounds { segment_id: SegmentId, page_idx: PageId },

    #[error("Invalid file header")]
    InvalidFileHeader,

    #[error("Invalid segment header")]
    InvalidSegmentHeader,

    #[error("Invalid extent header")]
    InvalidExtentHeader,

    #[error("Checksum mismatch")]
    ChecksumMismatch,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type SegmentResult<T> = Result<T, SegmentError>;
```

---

## 10. 后续待确认问题

1. File Header是否需要持久化存储？目前设计为每次更新都刷盘
2. Segment ID生成策略？（自增？UUID？）
3. 页面分配是否需要考虑线程安全？

---

*文档版本: 1.0*
*最后更新: 2026-02-05*
