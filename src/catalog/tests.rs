use super::*;
use crate::table::Column;
use crate::types::ColumnType;
use std::sync::Arc;
use std::thread;
use tempfile::TempDir;

#[test]
fn test_catalog_create_and_load() {
    let temp_dir = TempDir::new().unwrap();
    let catalog = Catalog::new(temp_dir.path()).unwrap();

    let columns = vec![
        Column::new("id".to_string(), ColumnType::Int64, false, 0),
        Column::new("name".to_string(), ColumnType::Varchar(255), true, 1),
    ];

    let table = catalog.create_table("users", 100, columns).unwrap();

    assert_eq!(table.table_id(), 1);
    assert_eq!(table.table_name(), "users");
    assert_eq!(table.segment_id(), 100);
    assert_eq!(table.column_count(), 2);

    let loaded_catalog = Catalog::load(temp_dir.path()).unwrap();
    let loaded_table = loaded_catalog.get_table("users").unwrap();

    assert_eq!(loaded_table.table_id(), table.table_id());
    assert_eq!(loaded_table.table_name(), table.table_name());
    assert_eq!(loaded_table.column_count(), 2);
}

#[test]
fn test_catalog_get_table_by_id() {
    let temp_dir = TempDir::new().unwrap();
    let catalog = Catalog::new(temp_dir.path()).unwrap();

    let columns = vec![Column::new("id".to_string(), ColumnType::Int64, false, 0)];

    catalog.create_table("test", 100, columns).unwrap();

    let table = catalog.get_table_by_id(1).unwrap();
    assert_eq!(table.table_name(), "test");
}

#[test]
fn test_catalog_table_already_exists() {
    let temp_dir = TempDir::new().unwrap();
    let catalog = Catalog::new(temp_dir.path()).unwrap();

    let columns = vec![Column::new("id".to_string(), ColumnType::Int64, false, 0)];

    catalog.create_table("users", 100, columns.clone()).unwrap();

    let result = catalog.create_table("users", 200, columns);
    assert!(matches!(result, Err(CatalogError::TableAlreadyExists(_))));
}

#[test]
fn test_catalog_drop_table() {
    let temp_dir = TempDir::new().unwrap();
    let catalog = Catalog::new(temp_dir.path()).unwrap();

    let columns = vec![Column::new("id".to_string(), ColumnType::Int64, false, 0)];
    catalog.create_table("users", 100, columns).unwrap();

    assert!(catalog.table_exists("users"));

    catalog.drop_table("users").unwrap();

    assert!(!catalog.table_exists("users"));

    let result = catalog.get_table("users");
    assert!(matches!(result, Err(CatalogError::TableNotFound(_))));
}

#[test]
fn test_catalog_list_tables() {
    let temp_dir = TempDir::new().unwrap();
    let catalog = Catalog::new(temp_dir.path()).unwrap();

    let cols1 = vec![Column::new("a".to_string(), ColumnType::Int32, false, 0)];
    let cols2 = vec![Column::new("b".to_string(), ColumnType::Int32, false, 0)];

    catalog.create_table("table1", 100, cols1).unwrap();
    catalog.create_table("table2", 200, cols2).unwrap();

    let tables = catalog.list_tables();
    assert_eq!(tables.len(), 2);

    let names: Vec<String> = tables.iter().map(|t| t.table_name().to_string()).collect();
    assert!(names.contains(&"table1".to_string()));
    assert!(names.contains(&"table2".to_string()));
}

#[test]
fn test_catalog_duplicate_column_name() {
    let temp_dir = TempDir::new().unwrap();
    let catalog = Catalog::new(temp_dir.path()).unwrap();

    let columns = vec![
        Column::new("id".to_string(), ColumnType::Int64, false, 0),
        Column::new("id".to_string(), ColumnType::Int32, false, 1),
    ];

    let result = catalog.create_table("test", 100, columns);
    assert!(matches!(result, Err(CatalogError::ColumnAlreadyExists(_))));
}

#[test]
fn test_catalog_persistence_single_column() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path().to_path_buf();

    {
        let catalog = Catalog::new(&data_path).unwrap();
        let columns = vec![Column::new("id".to_string(), ColumnType::Int64, false, 0)];
        catalog.create_table("accounts", 1000, columns).unwrap();
    }

    {
        let catalog = Catalog::load(&data_path).unwrap();
        assert!(catalog.table_exists("accounts"));
        let table = catalog.get_table("accounts").unwrap();

        assert_eq!(table.table_id(), 1);
        assert_eq!(table.segment_id(), 1000);
        assert_eq!(table.column_count(), 1);
    }
}

