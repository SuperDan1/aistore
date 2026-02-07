# Tablespace & Segment Storage Design Document

## Overview

This document describes the design of a segment-page storage system inspired by MySQL InnoDB, implemented in Rust. The system provides:

- **Tablespace**: A logical container that can span multiple physical files
- **Segment**: A storage allocation unit for a table or index
- **Extent**: A contiguous allocation unit (1MB) using free list management
- **Page**: The basic I/O unit (8KB)
- **Buffer Pool**: In-memory caching of pages with LRU replacement

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         TablespaceManager                                │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────┐  │
│  │   SegmentDir    │  │  FileManager    │  │  BufferPoolMgr   │  │
│  │  (in-memory)   │  │  (multi-file)  │  │  (caching)     │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────┘  │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │                     Tablespace (per .ibd file)                │  │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────────────┐  │  │
│  │  │ File 0  │ │ File 1  │ │ File 2  │ │  ...           │  │  │
│  │  │ (Header│ │ (Header │ │ (Header │ │                 │  │  │
│  │  │ +Extents)│ │ +Extents)│ │ +Extents)│                 │  │  │
│  │  └─────────┘ └─────────┘ └─────────┘ └─────────────────┘  │  │
│  └─────────────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │                     Segment (per table/index)                     │  │
│  │  ┌─────────────────────┐ ┌─────────────────────────────────┐  │  │
│  │  │  SegmentHeader     │ │  Extent 0  │  Extent 1  │ ...│  │  │
│  │  │  (in directory)   │ │  (FreeList)│  (FreeList) │    │  │  │
│  │  └─────────────────────┘ └─────────────────────────────────┘  │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. Tablespace

A tablespace is a logical container that can span multiple physical files.

#### User Decisions (Confirmed)

- **Multi-file support**: Yes, tablespace can span multiple files
- **Initial files**: 1 file per tablespace (configurable)
- **File growth**: Auto-extend when space is exhausted

#### Tablespace Structure

```rust
/// Tablespace ID type
pub type TablespaceId = u64;

/// Tablespace configuration
#[derive(Debug, Clone)]
pub struct TablespaceConfig {
    /// Tablespace name
    pub name: String,
    /// Initial file size in bytes (default: 16MB)
    pub initial_file_size: usize,
    /// Number of initial files (default: 1)
    pub initial_files: u32,
    /// Auto-extend size in bytes (default: 16MB)
    pub auto_extend_size: usize,
    /// Maximum tablespace size (0 = unlimited)
    pub max_size: usize,
}

/// Tablespace status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TablespaceStatus {
    /// Being created
    Creating,
    /// Normal operation
    Active,
    /// Being dropped
    Dropping,
    /// After crash recovery
    Recovering,
}

/// Free list entry (extent information)
#[derive(Debug, Clone, Copy)]
pub struct FreeExtent {
    /// File ID containing this extent
    pub file_id: u32,
    /// Offset of extent in file
    pub extent_offset: u64,
    /// Number of free pages in this extent
    pub free_pages: u32,
}

/// Tablespace metadata (stored in control file and in-memory directory)
#[derive(Debug, Clone)]
pub struct TablespaceMeta {
    /// Tablespace ID
    pub tablespace_id: TablespaceId,
    /// Tablespace name
    pub name: String,
    /// Tablespace status
    pub status: TablespaceStatus,
    /// File paths (relative to data directory)
    pub file_paths: Vec<PathBuf>,
    /// Current file size per file
    pub file_size: u64,
    /// Number of segments in this tablespace
    pub segment_count: u32,
    /// Global free extent list (file_id, extent_offset, free_pages)
    pub free_list: Vec<FreeExtent>,
    /// Create timestamp
    pub created_at: Timestamp,
    /// Last modification timestamp
    pub modified_at: Timestamp,
}
```

### 2. File Management

Each tablespace can have one or more physical files. Files grow dynamically using free list management.

#### File Structure

```rust
/// File header (24 bytes, at offset 0 of each file)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FileHeader {
    /// Magic number "ASTR" (0x41535452)
    pub magic: u32,
    /// File version
    pub version: u32,
    /// File ID within tablespace
    pub file_id: u32,
    /// Tablespace ID
    pub tablespace_id: u64,
    /// File size in bytes
    pub file_size: u64,
    /// Number of extents in this file
    pub extent_count: u32,
    /// Number of free pages in this file
    pub free_pages: u32,
    /// Flags
    pub flags: u32,
    /// Checksum (CRC32)
    pub checksum: u32,
}

impl FileHeader {
    /// Size of file header (24 bytes)
    pub const SIZE: usize = 24;
    
    /// Create new header
    pub fn new(tablespace_id: TablespaceId, file_id: u32) -> Self {
        Self {
            magic: FILE_MAGIC,
            version: FILE_VERSION,
            file_id,
            tablespace_id,
            file_size: FILE_HEADER_SIZE as u64,
            extent_count: 0,
            free_pages: 0,
            flags: 0,
            checksum: 0,
        }
    }
    
    /// Validate magic number
    pub fn is_valid(&self) -> bool {
        self.magic == FILE_MAGIC
    }
}
```

