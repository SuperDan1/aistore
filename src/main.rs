//! Aistore storage engine demo
//! Demonstrates the StorageEngine API

// Use jemalloc as global allocator
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

mod buffer;
mod catalog;
mod controlfile;
mod heap;
mod index;
mod infrastructure;
mod lock;
mod logger;
mod mvcc;
mod page;
mod segment;
mod sql;
mod storage;
mod table;
mod tablespace;
mod types;
mod undo;
mod vfs;
mod wal;

use heap::{RowId, Value};
use storage::StorageEngine;
use table::Column;
use tracing::{error, info, warn};
use types::ColumnType;

fn main() {
    logger::init(None, "info,aistore=debug");

    info!("Aistore Storage Engine Demo");
    info!("==============================");

    // Create storage engine
    let mut engine = StorageEngine::new("./data").expect("Failed to create storage engine");

    // Create table
    let columns = vec![
        Column::new("id".into(), ColumnType::Int64, false, 0),
        Column::new("name".into(), ColumnType::Varchar(255), true, 1),
        Column::new("age".into(), ColumnType::Int32, true, 2),
    ];

    let table_id = engine
        .create_table("users", columns)
        .expect("Failed to create table");
    info!("Created table 'users' with id: {}", table_id);

    // Insert rows
    info!("Inserting rows");

    let row1 = vec![
        Value::Int64(1),
        Value::VarChar("Alice".into()),
        Value::Int32(30),
    ];
    let id1 = engine.insert("users", row1).expect("Failed to insert");
    info!(
        "Inserted row 1: page_id={}, slot_idx={}",
        id1.page_id, id1.slot_idx
    );

    let row2 = vec![
        Value::Int64(2),
        Value::VarChar("Bob".into()),
        Value::Int32(25),
    ];
    let id2 = engine.insert("users", row2).expect("Failed to insert");
    info!(
        "Inserted row 2: page_id={}, slot_idx={}",
        id2.page_id, id2.slot_idx
    );

    let row3 = vec![
        Value::Int64(3),
        Value::VarChar("Charlie".into()),
        Value::Int32(35),
    ];
    let id3 = engine.insert("users", row3).expect("Failed to insert");
    info!(
        "Inserted row 3: page_id={}, slot_idx={}",
        id3.page_id, id3.slot_idx
    );

    // Scan all rows
    info!("Scanning all rows");
    let rows = engine.scan("users", None).expect("Failed to scan");
    info!("Found {} rows:", rows.len());
    for row in &rows {
        let vals: Vec<String> = row.values().iter().map(|v| format!("{:?}", v)).collect();
        info!("  {:?}", vals);
    }

    // Update a row
    info!("Updating row 1");
    let new_row1 = vec![
        Value::Int64(1),
        Value::VarChar("Alice Smith".into()),
        Value::Int32(31),
    ];
    engine
        .update("users", id1, new_row1)
        .expect("Failed to update");
    info!("Updated row 1");

    // Scan again
    info!("Scanning after update");
    let rows = engine.scan("users", None).expect("Failed to scan");
    for row in &rows {
        let vals: Vec<String> = row.values().iter().map(|v| format!("{:?}", v)).collect();
        info!("  {:?}", vals);
    }

    // Delete a row
    info!("Deleting row 2");
    engine.delete("users", id2).expect("Failed to delete");
    info!("Deleted row 2");

    // Final scan
    info!("Final scan");
    let rows = engine.scan("users", None).expect("Failed to scan");
    info!("{} rows remaining:", rows.len());
    for row in &rows {
        let vals: Vec<String> = row.values().iter().map(|v| format!("{:?}", v)).collect();
        info!("  {:?}", vals);
    }

    info!("Demo Complete");
}
