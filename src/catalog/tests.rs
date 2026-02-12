use super::*;
use crate::table::Column;
use crate::types::ColumnType;
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

    let table = catalog.create_table("metrics", 100, columns.clone()).unwrap();

    assert_eq!(table.table_name(), "metrics");
    assert_eq!(table.column_count(), 4);

    let loaded_table = catalog.get_table("metrics").unwrap();
    assert_eq!(loaded_table.table_id(), table.table_id());
    assert_eq!(loaded_table.column_count(), 4);

    for (i, original_col) in columns.iter().enumerate() {
        let loaded_col = loaded_table.get_column_by_ordinal(i as u32).unwrap();
        assert_eq!(loaded_col.name(), original_col.name(), "Column {} name mismatch", i);
        assert_eq!(loaded_col.column_type(), original_col.column_type(), "Column {} type mismatch", i);
        assert_eq!(loaded_col.is_nullable(), original_col.is_nullable(), "Column {} nullable mismatch", i);
    }

    assert_eq!(loaded_table.get_column("id").unwrap().column_type(), ColumnType::Int64);
    assert_eq!(loaded_table.get_column("age").unwrap().column_type(), ColumnType::Int32);
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
            assert_eq!(col.column_type(), *expected_type, "Column {} type mismatch", i);
            assert_eq!(col.is_nullable(), *expected_nullable, "Column {} nullable mismatch", i);
        }
    }
}
