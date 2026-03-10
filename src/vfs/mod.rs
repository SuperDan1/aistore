//! VFS (Virtual File System) module
//!
//! This module provides a unified interface for file system operations, with a local file system implementation
//! that wraps glibc system calls.

// Re-export error types and result type
pub(crate) mod error;
pub(crate) use error::VfsError;
pub(crate) use error::VfsResult;

// Re-export interface traits
pub(crate) mod interface;
pub(crate) use interface::VfsInterface;

#[cfg(test)]
pub use interface::FileHandle;

// Re-export local file system implementation
pub(crate) mod local_fs;
pub(crate) use local_fs::LocalFs;

#[cfg(test)]
mod tests {
    include!("tests.rs");
}
