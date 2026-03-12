#![cfg(test)]

use crate::heap::Value;
use crate::index::key::{compare_int64, deserialize_int64, serialize_int64, serialize_value};
use std::f64::consts::PI;

mod key_tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn test_serialize_int64_positive() {
        let bytes = serialize_int64(100);
        assert_eq!(bytes.len(), 8);
    }

    #[test]
    fn test_serialize_int64_negative() {
        let bytes = serialize_int64(-100);
        assert_eq!(bytes.len(), 8);
    }

    #[test]
    fn test_serialize_int64_zero() {
        let bytes = serialize_int64(0);
        assert_eq!(bytes.len(), 8);
    }

    #[test]
    fn test_deserialize_int64_roundtrip() {
        let original = 42i64;
        let bytes = serialize_int64(original);
        let recovered = deserialize_int64(&bytes).unwrap();
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_deserialize_int64_negative_roundtrip() {
        let original = -12345i64;
        let bytes = serialize_int64(original);
        let recovered = deserialize_int64(&bytes).unwrap();
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_deserialize_int64_invalid_length() {
        let result = deserialize_int64(&[1, 2, 3]);
        assert!(result.is_none());
    }

    #[test]
    fn test_compare_int64_ordering() {
        let a = serialize_int64(10);
        let b = serialize_int64(20);
        assert_eq!(compare_int64(&a, &b), Ordering::Less);
        assert_eq!(compare_int64(&b, &a), Ordering::Greater);
        assert_eq!(compare_int64(&a, &a), Ordering::Equal);
    }

    #[test]
    fn test_compare_int64_negative() {
        let a = serialize_int64(-50);
        let b = serialize_int64(50);
        assert_eq!(compare_int64(&a, &b), Ordering::Less);
    }
}

mod value_serialization_tests {
    use super::*;

    #[test]
    fn test_serialize_value_int64() {
        let value = Value::Int64(42);
        let bytes = serialize_value(&value);
        assert!(bytes.is_some());
    }

    #[test]
    fn test_serialize_value_varchar() {
        let value = Value::VarChar("hello".to_string());
        let bytes = serialize_value(&value);
        assert!(bytes.is_some());
    }

    #[test]
    fn test_serialize_value_null() {
        let value = Value::Null;
        let bytes = serialize_value(&value);
        assert!(bytes.is_some());
    }

    #[test]
    fn test_serialize_value_boolean() {
        let value = Value::Boolean(true);
        let bytes = serialize_value(&value);
        assert!(bytes.is_some());

        let value2 = Value::Boolean(false);
        let bytes2 = serialize_value(&value2);
        assert!(bytes2.is_some());
    }

    #[test]
    fn test_serialize_value_float() {
        let value = Value::Float64(PI);
        let bytes = serialize_value(&value);
        assert!(bytes.is_some());
    }

    #[test]
    fn test_serialize_value_blob() {
        let value = Value::Blob(vec![1, 2, 3, 4]);
        let bytes = serialize_value(&value);
        assert!(bytes.is_some());
    }
}
