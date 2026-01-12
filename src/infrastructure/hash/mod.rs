// Hash functions for strings

/// Benchmark module for hash algorithm performance comparison
pub mod bench;

/// FNV-1a hash implementation for strings
/// Returns a 64-bit integer hash value
pub fn fnv1a_hash(s: &str) -> u64 {
    // FNV-1a constants
    const FNV_OFFSET_BASIS: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;

    let mut hash = FNV_OFFSET_BASIS;
    
    // Process each byte in the string
    for byte in s.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    
    hash
}

/// djb2 hash implementation for strings
/// Returns a 64-bit integer hash value
pub fn djb2_hash(s: &str) -> u64 {
    // djb2 constants
    const DJB2_MAGIC_NUMBER: u64 = 5381;

    let mut hash = DJB2_MAGIC_NUMBER;
    
    // Process each byte in the string
    for byte in s.as_bytes() {
        // Use wrapping operations to handle overflow safely
        hash = hash.wrapping_shl(5).wrapping_add(hash).wrapping_add(*byte as u64); // hash * 33 + c
    }
    
    hash
}

/// MurmurHash3-128 implementation for strings
/// Returns a 64-bit integer hash value (using the first 64 bits of the 128-bit result)
pub fn murmur3_hash(s: &str) -> u64 {
    // MurmurHash3 constants
    const C1: u64 = 0x87c37b91114253d5;
    const C2: u64 = 0x4cf5ad432745937f;
    const SEED: u64 = 0; // Default seed

    let data = s.as_bytes();
    let len = data.len() as u64;
    let mut hash = SEED;

    // Process 128-bit blocks (16 bytes)
    let mut i = 0;
    while i + 16 <= data.len() {
        // Read 128-bit block as two 64-bit words
        let mut k1 = u64::from_le_bytes([
            data[i], data[i + 1], data[i + 2], data[i + 3],
            data[i + 4], data[i + 5], data[i + 6], data[i + 7],
        ]);
        let mut k2 = u64::from_le_bytes([
            data[i + 8], data[i + 9], data[i + 10], data[i + 11],
            data[i + 12], data[i + 13], data[i + 14], data[i + 15],
        ]);

        // Process k1
        k1 = k1.wrapping_mul(C1);
        k1 = k1.rotate_left(31);
        k1 = k1.wrapping_mul(C2);
        hash ^= k1;
        hash = hash.rotate_left(27);
        hash = hash.wrapping_mul(5).wrapping_add(0x52dce729);

        // Process k2
        k2 = k2.wrapping_mul(C2);
        k2 = k2.rotate_left(33);
        k2 = k2.wrapping_mul(C1);
        hash ^= k2;
        hash = hash.rotate_left(31);
        hash = hash.wrapping_mul(5).wrapping_add(0x38495ab5);

        i += 16;
    }

    // Process remaining bytes
        let mut remaining = data.len() - i;
        if remaining > 0 {
            let mut k1 = 0u64;
            let mut k2 = 0u64;

            // Process k2 (bytes 8-15 if present)
            if remaining > 8 {
                // Set all bytes in k2 for remaining bytes > 8
                for j in 8..remaining {
                    let byte_pos = (j - 8) * 8;
                    k2 ^= (data[i + j] as u64) << byte_pos;
                }
            }

        if remaining > 8 {
            k2 = k2.wrapping_mul(C2);
            k2 = k2.rotate_left(33);
            k2 = k2.wrapping_mul(C1);
            hash ^= k2;
        }

        match remaining {
            8 => k1 ^= u64::from_le_bytes([data[i], data[i+1], data[i+2], data[i+3], data[i+4], data[i+5], data[i+6], data[i+7]]),
            7 => k1 ^= (data[i + 6] as u64) << 48,
            6 => k1 ^= (data[i + 5] as u64) << 40,
            5 => k1 ^= (data[i + 4] as u64) << 32,
            4 => k1 ^= (data[i + 3] as u64) << 24,
            3 => k1 ^= (data[i + 2] as u64) << 16,
            2 => k1 ^= (data[i + 1] as u64) << 8,
            1 => k1 ^= data[i] as u64,
            _ => {}
        }

        if remaining <= 8 {
            k1 = k1.wrapping_mul(C1);
            k1 = k1.rotate_left(31);
            k1 = k1.wrapping_mul(C2);
            hash ^= k1;
        }
    }

    // Final mix
    hash ^= len;
    hash ^= hash.wrapping_shr(33);
    hash = hash.wrapping_mul(0xff51afd7ed558ccd);
    hash ^= hash.wrapping_shr(33);
    hash = hash.wrapping_mul(0xc4ceb9fe1a85ec53);
    hash ^= hash.wrapping_shr(33);

    hash
}

