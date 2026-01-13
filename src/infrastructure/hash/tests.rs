use super::*;

#[test]
fn test_fnv1a_hash_consistency() {
    // Test that the same string always produces the same hash
    let s = "hello world";
    let hash1 = fnv1a_hash(s);
    let hash2 = fnv1a_hash(s);
    let hash3 = fnv1a_hash(s);
    
    assert_eq!(hash1, hash2);
    assert_eq!(hash2, hash3);
}



#[test]
fn test_hash_string_consistency() {
    // Test that the same string always produces the same hash
    let s = "hello world";
    let hash1 = hash_string(s);
    let hash2 = hash_string(s);
    let hash3 = hash_string(s);
    
    assert_eq!(hash1, hash2);
    assert_eq!(hash2, hash3);
    
    // hash_string should use fnv1a_hash by default
    assert_eq!(hash1, fnv1a_hash(s));
}

#[test]
fn test_hash_different_strings() {
    // Test that different strings produce different hashes
    let s1 = "hello";
    let s2 = "world";
    let s3 = "hello world";
    
    let hash1_fnv = fnv1a_hash(s1);
    let hash2_fnv = fnv1a_hash(s2);
    let hash3_fnv = fnv1a_hash(s3);
    
    // Different strings should have different hashes
    assert_ne!(hash1_fnv, hash2_fnv);
    assert_ne!(hash2_fnv, hash3_fnv);
    assert_ne!(hash1_fnv, hash3_fnv);
}

#[test]
fn test_hash_empty_string() {
    // Test hashing of empty string
    let s = "";
    
    let hash_fnv = fnv1a_hash(s);
    let hash_default = hash_string(s);
    
    // Should always produce the same result for empty string
    assert_eq!(hash_default, hash_fnv);
    
    // Should be consistent
    assert_eq!(hash_fnv, fnv1a_hash(s));
}

#[test]
fn test_hash_long_string() {
    // Test hashing of a long string
    let s = "This is a very long string that should test the hash function's ability to handle longer inputs efficiently.";
    
    let hash_fnv = fnv1a_hash(s);
    let hash_default = hash_string(s);
    
    // Should be consistent
    assert_eq!(hash_fnv, fnv1a_hash(s));
    assert_eq!(hash_default, hash_string(s));
    
    // hash_string should use fnv1a_hash
    assert_eq!(hash_default, hash_fnv);
}

#[test]
fn test_hash_similar_strings() {
    // Test hashing of similar strings
    let s1 = "test string 1";
    let s2 = "test string 2";
    
    let hash1_fnv = fnv1a_hash(s1);
    let hash2_fnv = fnv1a_hash(s2);
    
    let hash1_murmur3 = murmur3_hash(s1);
    let hash2_murmur3 = murmur3_hash(s2);
    
    // Similar strings should have different hashes
    assert_ne!(hash1_fnv, hash2_fnv);
    assert_ne!(hash1_murmur3, hash2_murmur3);
}

#[test]
fn test_murmur3_hash_consistency() {
    // Test that the same string always produces the same hash
    let s = "hello world";
    let hash1 = murmur3_hash(s);
    let hash2 = murmur3_hash(s);
    let hash3 = murmur3_hash(s);
    
    assert_eq!(hash1, hash2);
    assert_eq!(hash2, hash3);
}

#[test]
fn test_murmur3_hash_different_strings() {
    // Test that different strings produce different hashes
    let s1 = "hello";
    let s2 = "world";
    let s3 = "hello world";
    
    let hash1 = murmur3_hash(s1);
    let hash2 = murmur3_hash(s2);
    let hash3 = murmur3_hash(s3);
    
    // Different strings should have different hashes
    assert_ne!(hash1, hash2);
    assert_ne!(hash2, hash3);
    assert_ne!(hash1, hash3);
}

#[test]
fn test_murmur3_hash_empty_string() {
    // Test hashing of empty string
    let s = "";
    
    let hash = murmur3_hash(s);
    
    // Should always produce the same result for empty string
    assert_eq!(hash, murmur3_hash(s));
}

#[test]
fn test_murmur3_hash_long_string() {
    // Test hashing of a long string
    let s = "This is a very long string that should test the hash function's ability to handle longer inputs efficiently.";
    
    let hash = murmur3_hash(s);
    
    // Should be consistent
    assert_eq!(hash, murmur3_hash(s));
}
