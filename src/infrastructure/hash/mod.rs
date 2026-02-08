// Hash functions for strings

use crc32fast;
use xxhash_rust::xxh64;

/// FNV-1a hash implementation for strings
/// Returns a 64-bit integer hash value using the simplehash library
pub fn fnv1a_hash(s: &str) -> u64 {
    simplehash::fnv1a_64(s.as_bytes())
}

/// MurmurHash3 implementation for strings
/// Returns a 64-bit integer hash value using the simplehash library
pub fn murmur3_hash(s: &str) -> u64 {
    simplehash::murmurhash3_128(s.as_bytes(), 0) as u64
}

/// XXH64 hash implementation for strings
/// Returns a 64-bit integer hash value using the xxhash-rust library
pub fn xxh64_hash(s: &str) -> u64 {
    xxh64::xxh64(s.as_bytes(), 0)
}

/// CityHash64 implementation for strings
/// Returns a 64-bit integer hash value using the simplehash library
pub fn cityhash_64_hash(s: &str) -> u64 {
    simplehash::city_hash_64(s.as_bytes())
}

/// CRC32 implementation for strings
/// Returns a 64-bit integer hash value using the crc32fast library
pub fn crc32_hash(s: &str) -> u64 {
    crc32fast::hash(s.as_bytes()) as u64
}

/// Hash a string and return an integer
/// Uses FNV-1a as the default hash algorithm
pub fn hash_string(s: &str) -> u64 {
    fnv1a_hash(s)
}

#[cfg(test)]
mod tests {
    include!("tests.rs");
}