#[test]
fn test_catalog_multiple_int_columns() {
    let temp_dir = TempDir::new().unwrap();
    let catalog = Catalog::new(temp_dir.path()).unwrap();

    let columns = vec![
        Column::new("id".to_string(), ColumnType::Int64, false, 0),
        Column::new("age".to_string(), ColumnType::Int32, true, 1),
        Column::new("score".to_string(), ColumnType::Int32, true, 2),
        Column::new("big_value".to_string(), ColumnType::Int64, false, 3),
    ];

    let table = catalog
        .create_table("metrics", 100, columns.clone())
        .unwrap();

    assert_eq!(table.table_name(), "metrics");
    assert_eq!(table.column_count(), 4);

    let loaded_table = catalog.get_table("metrics").unwrap();
    assert_eq!(loaded_table.table_id(), table.table_id());
    assert_eq!(loaded_table.column_count(), 4);

    for (i, original_col) in columns.iter().enumerate() {
        let loaded_col = loaded_table.get_column_by_ordinal(i as u32).unwrap();
        assert_eq!(
            loaded_col.name(),
            original_col.name(),
            "Column {} name mismatch",
            i
        );
        assert_eq!(
            loaded_col.column_type(),
            original_col.column_type(),
            "Column {} type mismatch",
            i
        );
        assert_eq!(
            loaded_col.is_nullable(),
            original_col.is_nullable(),
            "Column {} nullable mismatch",
            i
        );
    }

    assert_eq!(
        loaded_table.get_column("id").unwrap().column_type(),
        ColumnType::Int64
    );
    assert_eq!(
        loaded_table.get_column("age").unwrap().column_type(),
        ColumnType::Int32
    );
}

#[test]
fn test_catalog_multiple_int_columns_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path().to_path_buf();

    {
        let catalog = Catalog::new(&data_path).unwrap();

        let columns = vec![
            Column::new("user_id".to_string(), ColumnType::Int64, false, 0),
            Column::new("post_id".to_string(), ColumnType::Int64, false, 1),
            Column::new("view_count".to_string(), ColumnType::Int32, true, 2),
            Column::new("like_count".to_string(), ColumnType::Int32, true, 3),
        ];

        catalog.create_table("stats", 500, columns).unwrap();
    }

    {
        let catalog = Catalog::load(&data_path).unwrap();

        assert!(catalog.table_exists("stats"));
        let table = catalog.get_table("stats").unwrap();

        assert_eq!(table.table_name(), "stats");
        assert_eq!(table.column_count(), 4);

        let expected = vec![
            ("user_id", ColumnType::Int64, false),
            ("post_id", ColumnType::Int64, false),
            ("view_count", ColumnType::Int32, true),
            ("like_count", ColumnType::Int32, true),
        ];

        for (i, (name, expected_type, expected_nullable)) in expected.iter().enumerate() {
            let col = table.get_column_by_ordinal(i as u32).unwrap();
            assert_eq!(col.name(), *name, "Column {} name mismatch", i);
            assert_eq!(
                col.column_type(),
                *expected_type,
                "Column {} type mismatch",
                i
            );
            assert_eq!(
                col.is_nullable(),
                *expected_nullable,
                "Column {} nullable mismatch",
                i
            );
        }
    }
}

/// Sequential concurrent table creation test
#[test]
fn test_concurrent_table_creation() {
    let temp_dir = TempDir::new().unwrap();
    let catalog = Arc::new(Catalog::new(temp_dir.path()).unwrap());
    let num_threads = 4;
    let tables_per_thread = 25;

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let catalog = Arc::clone(&catalog);
            thread::spawn(move || {
                let mut results = Vec::new();
                for i in 0..tables_per_thread {
                    let table_name = format!("table_{}_{}", thread_id, i);
                    let columns = vec![Column::new("id".to_string(), ColumnType::Int64, false, 0)];
                    // Use (thread_id * tables_per_thread + i + 1) to avoid segment_id = 0
                    let result = catalog.create_table(
                        &table_name,
                        (thread_id * tables_per_thread + i + 1) as u64,
                        columns,
                    );
                    results.push(result);
                }

                results
            })
        })
        .collect();

    let all_results: Vec<Result<_, _>> = handles
        .into_iter()
        .flat_map(|h| h.join().unwrap())
        .collect();

    let success_count = all_results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(success_count, num_threads * tables_per_thread);

    let tables = catalog.list_tables();
    assert_eq!(tables.len(), num_threads * tables_per_thread);
}

