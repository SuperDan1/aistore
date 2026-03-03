// Quick test to verify scan returns correct data
use aistore::storage::StorageEngine;
use aistore::table::Column;
use aistore::types::ColumnType;

fn main() {
    let mut storage = StorageEngine::new("/tmp/test_scan").unwrap();

    // Create table
    storage
        .create_table(
            "test",
            vec![
                Column::new("id".to_string(), ColumnType::Int64, false, 0),
                Column::new("name".to_string(), ColumnType::Varchar(100), false, 1),
            ],
        )
        .unwrap();

    // Insert 3 rows
    let id1 = storage
        .insert(
            "test",
            vec![
                aistore::heap::Value::Int64(1),
                aistore::heap::Value::VarChar("Alice".to_string()),
            ],
        )
        .unwrap();

    let id2 = storage
        .insert(
            "test",
            vec![
                aistore::heap::Value::Int64(2),
                aistore::heap::Value::VarChar("Bob".to_string()),
            ],
        )
        .unwrap();

    let id3 = storage
        .insert(
            "test",
            vec![
                aistore::heap::Value::Int64(3),
                aistore::heap::Value::VarChar("Charlie".to_string()),
            ],
        )
        .unwrap();

    println!("Inserted row IDs: {:?}, {:?}, {:?}", id1, id2, id3);

    // Scan all
    let all = storage.scan("test", None).unwrap();
    println!("\nAll rows: {}", all.len());
    for (i, tuple) in all.iter().enumerate() {
        println!("  Row {}: {:?}", i, tuple.values());
    }

    // Scan with filter
    let filtered = storage
        .scan(
            "test",
            Some(aistore::storage::Filter {
                column: "id".to_string(),
                value: aistore::heap::Value::Int64(2),
            }),
        )
        .unwrap();

    println!("\nFiltered (id=2): {}", filtered.len());
    for tuple in &filtered {
        println!("  {:?}", tuple.values());
    }

    // Test update using returned RowId
    storage
        .update(
            "test",
            id2,
            vec![
                aistore::heap::Value::Int64(2),
                aistore::heap::Value::VarChar("BobUpdated".to_string()),
            ],
        )
        .unwrap();

    let after_update = storage
        .scan(
            "test",
            Some(aistore::storage::Filter {
                column: "id".to_string(),
                value: aistore::heap::Value::Int64(2),
            }),
        )
        .unwrap();

    println!("\nAfter update:");
    for tuple in &after_update {
        println!("  {:?}", tuple.values());
    }

    // Test delete
    storage.delete("test", id3).unwrap();

    let after_delete = storage.scan("test", None).unwrap();
    println!("\nAfter delete: {} rows", after_delete.len());
    for tuple in &after_delete {
        println!("  {:?}", tuple.values());
    }
}
