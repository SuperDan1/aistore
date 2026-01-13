# Hash Algorithm Comparison

This directory contains implementations of various hash algorithms and their performance comparison.

## Implemented Algorithms

1. **BufferTag Built-in Hash** - Simple combination of file_id and block_id
2. **XXH64** - High-performance hash algorithm
3. **MurmurHash3** - Modern, fast non-cryptographic hash
4. **FNV-1a** - Simple, good distribution hash
5. **CityHash64** - Google's fast, high-quality non-cryptographic hash
6. **CRC32** - Fast, SIMD-accelerated cyclic redundancy check algorithm

## Performance Test Results

Performance test conducted with 1,000,000 random BufferTag instances:

### Execution Time (milliseconds)
```
┌──────────────────────────────────────────────────
│ Hash Algorithm Performance (1,000,000 tags)
├─────────────────────────┬───────────────────────
│ Algorithm               │ Time (ms)   │
├─────────────────────────┼───────────────────────
│ BufferTag Built-in      │ 0         │ 
│ XXH64                   │ 56        │ ████████████████████████████████
│ MurmurHash3             │ 59        │ █████████████████████████████████
│ FNV-1a                  │ 54        │ ███████████████████████████████
│ CityHash64              │ 54        │ ███████████████████████████████
│ CRC32                   │ 76        │ █████████████████████████████████████████
└─────────────────────────┴───────────────────────
```

## Hash Distribution Analysis

Distribution analysis with 1,000,000 buckets (1 tag per bucket on average):

| Algorithm | Avg Bucket Size | Max Bucket Size | Collision Rate |
|-----------|-----------------|-----------------|----------------|
| BufferTag Built-in | 1.00 | 9 | 36.81% |
| XXH64 | 1.00 | 8 | 36.83% |
| MurmurHash3 | 1.00 | 9 | 36.80% |
| FNV-1a | 1.00 | 9 | 36.76% |
| CityHash64 | 1.00 | 10 | 36.79% |
| CRC32 | 1.00 | 8 | 36.77% |

## Conclusion

1. **Performance**: 
   - BufferTag Built-in hash is the fastest (0ms)
   - FNV-1a and CityHash64 show the best performance among third-party algorithms (both 54ms)
   - XXH64 performs slightly slower (56ms)
   - MurmurHash3 has moderate performance (58ms)
   - CRC32 has the highest execution time among all algorithms (76ms)
   - All third-party algorithms show significant performance improvement (previously around 220-235ms)

2. **Distribution Quality**:
   - All algorithms show excellent distribution with average bucket size of 1.00
   - XXH64 and CRC32 have the smallest maximum bucket size (8)
   - CityHash64 has the largest maximum bucket size (10)
   - All algorithms have very similar collision rates around 36.8%, which is expected for this test configuration

## Usage

To run the benchmark:
```bash
cd /home/root123/aistore
cargo run
```

## Files

- `mod.rs` - Main hash module with algorithm implementations
- `bench.rs` - Benchmark implementation
- `README.md` - This documentation file