/// XXH64 hash implementation for strings
/// Returns a 64-bit integer hash value
pub fn xxh64_hash(s: &str) -> u64 {
    // XXH64 constants
    const PRIME64_1: u64 = 0x9E3779B185EBCA87;
    const PRIME64_2: u64 = 0xC2B2AE3D27D4EB4F;
    const PRIME64_3: u64 = 0x165667B19E3779F9;
    const PRIME64_4: u64 = 0x85EBCA77C2B2AE63;
    const PRIME64_5: u64 = 0x27D4EB2F165667C5;
    const SEED: u64 = 0; // Default seed

    let data = s.as_bytes();
    let len = data.len() as u64;
    let mut h64 = if len >= 32 {
        let mut v1 = SEED.wrapping_add(PRIME64_1).wrapping_add(PRIME64_2);
        let mut v2 = SEED.wrapping_add(PRIME64_2);
        let mut v3 = SEED;
        let mut v4 = SEED.wrapping_sub(PRIME64_1);

        let mut i = 0;
        while i + 32 <= data.len() {
            // Process 32 bytes (4x8 bytes)
            let block = &data[i..i+32];
            
            // Update v1
            let mut k1 = u64::from_le_bytes([block[0], block[1], block[2], block[3], block[4], block[5], block[6], block[7]]);
            k1 = k1.wrapping_mul(PRIME64_2);
            k1 = k1.rotate_left(31);
            k1 = k1.wrapping_mul(PRIME64_1);
            v1 ^= k1;
            v1 = v1.rotate_left(27);
            v1 = v1.wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4);

            // Update v2
            let mut k2 = u64::from_le_bytes([block[8], block[9], block[10], block[11], block[12], block[13], block[14], block[15]]);
            k2 = k2.wrapping_mul(PRIME64_2);
            k2 = k2.rotate_left(31);
            k2 = k2.wrapping_mul(PRIME64_1);
            v2 ^= k2;
            v2 = v2.rotate_left(27);
            v2 = v2.wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4);

            // Update v3
            let mut k3 = u64::from_le_bytes([block[16], block[17], block[18], block[19], block[20], block[21], block[22], block[23]]);
            k3 = k3.wrapping_mul(PRIME64_2);
            k3 = k3.rotate_left(31);
            k3 = k3.wrapping_mul(PRIME64_1);
            v3 ^= k3;
            v3 = v3.rotate_left(27);
            v3 = v3.wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4);

            // Update v4
            let mut k4 = u64::from_le_bytes([block[24], block[25], block[26], block[27], block[28], block[29], block[30], block[31]]);
            k4 = k4.wrapping_mul(PRIME64_2);
            k4 = k4.rotate_left(31);
            k4 = k4.wrapping_mul(PRIME64_1);
            v4 ^= k4;
            v4 = v4.rotate_left(27);
            v4 = v4.wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4);

            i += 32;
        }

        // Merge all values
        let mut h64 = v1.rotate_left(1).wrapping_add(v2.rotate_left(7)).wrapping_add(v3.rotate_left(12)).wrapping_add(v4.rotate_left(18));

        // Mix v1
        v1 = v1.wrapping_mul(PRIME64_2);
        v1 = v1.rotate_left(31);
        v1 = v1.wrapping_mul(PRIME64_1);
        h64 ^= v1;
        h64 = h64.wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4);

        // Mix v2
        v2 = v2.wrapping_mul(PRIME64_2);
        v2 = v2.rotate_left(31);
        v2 = v2.wrapping_mul(PRIME64_1);
        h64 ^= v2;
        h64 = h64.wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4);

        // Mix v3
        v3 = v3.wrapping_mul(PRIME64_2);
        v3 = v3.rotate_left(31);
        v3 = v3.wrapping_mul(PRIME64_1);
        h64 ^= v3;
        h64 = h64.wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4);

        // Mix v4
        v4 = v4.wrapping_mul(PRIME64_2);
        v4 = v4.rotate_left(31);
        v4 = v4.wrapping_mul(PRIME64_1);
        h64 ^= v4;
        h64 = h64.wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4);

        h64
    } else {
        SEED.wrapping_add(PRIME64_5)
    };

    // Add length
    h64 ^= len;

    // Process remaining bytes
    let mut i = if len >= 32 { len as usize } else { 0 };
    while i + 8 <= data.len() {
        let mut k1 = u64::from_le_bytes([data[i], data[i+1], data[i+2], data[i+3], data[i+4], data[i+5], data[i+6], data[i+7]]);
        k1 = k1.wrapping_mul(PRIME64_2);
        k1 = k1.rotate_left(31);
        k1 = k1.wrapping_mul(PRIME64_1);
        h64 ^= k1;
        h64 = h64.rotate_left(27);
        h64 = h64.wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4);
        i += 8;
    }

    if i + 4 <= data.len() {
        let k1 = u32::from_le_bytes([data[i], data[i+1], data[i+2], data[i+3]]) as u64;
        k1.wrapping_mul(PRIME64_1);
        h64 ^= k1;
        h64 = h64.rotate_left(23);
        h64 = h64.wrapping_mul(PRIME64_2).wrapping_add(PRIME64_3);
        i += 4;
    }

    while i < data.len() {
        let k1 = data[i] as u64;
        k1.wrapping_mul(PRIME64_5);
        h64 ^= k1;
        h64 = h64.rotate_left(11);
        h64 = h64.wrapping_mul(PRIME64_1);
        i += 1;
    }

    // Final mix
    h64 ^= h64.wrapping_shr(33);
    h64 = h64.wrapping_mul(0xFF51AFD7ED558CCD);
    h64 ^= h64.wrapping_shr(33);
    h64 = h64.wrapping_mul(0xC4CEB9FE1A85EC53);
    h64 ^= h64.wrapping_shr(33);

    h64
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
