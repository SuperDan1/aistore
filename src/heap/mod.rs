//! Heap storage module
//!
//! Provides heap table storage for tuples with slot-based page layout.

use crate::table::{Column, Table};
use crate::types::{PageId, PAGE_SIZE};
use std::collections::HashMap;
use std::sync::Arc;

/// Heap result type
pub type HeapResult<T> = Result<T, HeapError>;

/// Heap error types
#[derive(Debug, Clone)]
pub enum HeapError {
    PageNotFound(PageId),
    OutOfSpace,
    InvalidSlot(usize),
    SerializationError(String),
    TupleNotFound(RowId),
    Other(String),
}

impl std::fmt::Display for HeapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HeapError::PageNotFound(id) => write!(f, "Page not found: {}", id),
            HeapError::OutOfSpace => write!(f, "Out of space in page"),
            HeapError::InvalidSlot(idx) => write!(f, "Invalid slot: {}", idx),
            HeapError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            HeapError::TupleNotFound(id) => write!(f, "Tuple not found: {:?}", id),
            HeapError::Other(msg) => write!(f, "Heap error: {}", msg),
        }
    }
}

impl std::error::Error for HeapError {}

/// Row ID - uniquely identifies a tuple in the heap
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RowId {
    pub page_id: PageId,
    pub slot_idx: usize,
}

impl RowId {
    pub fn new(page_id: PageId, slot_idx: usize) -> Self {
        Self { page_id, slot_idx }
    }
}

/// Slot entry in the page slot array
#[derive(Debug, Clone, Copy)]
#[repr(packed)]
struct SlotEntry {
    offset: i32,
    length: u32,
}

impl SlotEntry {
    fn new(offset: i32, length: u32) -> Self {
        Self { offset, length }
    }
}

/// Tuple value representation
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    UInt8(u8),
    UInt16(u16),
    UInt32(u32),
    UInt64(u64),
    Float32(f32),
    Float64(f64),
    Boolean(bool),
    VarChar(String),
    Blob(Vec<u8>),
}

