// ============================================================================
// Tests
// ============================================================================

use super::*;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn get_unique_test_path() -> String {
    let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("/tmp/segment_test_{}.dat", counter)
}

fn cleanup_test_file(path: &str) {
    let _ = fs::remove_file(path);
}

#[test]
fn test_file_header_creation() {
    let mut header = FileHeader::new();

    assert_eq!(header.magic, FILE_MAGIC);
    assert_eq!(header.version, FILE_VERSION);
    assert_eq!(header.file_size, FILE_HEADER_SIZE as u64);
    assert_eq!(header.segment_count, 0);

    header.init_checksum();
    assert!(header.verify_checksum());
}

#[test]
fn test_file_header_checksum() {
    let mut header = FileHeader::new();
    header.init_checksum();

    // Verify checksum
    assert!(header.verify_checksum());

    // Modify header and verify checksum fails
    header.file_size = 9999;
    assert!(!header.verify_checksum());
}

#[test]
fn test_extent_header() {
    let header = ExtentHeader::new();

    assert_eq!(header.next_extent_ptr, 0);

    let checksum = header.compute_checksum();
    assert_ne!(checksum, 0);
}

#[test]
fn test_segment_header() {
    let mut header = SegmentHeader::new(1, SegmentType::Generic);

    assert_eq!(header.segment_id, 1);
    assert_eq!(header.segment_type, SegmentType::Generic);
    assert_eq!(header.next_extent_ptr, 0);
    assert_eq!(header.total_pages, 0);

    header.init_checksum();
    assert!(header.verify_checksum());

    // Modify and verify checksum fails
    header.total_pages = 100;
    assert!(!header.verify_checksum());
}

#[test]
fn test_segment_manager_new_file() {
    let test_path = get_unique_test_path();
    cleanup_test_file(&test_path);

    let manager = SegmentManager::new(&test_path);

    assert!(manager.is_ok());
    let mgr = manager.unwrap();

    // Verify file header
    let header = mgr.cached_file_header();
    assert!(header.is_valid());
    assert_eq!(header.segment_count, 0);
    assert_eq!(header.file_size, FILE_HEADER_SIZE as u64);

    // Cleanup
    cleanup_test_file(&test_path);
}

#[test]
fn test_segment_manager_existing_file() {
    let test_path = get_unique_test_path();
    cleanup_test_file(&test_path);

    // Create file first
    {
        let manager = SegmentManager::new(&test_path);
        assert!(manager.is_ok());
    }

    // Reopen file
    let manager = SegmentManager::new(&test_path);
    assert!(manager.is_ok());
    let mgr = manager.unwrap();

    // Verify file header is loaded correctly
    let header = mgr.cached_file_header();
    assert!(header.is_valid());
    assert_eq!(header.segment_count, 0);

    // Cleanup
    cleanup_test_file(&test_path);
}

#[test]
fn test_create_segment() {
    let test_path = get_unique_test_path();
    cleanup_test_file(&test_path);

    let manager = SegmentManager::new(&test_path);
    assert!(manager.is_ok());

    let mgr = manager.unwrap();
    let segment_id = mgr.create_segment(SegmentType::Generic);

    assert!(segment_id.is_ok());
    assert_eq!(segment_id.unwrap(), 1);

    // Verify file header updated
    let header = mgr.cached_file_header();
    assert_eq!(header.segment_count, 1);

    // Cleanup
    cleanup_test_file(&test_path);
}

#[test]
fn test_allocate_page() {
    let test_path = get_unique_test_path();
    cleanup_test_file(&test_path);

    let manager = SegmentManager::new(&test_path);
    assert!(manager.is_ok());

    let mgr = manager.unwrap();

    // Create a segment
    let segment_id = mgr.create_segment(SegmentType::Generic);
    assert!(segment_id.is_ok());
    let segment_id = segment_id.unwrap();

    // Allocate first page
    let page_idx = mgr.allocate_page(segment_id);
    assert!(page_idx.is_ok());
    assert_eq!(page_idx.unwrap(), 0);

    // Cleanup
    cleanup_test_file(&test_path);
}

#[test]
fn test_extent_size_constants() {
    // Verify extent size is 1MB
    assert_eq!(EXTENT_SIZE, 1024 * 1024);

    // Verify page count
    assert_eq!(EXTENT_PAGE_COUNT, 128);

    // Verify usable pages
    assert_eq!(EXTENT_USABLE_PAGES, 127);

    // Verify page fits in extent
    assert!(EXTENT_USABLE_PAGES as usize * BLOCK_SIZE < EXTENT_SIZE);
}