#### Free List Management (InnoDB-style)

```rust
/// Extent header (stored at start of each extent)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ExtentHeader {
    /// File ID
    pub file_id: u32,
    /// Tablespace ID
    pub tablespace_id: u64,
    /// Offset of this extent in file
    pub extent_offset: u64,
    /// Number of pages in this extent
    pub page_count: u32,
    /// Number of free pages
    pub free_pages: u32,
    /// Bitmap of page usage (127 bits for usable pages)
    /// 1 = free, 0 = used
    pub page_bitmap: [u8; 16],
    /// Checksum (CRC32)
    pub checksum: u32,
}

impl ExtentHeader {
    /// Size of extent header (40 bytes + 16 bytes bitmap = 56 bytes)
    pub const SIZE: usize = 56;
    
    /// Number of pages per extent
    pub const PAGE_COUNT: u32 = 128;
    
    /// Usable pages per extent (excluding header page)
    pub const USABLE_PAGES: u32 = 127;
    
    /// Create new extent header
    pub fn new(tablespace_id: TablespaceId, file_id: u32, extent_offset: u64) -> Self {
        Self {
            file_id,
            tablespace_id,
            extent_offset,
            page_count: Self::PAGE_COUNT,
            free_pages: Self::USABLE_PAGES,
            page_bitmap: [0xFF; 16],  // All pages free (1 = free)
            checksum: 0,
        }
    }
    
    /// Allocate a page from this extent
    pub fn allocate_page(&mut self) -> Option<u32> {
        for i in 0..Self::USABLE_PAGES {
            let byte_idx = (i / 8) as usize;
            let bit_idx = i % 8;
            if self.page_bitmap[byte_idx] & (1 << bit_idx) != 0 {
                // Allocate page (mark as used: set bit to 0)
                self.page_bitmap[byte_idx] &= !(1 << bit_idx);
                self.free_pages -= 1;
                return Some(i);
            }
        }
        None
    }
    
    /// Free a page in this extent
    pub fn free_page(&mut self, page_idx: u32) {
        assert!(page_idx < Self::USABLE_PAGES);
        let byte_idx = (page_idx / 8) as usize;
        let bit_idx = page_idx % 8;
        self.page_bitmap[byte_idx] |= 1 << bit_idx;  // Mark as free
        self.free_pages += 1;
    }
    
    /// Check if extent is full
    pub fn is_full(&self) -> bool {
        self.free_pages == 0
    }
    
    /// Check if extent has free pages
    pub fn has_free_pages(&self) -> bool {
        self.free_pages > 0
    }
}
```

#### Free List Management

```rust
/// Free extent list (InnoDB-style)
pub struct FreeExtentList {
    /// List of free extents
    free_extents: Vec<FreeExtent>,
    /// Lock for thread safety
    lock: RwLock<()>,
}

impl FreeExtentList {
    /// Get a free extent with at least n free pages
    pub fn get_extent(&mut self, min_pages: u32) -> Option<FreeExtent> {
        for (i, extent) in self.free_extents.iter().enumerate() {
            if extent.free_pages >= min_pages {
                return Some(self.free_extents.remove(i));
            }
        }
        None
    }
    
    /// Return an extent to the free list
    pub fn return_extent(&mut self, extent: FreeExtent) {
        self.free_extents.push(extent);
        // Keep sorted by free_pages for efficient allocation
        self.free_extents.sort_by(|a, b| b.free_pages.cmp(&a.free_pages));
    }
    
    /// Add a newly allocated extent to free list
    pub fn add_extent(&mut self, file_id: u32, extent_offset: u64, free_pages: u32) {
        self.free_extents.push(FreeExtent {
            file_id,
            extent_offset,
            free_pages,
        });
    }
}
```

### 3. Segment Directory

Segments are tracked using an in-memory directory.

