#![cfg(test)]

use crate::heap::{RowId, Value};

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
        let v = Value::Float32(3.14);
        assert_eq!(v.serialized_size(), 4);
    }

    #[test]
    fn test_value_float64() {
        let v = Value::Float64(3.14159);
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
