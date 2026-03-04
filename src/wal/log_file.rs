//! WAL Log File Management

use crate::vfs::{VfsError, VfsInterface, VfsResult};
use crate::wal::config::WalConfig;
use crate::wal::lsn::LSN;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;

const WAL_MAGIC: u32 = 0x57414C31;
const WAL_VERSION: u32 = 0x00000001;

pub struct LogFile {
    file_id: u16,
    path: PathBuf,
    size: u64,
    vfs: Arc<dyn VfsInterface>,
}

impl LogFile {
    pub fn create(vfs: Arc<dyn VfsInterface>, dir: &PathBuf, file_id: u16) -> VfsResult<Self> {
        let path = dir.join(format!("{:016x}.wal", file_id));

        vfs.create_dir(dir.to_str().unwrap())?;

        let handle = vfs.open_file(path.to_str().unwrap())?;
        let size = 0;

        let mut log_file = Self {
            file_id,
            path,
            size,
            vfs,
        };

        if size == 0 {
            log_file.write_header()?;
        }

        Ok(log_file)
    }

    fn write_header(&mut self) -> VfsResult<()> {
        let mut header = [0u8; 16];
        header[0..4].copy_from_slice(&WAL_MAGIC.to_le_bytes());
        header[4..8].copy_from_slice(&WAL_VERSION.to_le_bytes());

        self.vfs.pwrite(self.path.to_str().unwrap(), &header, 0)?;
        self.size = 16;

        Ok(())
    }

    pub fn read_header(&self) -> VfsResult<(u32, u32)> {
        let mut header = [0u8; 16];
        let n = self
            .vfs
            .pread(self.path.to_str().unwrap(), &mut header, 0)?;

        if n != 16 {
            return Err(VfsError::InvalidArgument("Invalid WAL header".to_string()));
        }

        let magic = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let version = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);

        if magic != WAL_MAGIC {
            return Err(VfsError::InvalidArgument("Invalid WAL magic".to_string()));
        }

        Ok((magic, version))
    }

    pub fn append(&mut self, data: &[u8], offset: u64) -> VfsResult<usize> {
        let handle = self.vfs.open_file(self.path.to_str().unwrap())?;
        let written = handle.pwrite(data, offset)?;
        self.size = offset + written as u64;

        Ok(written)
    }

    pub fn read(&self, offset: u64, len: usize) -> VfsResult<Vec<u8>> {
        let mut buf = vec![0u8; len];
        let handle = self.vfs.open_file(self.path.to_str().unwrap())?;
        let n = handle.pread(&mut buf, offset)?;
        buf.truncate(n);

        Ok(buf)
    }

    pub fn file_id(&self) -> u16 {
        self.file_id
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn sync(&self) -> VfsResult<()> {
        Ok(())
    }
}

pub struct LogFileManager {
    config: WalConfig,
    vfs: Arc<dyn VfsInterface>,
    files: RwLock<Vec<LogFile>>,
    current_file_id: RwLock<u16>,
    current_offset: RwLock<u64>,
}

impl LogFileManager {
    pub fn new(config: WalConfig, vfs: Arc<dyn VfsInterface>) -> VfsResult<Self> {
        let mut manager = Self {
            config,
            vfs,
            files: RwLock::new(Vec::new()),
            current_file_id: RwLock::new(0),
            current_offset: RwLock::new(16),
        };

        manager.init()?;

        Ok(manager)
    }

    fn init(&mut self) -> VfsResult<()> {
        let file = LogFile::create(Arc::clone(&self.vfs), &self.config.log_dir, 0)?;

        *self.current_file_id.write() = 0;
        *self.current_offset.write() = file.size();
        self.files.write().push(file);

        Ok(())
    }

    pub fn append(&self, data: &[u8]) -> VfsResult<LSN> {
        let file_id = *self.current_file_id.read();
        let offset = *self.current_offset.read();

        if offset + data.len() as u64 > self.config.max_file_size {
            return self.rotate_and_append(data);
        }

        let mut files = self.files.write();
        if let Some(file) = files.get_mut(file_id as usize) {
            file.append(data, offset)?;
        }

        let lsn = LSN::new(file_id, offset);
        *self.current_offset.write() = offset + data.len() as u64;

        Ok(lsn)
    }

    fn rotate_and_append(&self, data: &[u8]) -> VfsResult<LSN> {
        let new_file_id = *self.current_file_id.read() + 1;

        let file = LogFile::create(Arc::clone(&self.vfs), &self.config.log_dir, new_file_id)?;

        *self.current_file_id.write() = new_file_id;
        *self.current_offset.write() = 16;

        let lsn = LSN::new(new_file_id, 16);
        let mut files = self.files.write();
        files.push(file);

        if let Some(file) = files.last_mut() {
            file.append(data, 16)?;
        }

        *self.current_offset.write() = 16 + data.len() as u64;

        Ok(lsn)
    }

    pub fn flush(&self) -> VfsResult<()> {
        let files = self.files.read();
        for file in files.iter() {
            file.sync()?;
        }
        Ok(())
    }

    pub fn current_lsn(&self) -> LSN {
        LSN::new(*self.current_file_id.read(), *self.current_offset.read())
    }

    pub fn flushed_lsn(&self) -> LSN {
        self.current_lsn()
    }

    pub fn read_from(&self, lsn: LSN) -> VfsResult<Vec<u8>> {
        let files = self.files.read();

        if let Some(file) = files.get(lsn.file_id() as usize) {
            file.read(lsn.offset(), 1024 * 1024)
        } else {
            Err(VfsError::NotFound("Log file not found".to_string()))
        }
    }

    pub fn list_files(&self) -> Vec<PathBuf> {
        let files = self.files.read();
        files.iter().map(|f| f.path().clone()).collect()
    }

    /// Clean up old log files before checkpoint_lsn
    pub fn cleanup_old_logs(&self, checkpoint_lsn: LSN) -> VfsResult<usize> {
        let mut cleaned = 0;
        let checkpoint_file_id = checkpoint_lsn.file_id();
        let current_file_id = *self.current_file_id.read();

        let mut files = self.files.write();
        let mut to_remove = Vec::new();

        for (idx, file) in files.iter().enumerate() {
            let file_id = file.file_id();
            if file_id < checkpoint_file_id && file_id < current_file_id {
                to_remove.push(idx);
            }
        }

        for idx in to_remove.iter().rev() {
            let file = files.remove(*idx);
            let _ = self.vfs.remove_file(file.path().to_str().unwrap());
            cleaned += 1;
        }

        Ok(cleaned)
    }
}