```rust
/// Segment directory entry
#[derive(Debug, Clone)]
pub struct SegmentDirEntry {
    /// Segment ID (unique within tablespace)
    pub segment_id: u64,
    /// Segment type
    pub segment_type: SegmentType,
    /// Tablespace ID
    pub tablespace_id: TablespaceId,
    /// File ID where segment header is stored
    pub header_file_id: u32,
    /// Offset of segment header in file
    pub header_offset: u64,
    /// First extent (file_id, offset)
    pub first_extent: Option<FreeExtent>,
    /// Last extent (file_id, offset)
    pub last_extent: Option<FreeExtent>,
    /// Total pages allocated
    pub total_pages: u64,
    /// Free pages available
    pub free_pages: u64,
    /// Create timestamp
    pub created_at: Timestamp,
    /// Last modification timestamp
    pub modified_at: Timestamp,
}

/// Segment directory (in-memory)
pub struct SegmentDirectory {
    /// Entries indexed by segment_id (1-indexed)
    entries: RwLock<Vec<Option<SegmentDirEntry>>>,
    /// Free list of extents
    free_list: FreeExtentList,
}

impl SegmentDirectory {
    /// Create new segment directory
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(Vec::new()),
            free_list: FreeExtentList::new(),
        }
    }
    
    /// Create a new segment
    pub fn create_segment(
        &self,
        tablespace_id: TablespaceId,
        segment_type: SegmentType,
    ) -> Result<u64, TablespaceError> {
        let _guard = self.entries.write().unwrap();
        
        // Allocate segment ID
        let segment_id = (self.entries.read().unwrap().len() + 1) as u64;
        
        // Allocate first extent from free list
        let extent = self.free_list.get_extent(ExtentHeader::USABLE_PAGES)
            .ok_or(TablespaceError::NoFreeExtent)?;
        
        // Create directory entry
        let entry = SegmentDirEntry {
            segment_id,
            segment_type,
            tablespace_id,
            header_file_id: extent.file_id,
            header_offset: extent.extent_offset,
            first_extent: Some(extent),
            last_extent: None,
            total_pages: 0,
            free_pages: 0,
            created_at: current_timestamp(),
            modified_at: current_timestamp(),
        };
        
        // Ensure entries vector is large enough
        while self.entries.read().unwrap().len() < segment_id as usize {
            self.entries.write().unwrap().push(None);
        }
        
        // Store entry
        self.entries.write().unwrap()[segment_id as usize - 1] = Some(entry);
        
        Ok(segment_id)
    }
    
    /// Look up a segment by ID
    pub fn get_segment(&self, segment_id: u64) -> Option<SegmentDirEntry> {
        let entries = self.entries.read().unwrap();
        if segment_id == 0 || segment_id > entries.len() as u64 {
            return None;
        }
        entries[segment_id as usize - 1].clone()
    }
}
```

### 4. Segment Types

```rust
/// Segment type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentType {
    /// Data segment (stores table rows)
    Data,
    /// Index segment (stores index data)
    Index,
    /// Rollback segment (for transactions)
    Rollback,
    /// System metadata segment
    System,
    /// Temporary segment (for sorting operations)
    Temporary,
    /// Undo segment (for MVCC)
    Undo,
}
```

### 5. Page

Pages are the basic I/O unit.

```rust
/// Page types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum PageType {
    /// Unused page
    Invalid = 0,
    /// Freshly allocated page
    Fresh = 1,
    /// Data page (contains table rows)
    Data = 2,
    /// Index page (contains index entries)
    Index = 3,
    /// Rollback segment page
    Rollback = 4,
    /// System page (segment header, etc.)
    System = 5,
    /// File header page
    FileHeader = 6,
    /// Extent descriptor page
    ExtentDescriptor = 7,
    /// BLOB page
    Blob = 8,
}
```

## Storage Layout

### File Layout

```
┌─────────────────────────────────────────────────────────────────┐
│ Tablespace File N (.ibd)                                        │
├─────────────────────────────────────────────────────────────────┤
│ Offset 0                                                        │
│ ┌─────────────────────────────────────────────────────┐  │
│ │ FileHeader (24 B)                                   │  │
│ │ - magic: ASTR                                       │  │
│ │ - file_id: N                                        │  │
│ │ - tablespace_id: X                                   │  │
│ │ - file_size: 16MB (initial)                         │  │
│ │ - extent_count: 16                                   │  │
│ │ - free_pages: 2032 (16 extents * 127 pages)         │  │
│ └─────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│ Extent 0 (0x18 = 24)                                           │
│ ┌─────────────────────────────────────────────────────┐  │
│ │ ExtentHeader (56 B)                                  │  │
│ │ - file_id: 0                                        │  │
│ │ - tablespace_id: X                                   │  │
│ │ - extent_offset: 0x18                               │  │
│ │ - page_bitmap: [0xFF; 16]                          │  │
│ │ - free_pages: 127                                    │  │
│ └─────────────────────────────────────────────────────┘  │
│ ┌─────────────────────────────────────────────────────┐  │
│ │ Page 0 (8 KB) - Segment Header Page                 │  │
│ │ ┌───────────────────────────────────────────────┐  │  │
│ │ │ (Segment metadata stored in directory)        │  │  │
│ └───────────────────────────────────────────────┘  │  │
│ │ Page 1 (8 KB) - First data page                │  │
│ │ ...                                                 │  │
│ │ Page 126 (8 KB) - Last usable page               │  │
│ └─────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│ Extent 1 (0x100018 = 1,048,600)                             │
│ ...                                                         │
└─────────────────────────────────────────────────────────────────┘
```

