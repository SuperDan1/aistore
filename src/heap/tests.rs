#![cfg(test)]

use crate::heap::{RowId, Value};
use std::f32::consts::PI;
use std::f64::consts::PI as PI64;

mod value_tests {
    use super::*;

    #[test]
    fn test_value_null() {
        let v = Value::Null;
        assert_eq!(v.serialized_size(), 0);
    }

    #[test]
    fn test_value_int64() {
        let v = Value::Int64(42);
        assert_eq!(v.serialized_size(), 8);
    }

    #[test]
    fn test_value_varchar() {
        let v = Value::VarChar("hello".to_string());
        assert_eq!(v.serialized_size(), 5);
    }

    #[test]
    fn test_value_boolean() {
        let v = Value::Boolean(true);
        assert_eq!(v.serialized_size(), 1);
    }

    #[test]
    fn test_value_float32() {
        let v = Value::Float32(PI);
        assert_eq!(v.serialized_size(), 4);
    }

    #[test]
    fn test_value_float64() {
        let v = Value::Float64(PI64);
        assert_eq!(v.serialized_size(), 8);
    }
}

mod row_id_tests {
    use super::*;

    #[test]
    fn test_row_id_new() {
        let row_id = RowId::new(1, 5);
        assert_eq!(row_id.page_id, 1);
        assert_eq!(row_id.slot_idx, 5);
    }

    #[test]
    fn test_row_id_eq() {
        let row_id1 = RowId::new(1, 5);
        let row_id2 = RowId::new(1, 5);
        let row_id3 = RowId::new(1, 6);

        assert_eq!(row_id1, row_id2);
        assert_ne!(row_id1, row_id3);
    }
}

mod mvcc_tests {
    use super::*;
    use crate::heap::Tuple;
    use crate::types::RowMVCCHeader;

    #[test]
    fn test_tuple_with_mvcc_header() {
        let values = vec![Value::Int64(42), Value::VarChar("test".to_string())];
        let tuple = Tuple::new(values);

        let serialized = tuple.serialize_with_mvcc(&[], None, None);
        assert!(!serialized.is_empty());
    }

    #[test]
    fn test_mvcc_header_from_bytes() {
        let header = RowMVCCHeader::new(1, crate::types::UndoPtr::null(), 100);
        let bytes = header.to_bytes();
        let recovered = RowMVCCHeader::from_bytes(&bytes);

        assert_eq!(header.tx_id_created, recovered.tx_id_created);
    }

    use crate::table::Column;
    use crate::types::ColumnType;

    #[test]
    fn test_tuple_serialize_all_types() {
        let columns = vec![
            Column::new("c1".to_string(), ColumnType::Int64, false, 0),
            Column::new("c2".to_string(), ColumnType::Varchar(100), false, 1),
            Column::new("c3".to_string(), ColumnType::Bool, false, 2),
            Column::new("c4".to_string(), ColumnType::Float32, false, 3),
            Column::new("c5".to_string(), ColumnType::Float64, false, 4),
        ];

        let values = vec![
            Value::Null,
            Value::Int64(42),
            Value::VarChar("hello".to_string()),
            Value::Boolean(true),
            Value::Float32(3.14),
            Value::Float64(3.14159),
        ];

        let tuple = Tuple::new(values);
        let serialized = tuple.serialize(&columns);

        assert!(!serialized.is_empty());
    }

    #[test]
    fn test_value_null_serialization() {
        let v = Value::Null;
        let serialized = v.serialize();
        assert_eq!(serialized.len(), 0);
    }

    #[test]
    fn test_value_round_trip() {
        let original = Value::VarChar("test string".to_string());
        let serialized = original.serialize();
        let deserialized = Value::deserialize(&serialized, &crate::types::ColumnType::Varchar(100));

        assert!(deserialized.is_ok());
    }
}
