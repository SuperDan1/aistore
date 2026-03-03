use aistore::heap::Value;
use aistore::storage::StorageEngine;
use aistore::table::Column;
use aistore::types::ColumnType;

fn main() {
    let mut storage = StorageEngine::new("./test_data").expect("Failed to create storage");

    let columns = vec![
        Column::new("id".to_string(), ColumnType::Int64, false, 0),
        Column::new("k1".to_string(), ColumnType::Int64, false, 1),
        Column::new("k2".to_string(), ColumnType::Int64, false, 2),
        Column::new("k3".to_string(), ColumnType::Int64, false, 3),
    ];

    storage
        .create_table("test", columns)
        .expect("Failed to create table");

    println!("=== Data Consistency Test ===");

    let test_data: Vec<(i64, i64, i64, i64)> =
        (1..=1000).map(|i| (i, i * 1, i * 2, i * 3)).collect();

    println!("Inserting 1000 rows...");
    for (id, k1, k2, k3) in &test_data {
        storage
            .insert(
                "test",
                vec![
                    Value::Int64(*id),
                    Value::Int64(*k1),
                    Value::Int64(*k2),
                    Value::Int64(*k3),
                ],
            )
            .expect("Failed to insert");
    }
    println!("Insert complete.");

    println!("Reading all rows...");
    let results = storage.scan("test", None).expect("Failed to scan");

    println!("Read {} rows", results.len());

    let mut mismatch_count = 0;
    for tuple in results.iter() {
        let id = match tuple.get(0) {
            Some(Value::Int64(v)) => *v,
            _ => {
                mismatch_count += 1;
                continue;
            }
        };
        let expected = test_data.iter().find(|(eid, _, _, _)| *eid == id);
        if let Some((_eid, ek1, ek2, ek3)) = expected {
            let k1 = match tuple.get(1) {
                Some(Value::Int64(v)) => *v,
                _ => {
                    mismatch_count += 1;
                    continue;
                }
            };
            let k2 = match tuple.get(2) {
                Some(Value::Int64(v)) => *v,
                _ => {
                    mismatch_count += 1;
                    continue;
                }
            };
            let k3 = match tuple.get(3) {
                Some(Value::Int64(v)) => *v,
                _ => {
                    mismatch_count += 1;
                    continue;
                }
            };
            if k1 != *ek1 || k2 != *ek2 || k3 != *ek3 {
                println!(
                    "Mismatch at id={}: expected ({},{},{}), got ({},{},{})",
                    id, ek1, ek2, ek3, k1, k2, k3
                );
                mismatch_count += 1;
            }
        }
    }

    println!("\n=== Results ===");
    println!("Total inserted: {}", test_data.len());
    println!("Total read: {}", results.len());
    println!("Mismatches: {}", mismatch_count);

    if mismatch_count == 0 && results.len() == test_data.len() {
        println!("DATA CONSISTENCY VERIFIED!");
    } else {
        println!("DATA INCONSISTENCY DETECTED!");
    }
}