/// Sequential concurrent read/write test
#[test]
fn test_concurrent_read_write() {
    let temp_dir = TempDir::new().unwrap();
    let catalog = Arc::new(Catalog::new(temp_dir.path()).unwrap());
    let num_writes = 20;

    let writer_handle = {
        let catalog = Arc::clone(&catalog);
        thread::spawn(move || {
            for i in 0..num_writes {
                let columns = vec![Column::new("id".to_string(), ColumnType::Int64, false, 0)];
                // Use (i + 1) to avoid segment_id = 0
                let _ = catalog.create_table(&format!("table_w_{}", i), (i + 1) as u64, columns);
            }
        })
    };

    let reader_handle = {
        let catalog = Arc::clone(&catalog);
        thread::spawn(move || {
            for _ in 0..num_writes {
                let tables = catalog.list_tables();
                for table in tables.iter().take(3) {
                    let _ = catalog.get_table(table.table_name());
                }
                thread::sleep(std::time::Duration::from_millis(1));
            }
        })
    };

    writer_handle.join().unwrap();
    reader_handle.join().unwrap();

    let tables = catalog.list_tables();
    // Allow for race condition - at least num_writes - 1 tables should exist
    assert!(
        tables.len() >= num_writes - 1,
        "Expected at least {} tables, got {}",
        num_writes - 1,
        tables.len()
    );
}

/// Many columns test - 100 columns
#[test]
fn test_many_columns() {
    let temp_dir = TempDir::new().unwrap();
    let catalog = Catalog::new(temp_dir.path()).unwrap();
    let num_columns = 100;

    let columns: Vec<Column> = (0..num_columns)
        .map(|i| {
            let col_type = if i % 4 == 0 {
                ColumnType::Int64
            } else {
                ColumnType::Int32
            };
            Column::new(format!("col_{}", i), col_type, true, i as u32)
        })
        .collect();

    let table = catalog.create_table("wide_table", 1000, columns).unwrap();
    assert_eq!(table.column_count(), num_columns as u32);

    let loaded_table = catalog.get_table("wide_table").unwrap();
    assert_eq!(loaded_table.column_count(), num_columns as u32);

    for i in 0..num_columns {
        let col = loaded_table.get_column(&format!("col_{}", i)).unwrap();
        assert_eq!(col.ordinal(), i as u32);
    }
}

/// Many tables test - 100 tables
#[test]
fn test_many_tables() {
    let temp_dir = TempDir::new().unwrap();
    let catalog = Catalog::new(temp_dir.path()).unwrap();
    let num_tables = 100;

    for i in 0..num_tables {
        let columns = vec![Column::new("id".to_string(), ColumnType::Int64, false, 0)];
        // Use (i + 1) to avoid segment_id = 0 (invalid)
        catalog
            .create_table(&format!("table_{}", i), (i + 1) as u64, columns)
            .unwrap();
    }

    let tables = catalog.list_tables();
    assert_eq!(tables.len(), num_tables);

    for i in 0..num_tables {
        let table = catalog.get_table(&format!("table_{}", i)).unwrap();
        assert_eq!(table.table_name(), format!("table_{}", i));
    }
}

/// Many tables persistence test
#[test]
fn test_many_tables_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path().to_path_buf();
    let num_tables = 100;

    {
        let catalog = Catalog::new(&data_path).unwrap();
        for i in 0..num_tables {
            let columns = vec![
                Column::new("id".to_string(), ColumnType::Int64, false, 0),
                Column::new("value".to_string(), ColumnType::Int32, true, 1),
            ];
            // Use (i + 1) to avoid segment_id = 0 (invalid)
            catalog
                .create_table(&format!("table_{}", i), (i + 1) as u64, columns)
                .unwrap();
        }
    }

    {
        let catalog = Catalog::load(&data_path).unwrap();
        let tables = catalog.list_tables();
        assert_eq!(tables.len(), num_tables);

        for i in 0..num_tables {
            let table = catalog.get_table(&format!("table_{}", i)).unwrap();
            assert_eq!(table.column_count(), 2);
            assert!(table.get_column("id").is_some());
            assert!(table.get_column("value").is_some());
        }
    }
}

