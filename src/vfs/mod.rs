//! VFS (Virtual File System) module
//! 
//! This module provides a unified interface for file system operations, with a local file system implementation
//! that wraps glibc system calls.

// Re-export error types and result type
pub mod error;
pub use error::{VfsError, VfsResult};

// Re-export interface traits
pub mod interface;
pub use interface::{FileHandle, VfsInterface};

// Re-export local file system implementation
pub mod local_fs;
pub use local_fs::{LocalFileHandle, LocalFs};

#[cfg(test)]
mod tests {
    include!("tests.rs");
}


