//! VFS error definitions

use std::error::Error;
use std::fmt;

/// VFS error types
#[derive(Debug)]
pub enum VfsError {
    /// Permission denied error
    PermissionDenied(String),
    /// File or directory not found error
    NotFound(String),
    /// File already exists error
    AlreadyExists(String),
    /// Invalid argument error
    InvalidArgument(String),
    /// I/O error
    IoError(std::io::Error),
    /// System call error with error code
    SystemError(i32, String),
}

impl fmt::Display for VfsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VfsError::PermissionDenied(path) => write!(f, "Permission denied: {}", path),
            VfsError::NotFound(path) => write!(f, "File or directory not found: {}", path),
            VfsError::AlreadyExists(path) => {
                write!(f, "File or directory already exists: {}", path)
            }
            VfsError::InvalidArgument(msg) => write!(f, "Invalid argument: {}", msg),
            VfsError::IoError(err) => write!(f, "I/O error: {}", err),
            VfsError::SystemError(errno, msg) => {
                write!(f, "System error (errno {}): {}", errno, msg)
            }
        }
    }
}

impl Error for VfsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            VfsError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for VfsError {
    fn from(err: std::io::Error) -> Self {
        VfsError::IoError(err)
    }
}

impl From<std::ffi::NulError> for VfsError {
    fn from(err: std::ffi::NulError) -> Self {
        VfsError::InvalidArgument(err.to_string())
    }
}

/// Result type for VFS operations
pub type VfsResult<T> = Result<T, VfsError>;