## Allocation Strategies

### Extent Allocation (Free List)

```
allocate_extent(tablespace_id):
1. Check tablespace's free list
2. If free list has extent with free pages:
   a. Remove extent from free list
   b. Initialize extent header
   c. Add extent to segment's extent chain
3. If no free extent:
   a. Extend existing file or create new file
   b. Initialize new extent
   c. Add to free list (partially used)
4. Return extent (file_id, offset)
```

### Page Allocation

```
allocate_page(segment_id):
1. Look up segment in directory
2. Check segment's last extent for free pages
3. If extent has free pages:
   a. Allocate page from extent bitmap
   b. Update extent header
4. If extent full:
   a. Move extent to segment's full list
   b. Get new extent from free list
   c. Allocate page from new extent
5. Update segment metadata
6. Return page_id
```

## API Reference

### TablespaceManager

```rust
pub trait TablespaceManager {
    /// Create a new tablespace
    fn create_tablespace(
        &mut self,
        name: &str,
        config: TablespaceConfig,
    ) -> Result<TablespaceId, TablespaceError>;
    
    /// Open an existing tablespace
    fn open_tablespace(&self, name: &str) -> Result<TablespaceId, TablespaceError>;
    
    /// Drop a tablespace
    fn drop_tablespace(&mut self, tablespace_id: TablespaceId) -> Result<(), TablespaceError>;
    
    /// Create a segment within a tablespace
    fn create_segment(
        &mut self,
        tablespace_id: TablespaceId,
        segment_type: SegmentType,
    ) -> Result<u64, TablespaceError>;
    
    /// Allocate a page in a segment
    fn allocate_page(
        &mut self,
        tablespace_id: TablespaceId,
        segment_id: u64,
    ) -> Result<PageId, TablespaceError>;
    
    /// Read a page
    fn read_page(
        &self,
        tablespace_id: TablespaceId,
        segment_id: u64,
        page_id: PageId,
    ) -> Result<Vec<u8>, TablespaceError>;
    
    /// Write a page
    fn write_page(
        &self,
        tablespace_id: TablespaceId,
        segment_id: u64,
        page_id: PageId,
        data: &[u8],
    ) -> Result<(), TablespaceError>;
}
```

## Implementation Roadmap

### Phase 1: Core Infrastructure (TablespaceManager with Multi-file Support)
- [ ] TablespaceManager struct and basic operations
- [ ] File management (open, create, extend, sync)
- [ ] FileHeader and ExtentHeader structures
- [ ] Free extent list management
- [ ] Multi-file support (extend to new files)

### Phase 2: Segment Directory and Management
- [ ] SegmentDirectory structure
- [ ] Segment creation and lookup
- [ ] Segment metadata tracking
- [ ] Extent chain management (first/last extent)

### Phase 3: Page Operations
- [ ] Page allocation within segment
- [ ] Page read/write operations
- [ ] Page checksum verification
- [ ] Page type handling

### Phase 4: Buffer Pool Integration
- [ ] Integrate with existing BufferMgr
- [ ] Page caching in buffer pool
- [ ] Dirty page tracking and flush

## Comparison with Previous Design

| Feature | Updated Design | Previous Design |
|---------|----------------|----------------|
| Multi-file | Yes | Not specified |
| Extent allocation | Free list | Linked list |
| Segment lookup | Segment directory | Simple formula |
| Page ID | Direct (u64) | (tablespace<<48\|...) |
| Extent header | 56 bytes | 40 bytes |
| Free list | InnoDB-style | Not specified |

## Performance Considerations

### Free List Optimization

```rust
/// Free list keeps extents sorted by free_pages
/// Allocation strategy: Best Fit
/// - Search for extent with closest match to requested pages
/// - Reduces fragmentation
```

### Extent Size Trade-offs

| Size | Pros | Cons |
|------|------|------|
| 1 MB | Standard for most DBMS | Waste for small tables |
| 64 KB | Less waste | More extent headers |

## Next Steps

Based on user decisions:
1. ✅ Multi-file tablespace support
2. ✅ Free list management (InnoDB-style)
3. ✅ Segment directory
4. ✅ Direct PageId (u64)
5. Phase 1-4 implementation

Ready to begin Phase 1 implementation.
