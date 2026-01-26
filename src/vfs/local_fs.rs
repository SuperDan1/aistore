//! Local file system implementation using glibc system calls

use crate::vfs::error::{VfsError, VfsResult};
use crate::vfs::interface::{FileHandle, VfsInterface};
use libc::{self, c_int, c_ulong, c_void, mode_t, off_t, size_t};
use std::os::raw::c_char;
use std::path::Path;
use std::ptr;

/// Local file handle implementation
pub struct LocalFileHandle {
    fd: c_int,
}

impl LocalFileHandle {
    /// Create a new LocalFileHandle from a file descriptor
    pub fn new(fd: c_int) -> Self {
        LocalFileHandle { fd }
    }
}

impl FileHandle for LocalFileHandle {
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> {
        let result = unsafe {
            libc::read(
                self.fd,
                buf.as_mut_ptr() as *mut c_void,
                buf.len() as size_t
            )
        };
        
        if result < 0 {
            let errno = unsafe { *libc::__errno_location() };
            Err(VfsError::SystemError(errno, "read failed".to_string()))
        } else {
            Ok(result as usize)
        }
    }
    
    fn write(&mut self, buf: &[u8]) -> VfsResult<usize> {
        let result = unsafe {
            libc::write(
                self.fd,
                buf.as_ptr() as *const c_void,
                buf.len() as size_t
            )
        };
        
        if result < 0 {
            let errno = unsafe { *libc::__errno_location() };
            Err(VfsError::SystemError(errno, "write failed".to_string()))
        } else {
            Ok(result as usize)
        }
    }
    
    fn pread(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let result = unsafe {
            libc::pread(
                self.fd,
                buf.as_mut_ptr() as *mut c_void,
                buf.len() as size_t,
                offset as off_t
            )
        };
        
        if result < 0 {
            let errno = unsafe { *libc::__errno_location() };
            Err(VfsError::SystemError(errno, "pread failed".to_string()))
        } else {
            Ok(result as usize)
        }
    }
    
    fn pwrite(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        let result = unsafe {
            libc::pwrite(
                self.fd,
                buf.as_ptr() as *const c_void,
                buf.len() as size_t,
                offset as off_t
            )
        };
        
        if result < 0 {
            let errno = unsafe { *libc::__errno_location() };
            Err(VfsError::SystemError(errno, "pwrite failed".to_string()))
        } else {
            Ok(result as usize)
        }
    }
    
    fn truncate(&self, length: u64) -> VfsResult<()> {
        let result = unsafe {
            libc::ftruncate(self.fd, length as off_t)
        };
        
        if result < 0 {
            let errno = unsafe { *libc::__errno_location() };
            Err(VfsError::SystemError(errno, "ftruncate failed".to_string()))
        } else {
            Ok(())
        }
    }
    
    fn close(self: Box<Self>) -> VfsResult<()> {
        let result = unsafe {
            libc::close(self.fd)
        };
        
        if result < 0 {
            let errno = unsafe { *libc::__errno_location() };
            Err(VfsError::SystemError(errno, "close failed".to_string()))
        } else {
            Ok(())
        }
    }
}

/// Local file system implementation
pub struct LocalFs {
    // LocalFs doesn't need any state
}

impl LocalFs {
    /// Create a new LocalFs instance
    pub fn new() -> Self {
        LocalFs {}
    }
    
    /// Open a file with the given flags and mode
    fn open_file_internal(&self, path: &str, flags: c_int, mode: mode_t) -> VfsResult<c_int> {
        // Create CString in scope so it lives during the system call
        let c_path = std::ffi::CString::new(path)?;
        
        let result = unsafe {
            libc::open(c_path.as_ptr(), flags, mode)
        };
        
        if result < 0 {
            let errno = unsafe { *libc::__errno_location() };
            match errno {
                libc::ENOENT => {
                    // Check if parent directory exists
                    let parent_path = std::path::Path::new(path).parent().unwrap_or_else(|| std::path::Path::new("."));
                    let parent_str = parent_path.to_str().unwrap();
                    Err(VfsError::SystemError(errno, format!("open failed: file '{}' not found, parent directory: '{}'", path, parent_str)))
                },
                libc::EACCES | libc::EPERM => Err(VfsError::PermissionDenied(path.to_string())),
                _ => Err(VfsError::SystemError(errno, format!("open failed with errno {} for path '{}'", errno, path))),
            }
        } else {
            Ok(result)
        }
    }
}

