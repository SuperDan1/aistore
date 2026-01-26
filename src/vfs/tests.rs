// VFS functionality tests

use super::*;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

// Generate unique test directories to avoid conflicts when running tests in parallel
static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn get_unique_test_dir() -> String {
    let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("/tmp/vfs_test_dir_{}", counter)
}

#[test]
fn test_vfs_directory_operations() {
    let fs = LocalFs::new();
    let test_dir = get_unique_test_dir();
    let test_file = "test_file.txt";
    let test_file_path = &format!("{}/{}", test_dir, test_file);
    
    // Clean up any existing test files/dirs
    let _ = fs.remove_file(test_file_path);
    let _ = fs.remove_dir(&test_dir);
    
    // Create directory
    assert!(fs.create_dir(&test_dir).is_ok());
    
    // Create file in the directory
    let file = fs.create_file(test_file_path).unwrap();
    file.close().unwrap();
    
    // Remove file
    assert!(fs.remove_file(test_file_path).is_ok());
    
    // Remove directory
    assert!(fs.remove_dir(&test_dir).is_ok());
}

#[test]
fn test_vfs_file_operations() {
    let fs = LocalFs::new();
    let test_dir = get_unique_test_dir();
    let test_file = "test_file.txt";
    let test_file_path = &format!("{}/{}", test_dir, test_file);
    
    // Clean up any existing test files/dirs
    let _ = fs.remove_file(test_file_path);
    let _ = fs.remove_dir(&test_dir);
    
    // Create directory and file
    assert!(fs.create_dir(&test_dir).is_ok());
    
    // Create file
    let mut file = fs.create_file(test_file_path).unwrap();
    
    // Write data using file handle
    let write_data = b"Hello, VFS!";
    let write_result = file.write(write_data);
    assert!(write_result.is_ok());
    assert_eq!(write_result.unwrap(), write_data.len());
    
    // Close file
    file.close().unwrap();
    
    // Open file for reading
    let mut file = fs.open_file(test_file_path).unwrap();
    
    // Read data using file handle
    let mut read_data = [0u8; 11];
    let read_result = file.read(&mut read_data);
    assert!(read_result.is_ok());
    assert_eq!(read_result.unwrap(), 11);
    assert_eq!(&read_data, write_data);
    
    // Close file
    file.close().unwrap();
    
    // Clean up
    assert!(fs.remove_file(test_file_path).is_ok());
    assert!(fs.remove_dir(&test_dir).is_ok());
}

#[test]
fn test_vfs_truncate() {
    let fs = LocalFs::new();
    let test_dir = get_unique_test_dir();
    let test_file = "test_file.txt";
    let test_file_path = &format!("{}/{}", test_dir, test_file);
    
    // Clean up any existing test files/dirs
    let _ = fs.remove_file(test_file_path);
    let _ = fs.remove_dir(&test_dir);
    
    // Create directory and file
    assert!(fs.create_dir(&test_dir).is_ok());
    
    // Create file and write some data
    let mut file = fs.create_file(test_file_path).unwrap();
    let write_data = b"Hello, VFS! This is a longer string.";
    assert_eq!(file.write(write_data).unwrap(), write_data.len());
    file.close().unwrap();
    
    // Truncate file to shorter length
    assert!(fs.truncate(test_file_path, 13).is_ok());
    
    // Read truncated data
    let mut file = fs.open_file(test_file_path).unwrap();
    let mut read_data = [0u8; 13];
    let read_len = file.read(&mut read_data).unwrap();
    assert_eq!(read_len, 13);
    assert_eq!(&read_data[..13], b"Hello, VFS! This is a longer string.".split_at(13).0);
    file.close().unwrap();
    
    // Clean up
    assert!(fs.remove_file(test_file_path).is_ok());
    assert!(fs.remove_dir(&test_dir).is_ok());
}

#[test]
fn test_vfs_pread_pwrite() {
    let fs = LocalFs::new();
    let test_dir = get_unique_test_dir();
    let test_file = "test_file.txt";
    let test_file_path = &format!("{}/{}", test_dir, test_file);
    
    // Clean up any existing test files/dirs
    let _ = fs.remove_file(test_file_path);
    let _ = fs.remove_dir(&test_dir);
    
    // Create directory and file
    assert!(fs.create_dir(&test_dir).is_ok());
    assert!(fs.create_file(test_file_path).is_ok());
    
    // Write data at specific offset using pwrite
    let data1 = b"Hello";
    let data2 = b"World";
    assert_eq!(fs.pwrite(test_file_path, data1, 0).unwrap(), data1.len());
    assert_eq!(fs.pwrite(test_file_path, data2, 6).unwrap(), data2.len());
    
    // Read data at specific offsets using pread
    let mut buf1 = [0u8; 5];
    let mut buf2 = [0u8; 5];
    assert_eq!(fs.pread(test_file_path, &mut buf1, 0).unwrap(), buf1.len());
    assert_eq!(fs.pread(test_file_path, &mut buf2, 6).unwrap(), buf2.len());
    
    assert_eq!(&buf1, data1);
    assert_eq!(&buf2, data2);
    
    // Clean up
    assert!(fs.remove_file(test_file_path).is_ok());
    assert!(fs.remove_dir(&test_dir).is_ok());
}

#[test]
fn test_vfs_error_handling() {
    let fs = LocalFs::new();
    let non_existent_dir = get_unique_test_dir();
    let non_existent_file = format!("{}/non_existent_file.txt", non_existent_dir);
    
    // Try to create file in non-existent directory
    let result = fs.create_file(&non_existent_file);
    assert!(result.is_err());
    
    // Try to open non-existent file
    let result = fs.open_file(&non_existent_file);
    assert!(result.is_err());
    
    // Try to read from non-existent file
    let mut buf = [0u8; 10];
    let result = fs.pread(&non_existent_file, &mut buf, 0);
    assert!(result.is_err());
    
    // Try to write to non-existent file
    let result = fs.pwrite(&non_existent_file, b"test", 0);
    assert!(result.is_err());
    
    // Try to truncate non-existent file
    let result = fs.truncate(&non_existent_file, 0);
    assert!(result.is_err());
    
    // Try to remove non-existent file
    let result = fs.remove_file(&non_existent_file);
    assert!(result.is_err());
    
    // Try to remove non-existent directory
    let result = fs.remove_dir(&non_existent_dir);
    assert!(result.is_err());
}