impl Value {
    pub fn serialized_size(&self) -> usize {
        match self {
            Value::Null => 0,
            Value::Int8(_) | Value::UInt8(_) | Value::Boolean(_) => 1,
            Value::Int16(_) | Value::UInt16(_) => 2,
            Value::Int32(_) | Value::UInt32(_) | Value::Float32(_) => 4,
            Value::Int64(_) | Value::UInt64(_) | Value::Float64(_) => 8,
            Value::VarChar(s) => s.len(),
            Value::Blob(b) => b.len(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        match self {
            Value::Null => vec![],
            Value::Int8(v) => v.to_le_bytes().to_vec(),
            Value::Int16(v) => v.to_le_bytes().to_vec(),
            Value::Int32(v) => v.to_le_bytes().to_vec(),
            Value::Int64(v) => v.to_le_bytes().to_vec(),
            Value::UInt8(v) => v.to_le_bytes().to_vec(),
            Value::UInt16(v) => v.to_le_bytes().to_vec(),
            Value::UInt32(v) => v.to_le_bytes().to_vec(),
            Value::UInt64(v) => v.to_le_bytes().to_vec(),
            Value::Float32(v) => v.to_le_bytes().to_vec(),
            Value::Float64(v) => v.to_le_bytes().to_vec(),
            Value::Boolean(v) => [*v as u8].to_vec(),
            Value::VarChar(s) => s.as_bytes().to_vec(),
            Value::Blob(b) => b.clone(),
        }
    }

    pub fn deserialize(data: &[u8], col_type: &crate::types::ColumnType) -> HeapResult<Self> {
        if data.is_empty() {
            return Ok(Value::Null);
        }

        match col_type {
            crate::types::ColumnType::Int8 => {
                let arr: [u8; 1] = data
                    .try_into()
                    .map_err(|e| HeapError::SerializationError(format!("{:?}", e)))?;
                Ok(Value::Int8(i8::from_le_bytes(arr)))
            }
            crate::types::ColumnType::Int16 => {
                let arr: [u8; 2] = data
                    .try_into()
                    .map_err(|e| HeapError::SerializationError(format!("{:?}", e)))?;
                Ok(Value::Int16(i16::from_le_bytes(arr)))
            }
            crate::types::ColumnType::Int32 => {
                let arr: [u8; 4] = data
                    .try_into()
                    .map_err(|e| HeapError::SerializationError(format!("{:?}", e)))?;
                Ok(Value::Int32(i32::from_le_bytes(arr)))
            }
            crate::types::ColumnType::Int64 => {
                let arr: [u8; 8] = data
                    .try_into()
                    .map_err(|e| HeapError::SerializationError(format!("{:?}", e)))?;
                Ok(Value::Int64(i64::from_le_bytes(arr)))
            }
            crate::types::ColumnType::UInt8 => {
                let arr: [u8; 1] = data
                    .try_into()
                    .map_err(|e| HeapError::SerializationError(format!("{:?}", e)))?;
                Ok(Value::UInt8(u8::from_le_bytes(arr)))
            }
            crate::types::ColumnType::UInt16 => {
                let arr: [u8; 2] = data
                    .try_into()
                    .map_err(|e| HeapError::SerializationError(format!("{:?}", e)))?;
                Ok(Value::UInt16(u16::from_le_bytes(arr)))
            }
            crate::types::ColumnType::UInt32 => {
                let arr: [u8; 4] = data
                    .try_into()
                    .map_err(|e| HeapError::SerializationError(format!("{:?}", e)))?;
                Ok(Value::UInt32(u32::from_le_bytes(arr)))
            }
            crate::types::ColumnType::UInt64 => {
                let arr: [u8; 8] = data
                    .try_into()
                    .map_err(|e| HeapError::SerializationError(format!("{:?}", e)))?;
                Ok(Value::UInt64(u64::from_le_bytes(arr)))
            }
            crate::types::ColumnType::Float32 => {
                let arr: [u8; 4] = data
                    .try_into()
                    .map_err(|e| HeapError::SerializationError(format!("{:?}", e)))?;
                Ok(Value::Float32(f32::from_le_bytes(arr)))
            }
            crate::types::ColumnType::Float64 => {
                let arr: [u8; 8] = data
                    .try_into()
                    .map_err(|e| HeapError::SerializationError(format!("{:?}", e)))?;
                Ok(Value::Float64(f64::from_le_bytes(arr)))
            }
            crate::types::ColumnType::Bool => Ok(Value::Boolean(data[0] != 0)),
            crate::types::ColumnType::Varchar(_) | crate::types::ColumnType::Blob(_) => {
                Ok(Value::VarChar(String::from_utf8_lossy(data).to_string()))
            }
        }
    }
}

/// Tuple - in-memory representation of a row
#[derive(Debug, Clone)]
pub struct Tuple {
    values: Vec<Value>,
}

impl Tuple {
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }

    pub fn values(&self) -> &[Value] {
        &self.values
    }

    pub fn get(&self, idx: usize) -> Option<&Value> {
        self.values.get(idx)
    }

    pub fn serialize(&self, columns: &[Column]) -> Vec<u8> {
        let mut result = Vec::new();

        // Build null bitmap
        let mut null_bitmap = vec![0u8; (columns.len() + 7) / 8];
        for (i, val) in self.values.iter().enumerate() {
            if matches!(val, Value::Null) {
                null_bitmap[i / 8] |= 1 << (i % 8);
            }
        }
        result.extend(null_bitmap);

        // Serialize each column value
        for val in &self.values {
            result.extend(val.serialize());
        }

        result
    }

    pub fn deserialize(data: &[u8], columns: &[Column]) -> HeapResult<Self> {
        if data.is_empty() {
            return Ok(Tuple::new(vec![Value::Null; columns.len()]));
        }

        let null_bitmap_size = (columns.len() + 7) / 8;
        let null_bitmap = &data[..null_bitmap_size];

        let mut values = Vec::new();
        let mut offset = null_bitmap_size;

        for (i, col) in columns.iter().enumerate() {
            let is_null = (null_bitmap[i / 8] & (1 << (i % 8))) != 0;

            if is_null {
                values.push(Value::Null);
            } else {
                let size = col.column_type().size();
                if offset + size > data.len() {
                    return Err(HeapError::SerializationError(format!(
                        "Not enough data for column {}",
                        i
                    )));
                }
                let col_data = &data[offset..offset + size];
                values.push(Value::deserialize(col_data, &col.column_type())?);
                offset += size;
            }
        }

        Ok(Tuple::new(values))
    }
}

/// Heap page with slot-based tuple storage
pub struct HeapPage {
    page_id: PageId,
    data: [u8; PAGE_SIZE],
    slot_count: usize,
    upper: usize,
}

impl HeapPage {
    pub fn new(page_id: PageId) -> Self {
        let data = [0u8; PAGE_SIZE];
        let upper = PAGE_SIZE;

        Self {
            page_id,
            data,
            slot_count: 0,
            upper,
        }
    }

