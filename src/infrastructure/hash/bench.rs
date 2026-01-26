use criterion::{black_box, criterion_group, criterion_main, Criterion};  
use rand::Rng;

// Reference the main crate
extern crate aistore;

// Import the hash functions from the main crate
use aistore::infrastructure::hash::{fnv1a_hash, murmur3_hash, xxh64_hash, cityhash_64_hash, crc32_hash};

// Generate a random string of specified length
fn generate_random_string(length: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    let mut s = String::with_capacity(length);
    
    for _ in 0..length {
        let idx = rng.gen_range(0..CHARSET.len());
        s.push(CHARSET[idx] as char);
    }
    
    s
}



// Benchmark hash functions with short strings
pub fn bench_short_strings(c: &mut Criterion) {
    let s = generate_random_string(10);
    
    let mut group = c.benchmark_group("ShortStrings");
    
    group.bench_function("fnv1a_hash", |b: &mut criterion::Bencher| b.iter(|| fnv1a_hash(black_box(&s))));
    group.bench_function("murmur3_hash", |b: &mut criterion::Bencher| b.iter(|| murmur3_hash(black_box(&s))));
    group.bench_function("xxh64_hash", |b: &mut criterion::Bencher| b.iter(|| xxh64_hash(black_box(&s))));
    group.bench_function("cityhash_64_hash", |b: &mut criterion::Bencher| b.iter(|| cityhash_64_hash(black_box(&s))));
    group.bench_function("crc32_hash", |b: &mut criterion::Bencher| b.iter(|| crc32_hash(black_box(&s))));
    
    group.finish();
}

// Benchmark hash functions with medium strings
pub fn bench_medium_strings(c: &mut Criterion) {
    let s = generate_random_string(100);
    
    let mut group = c.benchmark_group("MediumStrings");
    
    group.bench_function("fnv1a_hash", |b: &mut criterion::Bencher| b.iter(|| fnv1a_hash(black_box(&s))));
    group.bench_function("murmur3_hash", |b: &mut criterion::Bencher| b.iter(|| murmur3_hash(black_box(&s))));
    group.bench_function("xxh64_hash", |b: &mut criterion::Bencher| b.iter(|| xxh64_hash(black_box(&s))));
    group.bench_function("cityhash_64_hash", |b: &mut criterion::Bencher| b.iter(|| cityhash_64_hash(black_box(&s))));
    group.bench_function("crc32_hash", |b: &mut criterion::Bencher| b.iter(|| crc32_hash(black_box(&s))));
    
    group.finish();
}

// Benchmark hash functions with long strings
pub fn bench_long_strings(c: &mut Criterion) {
    let s = generate_random_string(1000);
    
    let mut group = c.benchmark_group("LongStrings");
    
    group.bench_function("fnv1a_hash", |b: &mut criterion::Bencher| b.iter(|| fnv1a_hash(black_box(&s))));
    group.bench_function("murmur3_hash", |b: &mut criterion::Bencher| b.iter(|| murmur3_hash(black_box(&s))));
    group.bench_function("xxh64_hash", |b: &mut criterion::Bencher| b.iter(|| xxh64_hash(black_box(&s))));
    group.bench_function("cityhash_64_hash", |b: &mut criterion::Bencher| b.iter(|| cityhash_64_hash(black_box(&s))));
    group.bench_function("crc32_hash", |b: &mut criterion::Bencher| b.iter(|| crc32_hash(black_box(&s))));
    
    group.finish();
}

// Benchmark hash functions with formatted strings
pub fn bench_formatted_strings(c: &mut Criterion) {
    let count = 1000;
    
    let mut group = c.benchmark_group("FormattedStrings");
    
    group.bench_function("fnv1a_hash", |b: &mut criterion::Bencher| b.iter(|| {
        let mut rng = rand::thread_rng();
        for _ in 0..count {
            let file_id = rng.gen_range(0..u16::MAX);
            let block_id = rng.gen_range(0..u32::MAX);
            let s = format!("{}-{}", file_id, block_id);
            fnv1a_hash(&s);
        }
    }));
    
    group.bench_function("murmur3_hash", |b: &mut criterion::Bencher| b.iter(|| {
        let mut rng = rand::thread_rng();
        for _ in 0..count {
            let file_id = rng.gen_range(0..u16::MAX);
            let block_id = rng.gen_range(0..u32::MAX);
            let s = format!("{}-{}", file_id, block_id);
            murmur3_hash(&s);
        }
    }));
    
    group.bench_function("xxh64_hash", |b: &mut criterion::Bencher| b.iter(|| {
        let mut rng = rand::thread_rng();
        for _ in 0..count {
            let file_id = rng.gen_range(0..u16::MAX);
            let block_id = rng.gen_range(0..u32::MAX);
            let s = format!("{}-{}", file_id, block_id);
            xxh64_hash(&s);
        }
    }));
    
    group.bench_function("cityhash_64_hash", |b: &mut criterion::Bencher| b.iter(|| {
        let mut rng = rand::thread_rng();
        for _ in 0..count {
            let file_id = rng.gen_range(0..u16::MAX);
            let block_id = rng.gen_range(0..u32::MAX);
            let s = format!("{}-{}", file_id, block_id);
            cityhash_64_hash(&s);
        }
    }));
    
    group.bench_function("crc32_hash", |b: &mut criterion::Bencher| b.iter(|| {
        let mut rng = rand::thread_rng();
        for _ in 0..count {
            let file_id = rng.gen_range(0..u16::MAX);
            let block_id = rng.gen_range(0..u32::MAX);
            let s = format!("{}-{}", file_id, block_id);
            crc32_hash(&s);
        }
    }));
    
    group.finish();
}

// Export the benchmark group for criterion
criterion_group!(benches, bench_short_strings, bench_medium_strings, bench_long_strings, bench_formatted_strings);

// Only run the benchmark group when this file is executed directly
criterion_main!(benches);
