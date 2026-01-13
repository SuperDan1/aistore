// Benchmark for comparing hash algorithm performance and distribution

use std::time::Instant;
use std::collections::HashMap;
use rand::Rng;

use crate::buffer::BufferTag;
use crate::infrastructure::hash::{fnv1a_hash, murmur3_hash, xxh64_hash, cityhash_64_hash, crc32_hash};

/// Generate a specified number of random BufferTag instances
fn generate_random_buffer_tags(count: usize) -> Vec<BufferTag> {
    let mut rng = rand::thread_rng();
    let mut tags = Vec::with_capacity(count);
    
    for _ in 0..count {
        tags.push(BufferTag {
            file_id: rng.gen_range(0..u16::MAX),
            block_id: rng.gen_range(0..u32::MAX),
        });
    }
    
    tags
}

/// Measure hash function performance
/// Returns the time in milliseconds to hash all tags
fn measure_hash_performance<F>(tags: &[BufferTag], hash_func: F) -> u128
where
    F: Fn(&BufferTag) -> u64,
{
    let start = Instant::now();
    
    for tag in tags {
        hash_func(tag);
    }
    
    start.elapsed().as_millis()
}

/// Calculate hash distribution quality
/// Returns a tuple of (average_bucket_size, max_bucket_size, collision_count)
fn calculate_hash_distribution<F>(hash_function: F, tags: &[BufferTag], bucket_count: usize) -> (f64, usize, usize)
where
    F: Fn(&BufferTag) -> u64,
{
    let mut buckets: HashMap<usize, usize> = HashMap::new();
    let mut collision_count = 0;
    
    for tag in tags {
        let hash = hash_function(tag);
        let bucket = (hash as usize) % bucket_count;
        
        let count = buckets.entry(bucket).or_insert(0);
        if *count > 0 {
            collision_count += 1;
        }
        *count += 1;
    }
    
    let average_bucket_size = buckets.values().sum::<usize>() as f64 / bucket_count as f64;
    let max_bucket_size = buckets.values().max().unwrap_or(&0).clone();
    
    (average_bucket_size, max_bucket_size, collision_count)
}