    pub fn page_id(&self) -> PageId {
        self.page_id
    }

    pub fn slot_count(&self) -> usize {
        self.slot_count
    }

    pub fn available_space(&self) -> usize {
        let lower = self.slot_count * std::mem::size_of::<SlotEntry>();
        self.upper - lower
    }

    pub fn can_insert(&self, tuple_size: usize) -> bool {
        let slot_size = std::mem::size_of::<SlotEntry>();
        tuple_size + slot_size <= self.available_space()
    }

    pub fn insert_tuple(&mut self, tuple_data: &[u8]) -> HeapResult<usize> {
        if !self.can_insert(tuple_data.len()) {
            return Err(HeapError::OutOfSpace);
        }

        let slot_idx = self.slot_count;

        self.upper -= tuple_data.len();
        let offset = self.upper as i32 - PAGE_SIZE as i32;

        self.data[self.upper..self.upper + tuple_data.len()].copy_from_slice(tuple_data);

        let slot_offset = slot_idx * std::mem::size_of::<SlotEntry>();
        let slot = SlotEntry::new(offset, tuple_data.len() as u32);

        self.data[slot_offset..slot_offset + 4].copy_from_slice(&slot.offset.to_le_bytes());
        self.data[slot_offset + 4..slot_offset + 8].copy_from_slice(&slot.length.to_le_bytes());

        self.slot_count += 1;

        Ok(slot_idx)
    }

    pub fn get_tuple(&self, slot_idx: usize) -> HeapResult<Vec<u8>> {
        if slot_idx >= self.slot_count {
            return Err(HeapError::InvalidSlot(slot_idx));
        }

        let slot_offset = slot_idx * std::mem::size_of::<SlotEntry>();
        let offset_bytes: [u8; 4] = self.data[slot_offset..slot_offset + 4].try_into().unwrap();
        let length_bytes: [u8; 4] = self.data[slot_offset + 4..slot_offset + 8]
            .try_into()
            .unwrap();

        let offset = i32::from_le_bytes(offset_bytes);
        let length = u32::from_le_bytes(length_bytes) as usize;

        if offset == 0 && length == 0 {
            return Err(HeapError::TupleNotFound(RowId::new(self.page_id, slot_idx)));
        }

        let actual_offset = (PAGE_SIZE as i32 + offset) as usize;

        Ok(self.data[actual_offset..actual_offset + length].to_vec())
    }

    pub fn delete_tuple(&mut self, slot_idx: usize) -> HeapResult<()> {
        if slot_idx >= self.slot_count {
            return Err(HeapError::InvalidSlot(slot_idx));
        }

        let slot_offset = slot_idx * std::mem::size_of::<SlotEntry>();
        self.data[slot_offset..slot_offset + 8].fill(0);

        Ok(())
    }