impl VfsInterface for LocalFs {
    fn create_dir(&self, path: &str) -> VfsResult<()> {
        let c_path = std::ffi::CString::new(path)?;
        
        let result = unsafe {
            libc::mkdir(c_path.as_ptr(), 0o755)
        };
        
        if result < 0 {
            let errno = unsafe { *libc::__errno_location() };
            match errno {
                libc::EEXIST => Err(VfsError::AlreadyExists(path.to_string())),
                libc::EACCES | libc::EPERM => Err(VfsError::PermissionDenied(path.to_string())),
                _ => Err(VfsError::SystemError(errno, "mkdir failed".to_string())),
            }
        } else {
            Ok(())
        }
    }
    
    fn remove_dir(&self, path: &str) -> VfsResult<()> {
        let c_path = std::ffi::CString::new(path)?;
        
        let result = unsafe {
            libc::rmdir(c_path.as_ptr())
        };
        
        if result < 0 {
            let errno = unsafe { *libc::__errno_location() };
            match errno {
                libc::ENOENT => Err(VfsError::NotFound(path.to_string())),
                libc::EACCES | libc::EPERM => Err(VfsError::PermissionDenied(path.to_string())),
                _ => Err(VfsError::SystemError(errno, "rmdir failed".to_string())),
            }
        } else {
            Ok(())
        }
    }
    
    fn create_file(&self, path: &str) -> VfsResult<Box<dyn FileHandle>> {
        let flags = libc::O_CREAT | libc::O_RDWR | libc::O_TRUNC;
        let mode = 0o644;
        
        let fd = self.open_file_internal(path, flags, mode)?;
        Ok(Box::new(LocalFileHandle::new(fd)))
    }
    
    fn open_file(&self, path: &str) -> VfsResult<Box<dyn FileHandle>> {
        let flags = libc::O_RDWR;
        let mode = 0;
        
        let fd = self.open_file_internal(path, flags, mode)?;
        Ok(Box::new(LocalFileHandle::new(fd)))
    }
    
    fn remove_file(&self, path: &str) -> VfsResult<()> {
        let c_path = std::ffi::CString::new(path)?;
        
        let result = unsafe {
            libc::unlink(c_path.as_ptr())
        };
        
        if result < 0 {
            let errno = unsafe { *libc::__errno_location() };
            match errno {
                libc::ENOENT => Err(VfsError::NotFound(path.to_string())),
                libc::EACCES | libc::EPERM => Err(VfsError::PermissionDenied(path.to_string())),
                _ => Err(VfsError::SystemError(errno, "unlink failed".to_string())),
            }
        } else {
            Ok(())
        }
    }
    
    fn truncate(&self, path: &str, length: u64) -> VfsResult<()> {
        let c_path = std::ffi::CString::new(path)?;
        
        let result = unsafe {
            libc::truncate(c_path.as_ptr(), length as off_t)
        };
        
        if result < 0 {
            let errno = unsafe { *libc::__errno_location() };
            match errno {
                libc::ENOENT => Err(VfsError::NotFound(path.to_string())),
                libc::EACCES | libc::EPERM => Err(VfsError::PermissionDenied(path.to_string())),
                _ => Err(VfsError::SystemError(errno, "truncate failed".to_string())),
            }
        } else {
            Ok(())
        }
    }
    
    fn pread(&self, path: &str, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let fd = self.open_file_internal(path, libc::O_RDONLY, 0)?;
        
        let result = unsafe {
            libc::pread(
                fd,
                buf.as_mut_ptr() as *mut c_void,
                buf.len() as size_t,
                offset as off_t
            )
        };
        
        // Close the file descriptor regardless of result
        let _ = unsafe { libc::close(fd) };
        
        if result < 0 {
            let errno = unsafe { *libc::__errno_location() };
            Err(VfsError::SystemError(errno, "pread failed".to_string()))
        } else {
            Ok(result as usize)
        }
    }
    
    fn pwrite(&self, path: &str, buf: &[u8], offset: u64) -> VfsResult<usize> {
        let fd = self.open_file_internal(path, libc::O_WRONLY, 0)?;
        
        let result = unsafe {
            libc::pwrite(
                fd,
                buf.as_ptr() as *const c_void,
                buf.len() as size_t,
                offset as off_t
            )
        };
        
        // Close the file descriptor regardless of result
        let _ = unsafe { libc::close(fd) };
        
        if result < 0 {
            let errno = unsafe { *libc::__errno_location() };
            Err(VfsError::SystemError(errno, "pwrite failed".to_string()))
        } else {
            Ok(result as usize)
        }
    }
}
