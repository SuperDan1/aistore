use crate::types::TransactionId;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use super::error::{UndoError, UndoResult};

pub(crate) struct UndoSegment {
    segment_id: u32,
    file: File,
    file_size: usize,
    capacity: usize,
    current_offset: usize,
}

impl UndoSegment {
    pub fn create(path: &PathBuf, segment_id: u32, capacity: usize) -> UndoResult<Self> {
        let file_name = format!("undo_{:04}.seg", segment_id);
        let file_path = path.join(&file_name);

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(&file_path)?;

        let metadata = file.metadata()?;
        let file_size = metadata.len() as usize;

        Ok(Self {
            segment_id,
            file,
            file_size,
            capacity,
            current_offset: file_size,
        })
    }

    pub fn open(path: &PathBuf, segment_id: u32) -> UndoResult<Self> {
        let file_name = format!("undo_{:04}.seg", segment_id);
        let file_path = path.join(&file_name);

        let file = OpenOptions::new().write(true).read(true).open(&file_path)?;

        let metadata = file.metadata()?;
        let file_size = metadata.len() as usize;

        Ok(Self {
            segment_id,
            file,
            file_size,
            capacity: 64 * 1024 * 1024,
            current_offset: file_size,
        })
    }

    pub fn segment_id(&self) -> u32 {
        self.segment_id
    }

    pub fn append(&mut self, data: &[u8]) -> UndoResult<u16> {
        if self.current_offset + data.len() > self.capacity {
            return Err(UndoError::SegmentFull);
        }

        let offset = self.current_offset as u16;

        self.file
            .seek(SeekFrom::Start(self.current_offset as u64))?;
        self.file.write_all(data)?;
        self.file.flush()?;

        self.current_offset += data.len();

        Ok(offset)
    }

    pub fn read_record(&mut self, offset: usize) -> UndoResult<Vec<u8>> {
        if offset + 56 > self.current_offset {
            return Err(UndoError::OffsetOutOfBounds);
        }

        let mut header_buf = vec![0u8; 56];
        self.file.seek(SeekFrom::Start(offset as u64))?;
        self.file.read_exact(&mut header_buf)?;

        let length = u32::from_le_bytes(header_buf[0..4].try_into().unwrap()) as usize;

        let total_len = 56 + (length - 56);
        if offset + total_len > self.current_offset {
            return Err(UndoError::OffsetOutOfBounds);
        }

        let mut full_data = header_buf;
        if length > 56 {
            let mut rest = vec![0u8; length - 56];
            self.file.read_exact(&mut rest)?;
            full_data.extend(rest);
        }

        Ok(full_data)
    }

    pub fn purge_old_records(&self, _oldest_active_tx: TransactionId) -> UndoResult<usize> {
        Ok(0)
    }

    pub fn current_size(&self) -> usize {
        self.current_offset
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}