    pub fn iter_tuples(&self) -> impl Iterator<Item = (usize, Vec<u8>)> + '_ {
        (0..self.slot_count).filter_map(move |idx| self.get_tuple(idx).ok().map(|data| (idx, data)))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn from_bytes(page_id: PageId, data: &[u8]) -> Self {
        let mut page = Self::new(page_id);
        page.data.copy_from_slice(data);

        page.slot_count = 0;
        page.upper = PAGE_SIZE;

        for i in 0..(PAGE_SIZE / std::mem::size_of::<SlotEntry>()) {
            let slot_offset = i * std::mem::size_of::<SlotEntry>();
            let length_bytes: [u8; 4] = data[slot_offset + 4..slot_offset + 8].try_into().unwrap();
            let length = u32::from_le_bytes(length_bytes) as usize;
            if length > 0 {
                page.slot_count = i + 1;

                let offset_bytes: [u8; 4] = data[slot_offset..slot_offset + 4].try_into().unwrap();
                let offset = i32::from_le_bytes(offset_bytes);
                let actual = (PAGE_SIZE as i32 + offset) as usize;
                if actual < page.upper {
                    page.upper = actual;
                }
            }
        }

        page
    }
}

/// Heap table - manages heap pages for a Table
pub struct HeapTable {
    table: Arc<Table>,
    pages: HashMap<PageId, HeapPage>,
    first_page_id: PageId,
}

impl HeapTable {
    pub fn new(table: Arc<Table>, first_page_id: PageId) -> Self {
        Self {
            table,
            pages: HashMap::new(),
            first_page_id,
        }
    }

    pub fn table(&self) -> &Arc<Table> {
        &self.table
    }

    pub fn first_page_id(&self) -> PageId {
        self.first_page_id
    }

    fn get_or_create_page(&mut self, page_id: PageId) -> HeapResult<&mut HeapPage> {
        if !self.pages.contains_key(&page_id) {
            self.pages.insert(page_id, HeapPage::new(page_id));
        }
        self.pages
            .get_mut(&page_id)
            .ok_or(HeapError::PageNotFound(page_id))
    }

    pub fn insert(&mut self, values: &[Value]) -> HeapResult<RowId> {
        let columns = self.table.columns();

        let tuple = Tuple::new(values.to_vec());
        let tuple_data = tuple.serialize(columns);

        let mut target_page_id = self.first_page_id;

        for (page_id, page) in self.pages.iter_mut() {
            if page.can_insert(tuple_data.len()) {
                target_page_id = *page_id;
                break;
            }
        }

        let page = self.get_or_create_page(target_page_id)?;

        if page.can_insert(tuple_data.len()) {
            let slot_idx = page.insert_tuple(&tuple_data)?;
            return Ok(RowId::new(target_page_id, slot_idx));
        }

        Err(HeapError::Other(
            "Need to implement page allocation".to_string(),
        ))
    }

    pub fn get(&self, row_id: RowId) -> HeapResult<Tuple> {
        let page = self
            .pages
            .get(&row_id.page_id)
            .ok_or(HeapError::PageNotFound(row_id.page_id))?;

        let data = page.get_tuple(row_id.slot_idx)?;
        let columns = self.table.columns();

        Tuple::deserialize(&data, columns)
    }

    pub fn scan(&self) -> HeapResult<Vec<Tuple>> {
        let columns = self.table.columns();
        let mut results = Vec::new();

        for page in self.pages.values() {
            for (_, data) in page.iter_tuples() {
                match Tuple::deserialize(&data, columns) {
                    Ok(tuple) => results.push(tuple),
                    Err(e) => {
                        eprintln!("Error deserializing tuple: {}", e);
                    }
                }
            }
        }

        Ok(results)
    }

    pub fn update(&mut self, row_id: RowId, values: &[Value]) -> HeapResult<()> {
        {
            let page = self
                .pages
                .get_mut(&row_id.page_id)
                .ok_or(HeapError::PageNotFound(row_id.page_id))?;
            page.delete_tuple(row_id.slot_idx)?;
        }

        self.insert(values)?;

        Ok(())
    }

    pub fn delete(&mut self, row_id: RowId) -> HeapResult<()> {
        let page = self
            .pages
            .get_mut(&row_id.page_id)
            .ok_or(HeapError::PageNotFound(row_id.page_id))?;

        page.delete_tuple(row_id.slot_idx)
    }
}
