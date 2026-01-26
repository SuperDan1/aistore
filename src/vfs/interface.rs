//! VFS interface definitions

use crate::vfs::error::VfsResult;

/// File handle trait for VFS operations
/// This trait represents a handle to an open file and provides methods for reading and writing
pub trait FileHandle: Send + Sync {
    /// Read from the file at the current offset
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize>;
    
    /// Write to the file at the current offset
    fn write(&mut self, buf: &[u8]) -> VfsResult<usize>;
    
    /// Read from the file at a specific offset
    fn pread(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize>;
    
    /// Write to the file at a specific offset
    fn pwrite(&self, buf: &[u8], offset: u64) -> VfsResult<usize>;
    
    /// Resize the file to the specified length
    fn truncate(&self, length: u64) -> VfsResult<()>;
    
    /// Close the file handle
    fn close(self: Box<Self>) -> VfsResult<()>;
}

/// VFS interface trait
/// This trait defines the interface for all VFS implementations
pub trait VfsInterface {
    /// Create a new directory
    /// 
    /// # Arguments
    /// * `path` - The path to the directory to create
    /// 
    /// # Returns
    /// * `Ok(())` if the directory was created successfully
    /// * `Err(VfsError)` if an error occurred
    fn create_dir(&self, path: &str) -> VfsResult<()>;
    
    /// Remove an existing directory
    /// 
    /// # Arguments
    /// * `path` - The path to the directory to remove
    /// 
    /// # Returns
    /// * `Ok(())` if the directory was removed successfully
    /// * `Err(VfsError)` if an error occurred
    fn remove_dir(&self, path: &str) -> VfsResult<()>;
    
    /// Create a new file and return a handle to it
    /// 
    /// # Arguments
    /// * `path` - The path to the file to create
    /// 
    /// # Returns
    /// * `Ok(Box<dyn FileHandle>)` if the file was created successfully
    /// * `Err(VfsError)` if an error occurred
    fn create_file(&self, path: &str) -> VfsResult<Box<dyn FileHandle>>;
    
    /// Open an existing file and return a handle to it
    /// 
    /// # Arguments
    /// * `path` - The path to the file to open
    /// 
    /// # Returns
    /// * `Ok(Box<dyn FileHandle>)` if the file was opened successfully
    /// * `Err(VfsError)` if an error occurred
    fn open_file(&self, path: &str) -> VfsResult<Box<dyn FileHandle>>;
    
    /// Remove an existing file
    /// 
    /// # Arguments
    /// * `path` - The path to the file to remove
    /// 
    /// # Returns
    /// * `Ok(())` if the file was removed successfully
    /// * `Err(VfsError)` if an error occurred
    fn remove_file(&self, path: &str) -> VfsResult<()>;
    
    /// Resize a file to the specified length
    /// 
    /// # Arguments
    /// * `path` - The path to the file to resize
    /// * `length` - The new length of the file
    /// 
    /// # Returns
    /// * `Ok(())` if the file was resized successfully
    /// * `Err(VfsError)` if an error occurred
    fn truncate(&self, path: &str, length: u64) -> VfsResult<()>;
    
    /// Read from a file at a specific offset
    /// 
    /// # Arguments
    /// * `path` - The path to the file to read from
    /// * `buf` - The buffer to read into
    /// * `offset` - The offset in the file to start reading from
    /// 
    /// # Returns
    /// * `Ok(usize)` - The number of bytes read
    /// * `Err(VfsError)` if an error occurred
    fn pread(&self, path: &str, buf: &mut [u8], offset: u64) -> VfsResult<usize>;
    
    /// Write to a file at a specific offset
    /// 
    /// # Arguments
    /// * `path` - The path to the file to write to
    /// * `buf` - The buffer to write from
    /// * `offset` - The offset in the file to start writing to
    /// 
    /// # Returns
    /// * `Ok(usize)` - The number of bytes written
    /// * `Err(VfsError)` if an error occurred
    fn pwrite(&self, path: &str, buf: &[u8], offset: u64) -> VfsResult<usize>;
}