/// Run the hash algorithm benchmark
pub fn run_hash_benchmark() {
    const TAG_COUNT: usize = 10000000;
    const BUCKET_COUNT: usize = 10000000;
    
    println!("\nHash Algorithm Benchmark");
    println!("=========================");
    println!("Generating {} random BufferTag instances...
", TAG_COUNT);
    
    // Generate random tags
    let tags = generate_random_buffer_tags(TAG_COUNT);
    
    // Measure performance
    println!("Performance Test Results (time in ms):");
    println!("----------------------------------------");
    
    // Test BufferTag's built-in hash function
    let time_builtin = measure_hash_performance(&tags, |tag| tag.hash());
    println!("BufferTag's hash: {} ms", time_builtin);
    
    // Test FNV-1a hash function
    let time_fnv1a = measure_hash_performance(&tags, |tag| {
        let s = format!("{}-{}", tag.file_id, tag.block_id);
        fnv1a_hash(&s)
    });
    println!("FNV-1a hash: {} ms", time_fnv1a);
    

    
    // Test MurmurHash3 hash function
    let time_murmur3 = measure_hash_performance(&tags, |tag| {
        let s = format!("{}-{}", tag.file_id, tag.block_id);
        murmur3_hash(&s)
    });
    println!("MurmurHash3 hash: {} ms", time_murmur3);
    
    // Test XXH64 hash function
    let time_xxh64 = measure_hash_performance(&tags, |tag| {
        let s = format!("{}-{}", tag.file_id, tag.block_id);
        xxh64_hash(&s)
    });
    println!("XXH64 hash: {} ms", time_xxh64);

    // Test CityHash64 hash function
    let time_cityhash = measure_hash_performance(&tags, |tag| {
        let s = format!("{}-{}", tag.file_id, tag.block_id);
        cityhash_64_hash(&s)
    });
    println!("CityHash64 hash: {} ms", time_cityhash);

    // Test CRC32 hash function
    let time_crc32 = measure_hash_performance(&tags, |tag| {
        let s = format!("{}-{}", tag.file_id, tag.block_id);
        crc32_hash(&s)
    });
    println!("CRC32 hash: {} ms", time_crc32);
    



    
    println!("\nHash Distribution Analysis ({} buckets):", BUCKET_COUNT);
    println!("----------------------------------------");
    
    // Test BufferTag's built-in hash function
    let (avg_builtin, max_builtin, collisions_builtin) = calculate_hash_distribution(
        |tag| tag.hash(),
        &tags,
        BUCKET_COUNT
    );
    
    // Test FNV-1a hash function
    let (avg_fnv1a, max_fnv1a, collisions_fnv1a) = calculate_hash_distribution(
        |tag| {
            let s = format!("{}-{}", tag.file_id, tag.block_id);
            fnv1a_hash(&s)
        },
        &tags,
        BUCKET_COUNT
    );
    

    
    // Test MurmurHash3 hash function
    let (avg_murmur3, max_murmur3, collisions_murmur3) = calculate_hash_distribution(
        |tag| {
            let s = format!("{}-{}", tag.file_id, tag.block_id);
            murmur3_hash(&s)
        },
        &tags,
        BUCKET_COUNT
    );
    
    // Test XXH64 hash function
    let (avg_xxh64, max_xxh64, collisions_xxh64) = calculate_hash_distribution(
        |tag| {
            let s = format!("{}-{}", tag.file_id, tag.block_id);
            xxh64_hash(&s)
        },
        &tags,
        BUCKET_COUNT
    );

    // Test CityHash64 hash function
    let (avg_cityhash, max_cityhash, collisions_cityhash) = calculate_hash_distribution(
        |tag| {
            let s = format!("{}-{}", tag.file_id, tag.block_id);
            cityhash_64_hash(&s)
        },
        &tags,
        BUCKET_COUNT
    );

    // Test CRC32 hash function
    let (avg_crc32, max_crc32, collisions_crc32) = calculate_hash_distribution(
        |tag| {
            let s = format!("{}-{}", tag.file_id, tag.block_id);
            crc32_hash(&s)
        },
        &tags,
        BUCKET_COUNT
    );
    

    
    // Print distribution results
    let ideal_avg = TAG_COUNT as f64 / BUCKET_COUNT as f64;
    
    println!("BufferTag's hash:");
    println!("  Average bucket size: {:.2} (ideal: {:.2})
  Maximum bucket size: {}
  Collisions: {}
  Collision rate: {:.2}%", 
        avg_builtin, ideal_avg, max_builtin, collisions_builtin, 
        (collisions_builtin as f64 / TAG_COUNT as f64) * 100.0);
    
    println!("\nFNV-1a hash:");
    println!("  Average bucket size: {:.2} (ideal: {:.2})
  Maximum bucket size: {}
  Collisions: {}
  Collision rate: {:.2}%", 
        avg_fnv1a, ideal_avg, max_fnv1a, collisions_fnv1a, 
        (collisions_fnv1a as f64 / TAG_COUNT as f64) * 100.0);
    

    
    println!("\nMurmurHash3 hash:");
    println!("  Average bucket size: {:.2} (ideal: {:.2})
  Maximum bucket size: {}
  Collisions: {}
  Collision rate: {:.2}%", 
        avg_murmur3, ideal_avg, max_murmur3, collisions_murmur3, 
        (collisions_murmur3 as f64 / TAG_COUNT as f64) * 100.0);
    
    println!("\nXXH64 hash:");
    println!("  Average bucket size: {:.2} (ideal: {:.2})
  Maximum bucket size: {}
  Collisions: {}
  Collision rate: {:.2}%", 
        avg_xxh64, ideal_avg, max_xxh64, collisions_xxh64, 
        (collisions_xxh64 as f64 / TAG_COUNT as f64) * 100.0);

    println!("\nCityHash64 hash:");
    println!("  Average bucket size: {:.2} (ideal: {:.2})
  Maximum bucket size: {}
  Collisions: {}
  Collision rate: {:.2}%", 
        avg_cityhash, ideal_avg, max_cityhash, collisions_cityhash, 
        (collisions_cityhash as f64 / TAG_COUNT as f64) * 100.0);

    println!("\nCRC32 hash:");
    println!("  Average bucket size: {:.2} (ideal: {:.2})
  Maximum bucket size: {}
  Collisions: {}
  Collision rate: {:.2}%", 
        avg_crc32, ideal_avg, max_crc32, collisions_crc32, 
        (collisions_crc32 as f64 / TAG_COUNT as f64) * 100.0);
    

    
    println!("\n=========================");
    println!("Benchmark completed successfully!");
}
