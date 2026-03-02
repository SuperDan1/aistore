//! Aistore main program entry
//! Simple demo to test end-to-end SQL execution

// Use jemalloc as global allocator
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

mod buffer;
mod catalog;
mod controlfile;
mod executor;
mod heap;
mod index;
mod infrastructure;
mod lock;
mod page;
mod segment;
mod sql;
mod table;
mod tablespace;
mod types;
mod vfs;

use buffer::BufferMgr;
use catalog::Catalog;
use executor::Executor;
use std::path::PathBuf;
use std::sync::Arc;
use vfs::LocalFs;

fn main() {
    println!("Aistore storage engine starting...");

    // Setup: Create data directory
    let data_dir = PathBuf::from("./data");
    std::fs::create_dir_all(&data_dir).expect("Failed to create data dir");

    // 1. Create VFS
    let vfs = Arc::new(LocalFs::new());

    // 2. Create large BufferPool (no eviction needed for demo)
    // 10000 pages * 8KB = 80MB buffer pool
    let buffer_mgr = Arc::new(BufferMgr::init(10000, vfs.clone(), data_dir.clone()));

    // 3. Create Catalog
    let catalog = Arc::new(Catalog::new(&data_dir).expect("Failed to create catalog"));

    // 4. Create Executor with BufferPool
    let mut executor = Executor::new(catalog, buffer_mgr);

    // 5. Execute SQL statements
    println!("\n=== SQL Execution Demo ===\n");

    // CREATE TABLE
    let result = executor.execute("CREATE TABLE users (id INT, name VARCHAR, age INT)");
    println!("CREATE TABLE: {:?}", result);

    // INSERT
    let result = executor.execute("INSERT INTO users VALUES (1, 'Alice', 30)");
    println!("INSERT: {:?}", result);

    let result = executor.execute("INSERT INTO users VALUES (2, 'Bob', 25)");
    println!("INSERT: {:?}", result);

    let result = executor.execute("INSERT INTO users VALUES (3, 'Charlie', 35)");
    println!("INSERT: {:?}", result);

    // SELECT
    let result = executor.execute("SELECT name, age FROM users");
    println!("\nSELECT all:");
    println!("{}", result.unwrap_or_else(|e| format!("Error: {:?}", e)));

    // SELECT with WHERE
    let result = executor.execute("SELECT name, age FROM users WHERE age > 28");
    println!("\nSELECT WHERE age > 28:");
    println!("{}", result.unwrap_or_else(|e| format!("Error: {:?}", e)));

    // UPDATE
    let result = executor.execute("UPDATE users SET age = 31 WHERE name = 'Alice'");
    println!("\nUPDATE: {:?}", result);

    // SELECT after UPDATE
    let result = executor.execute("SELECT name, age FROM users WHERE name = 'Alice'");
    println!("\nSELECT after UPDATE:");
    println!("{}", result.unwrap_or_else(|e| format!("Error: {:?}", e)));

    // DELETE
    let result = executor.execute("DELETE FROM users WHERE name = 'Bob'");
    println!("\nDELETE: {:?}", result);

    // SELECT after DELETE
    let result = executor.execute("SELECT name, age FROM users");
    println!("\nSELECT after DELETE:");
    println!("{}", result.unwrap_or_else(|e| format!("Error: {:?}", e)));

    println!("\n=== Demo Complete ===");
}
