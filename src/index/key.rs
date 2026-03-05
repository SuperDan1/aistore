use crate::heap::Value;
use crate::types::ColumnType;
use std::cmp::Ordering;

pub const MAX_KEY_SIZE: usize = 1024;

pub fn serialize_int64(v: i64) -> Vec<u8> {
    (v.wrapping_add(i64::MAX / 2)).to_be_bytes().to_vec()
}

pub fn deserialize_int64(data: &[u8]) -> Option<i64> {
    if data.len() < 8 {
        return None;
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&data[..8]);
    let unsigned = i64::from_be_bytes(bytes);
    Some(unsigned.wrapping_sub(i64::MAX / 2))
}

pub fn compare_int64(a: &[u8], b: &[u8]) -> Ordering {
    let a_val = deserialize_int64(a).unwrap_or(0);
    let b_val = deserialize_int64(b).unwrap_or(0);
    a_val.cmp(&b_val)
}

pub fn serialize_value(value: &Value) -> Option<Vec<u8>> {
    match value {
        Value::Null => Some(vec![0]),
        Value::Int8(v) => {
            let mut bytes = vec![1];
            bytes.extend_from_slice(&v.to_le_bytes());
            Some(bytes)
        }
        Value::Int16(v) => {
            let mut bytes = vec![2];
            bytes.extend_from_slice(&v.to_le_bytes());
            Some(bytes)
        }
        Value::Int32(v) => {
            let mut bytes = vec![3];
            bytes.extend_from_slice(&v.to_le_bytes());
            Some(bytes)
        }
        Value::Int64(v) => {
            let mut bytes = vec![4];
            bytes.extend_from_slice(&v.to_le_bytes());
            Some(bytes)
        }
        Value::UInt8(v) => {
            let mut bytes = vec![5];
            bytes.extend_from_slice(&v.to_le_bytes());
            Some(bytes)
        }
        Value::UInt16(v) => {
            let mut bytes = vec![6];
            bytes.extend_from_slice(&v.to_le_bytes());
            Some(bytes)
        }
        Value::UInt32(v) => {
            let mut bytes = vec![7];
            bytes.extend_from_slice(&v.to_le_bytes());
            Some(bytes)
        }
        Value::UInt64(v) => {
            let mut bytes = vec![8];
            bytes.extend_from_slice(&v.to_le_bytes());
            Some(bytes)
        }
        Value::Float32(v) => {
            let mut bytes = vec![9];
            bytes.extend_from_slice(&v.to_le_bytes());
            Some(bytes)
        }
        Value::Float64(v) => {
            let mut bytes = vec![10];
            bytes.extend_from_slice(&v.to_le_bytes());
            Some(bytes)
        }
        Value::Boolean(b) => {
            let mut bytes = vec![11];
            bytes.push(*b as u8);
            Some(bytes)
        }
        Value::VarChar(s) => {
            let mut bytes = vec![12];
            let len = (s.len() as u32).to_le_bytes();
            bytes.extend_from_slice(&len);
            bytes.extend_from_slice(s.as_bytes());
            Some(bytes)
        }
        Value::Blob(b) => {
            let mut bytes = vec![13];
            let len = (b.len() as u32).to_le_bytes();
            bytes.extend_from_slice(&len);
            bytes.extend_from_slice(b);
            Some(bytes)
        }
    }
}

pub fn compare_keys(a: &[u8], b: &[u8]) -> Ordering {
    let min_len = a.len().min(b.len());
    let cmp = a[..min_len].cmp(&b[..min_len]);
    if cmp != Ordering::Equal {
        return cmp;
    }
    a.len().cmp(&b.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int64_serialization() {
        let original: i64 = 42;
        let serialized = serialize_int64(original);
        assert_eq!(serialized.len(), 8);
        let deserialized = deserialize_int64(&serialized).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_int64_ordering() {
        let neg = serialize_int64(-100);
        let zero = serialize_int64(0);
        let pos = serialize_int64(100);

        assert!(neg < zero);
        assert!(zero < pos);
    }

    #[test]
    fn test_key_comparison() {
        let a = vec![1, 2, 3];
        let b = vec![1, 2, 4];
        let c = vec![1, 2, 3, 4];

        assert!(compare_keys(&a, &b) == Ordering::Less);
        assert!(compare_keys(&b, &a) == Ordering::Greater);
        assert!(compare_keys(&a, &c) == Ordering::Less);
    }
}