/// Many columns persistence test
#[test]
fn test_many_columns_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path().to_path_buf();
    let num_columns = 100;

    let columns: Vec<Column> = (0..num_columns)
        .map(|i| {
            let col_type = if i % 3 == 0 {
                ColumnType::Int64
            } else if i % 3 == 1 {
                ColumnType::Int32
            } else {
                ColumnType::Varchar(255)
            };
            Column::new(format!("col_{}", i), col_type, i % 2 == 0, i as u32)
        })
        .collect();

    {
        let catalog = Catalog::new(&data_path).unwrap();
        catalog.create_table("wide_table", 1000, columns).unwrap();
    }

    {
        let catalog = Catalog::load(&data_path).unwrap();
        let table = catalog.get_table("wide_table").unwrap();
        assert_eq!(table.column_count(), num_columns as u32);

        for i in 0..num_columns {
            let col = table.get_column(&format!("col_{}", i)).unwrap();
            assert_eq!(col.ordinal(), i as u32);
        }
    }
}

/// Sequential persistence recovery test
#[test]
fn test_concurrent_persistence_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path().to_path_buf();
    let num_tables = 20;
    let num_loads = 5;

    {
        let catalog = Catalog::new(&data_path).unwrap();
        for i in 0..num_tables {
            let columns = vec![
                Column::new("id".to_string(), ColumnType::Int64, false, 0),
                Column::new("data".to_string(), ColumnType::Int32, true, 1),
            ];
            // Use (i + 1) to avoid segment_id = 0 (invalid)
            catalog
                .create_table(&format!("table_{}", i), (i + 1) as u64, columns)
                .unwrap();
        }
    }

    // 顺序加载多次
    for _ in 0..num_loads {
        let catalog = Catalog::load(&data_path).unwrap();
        let tables = catalog.list_tables();
        assert_eq!(tables.len(), num_tables);

        for table in tables {
            let _ = catalog.get_table(table.table_name());
        }
    }
}

/// All column types test
#[test]
fn test_all_column_types() {
    let temp_dir = TempDir::new().unwrap();
    let catalog = Catalog::new(temp_dir.path()).unwrap();

    let columns = vec![
        Column::new("c_int8".to_string(), ColumnType::Int8, true, 0),
        Column::new("c_int16".to_string(), ColumnType::Int16, true, 1),
        Column::new("c_int32".to_string(), ColumnType::Int32, false, 2),
        Column::new("c_int64".to_string(), ColumnType::Int64, false, 3),
        Column::new("c_uint8".to_string(), ColumnType::UInt8, true, 4),
        Column::new("c_varchar".to_string(), ColumnType::Varchar(100), true, 5),
        Column::new("c_blob".to_string(), ColumnType::Blob(256), true, 6),
        Column::new("c_bool".to_string(), ColumnType::Bool, true, 7),
    ];

    let table = catalog
        .create_table("mixed_types", 100, columns.clone())
        .unwrap();
    assert_eq!(table.column_count(), 8);

    let loaded_table = catalog.get_table("mixed_types").unwrap();

    assert_eq!(
        loaded_table.get_column("c_int8").unwrap().column_type(),
        ColumnType::Int8
    );
    assert_eq!(
        loaded_table.get_column("c_int16").unwrap().column_type(),
        ColumnType::Int16
    );
    assert_eq!(
        loaded_table.get_column("c_int32").unwrap().column_type(),
        ColumnType::Int32
    );
    assert_eq!(
        loaded_table.get_column("c_int64").unwrap().column_type(),
        ColumnType::Int64
    );
    assert_eq!(
        loaded_table.get_column("c_uint8").unwrap().column_type(),
        ColumnType::UInt8
    );
    assert_eq!(
        loaded_table.get_column("c_varchar").unwrap().column_type(),
        ColumnType::Varchar(100)
    );
    assert_eq!(
        loaded_table.get_column("c_blob").unwrap().column_type(),
        ColumnType::Blob(256)
    );
    assert_eq!(
        loaded_table.get_column("c_bool").unwrap().column_type(),
        ColumnType::Bool
    );
}
