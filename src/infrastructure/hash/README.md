# Hash Algorithm Comparison

This directory contains implementations of various hash algorithms and their performance comparison.

## Implemented Algorithms

1. **BufferTag Built-in Hash** - Simple combination of file_id and block_id
2. **XXH64** - High-performance hash algorithm
3. **MurmurHash3** - Modern, fast non-cryptographic hash
4. **FNV-1a** - Simple, good distribution hash
5. **djb2** - Classic string hash algorithm

## Performance Test Results

Performance test conducted with 1,000,000 random BufferTag instances:

### Execution Time (milliseconds)
```
┌──────────────────────────────────────────────────
│ Hash Algorithm Performance (1,000,000 tags)
├─────────────────────────┬───────────────────────
│ Algorithm               │ Time (ms)   │
├─────────────────────────┼───────────────────────
│ BufferTag Built-in      │ 3         │ █
│ XXH64                   │ 220       │ ████████████████████████████████
│ MurmurHash3             │ 232       │ ███████████████████████████████████
│ FNV-1a                  │ 235       │ ████████████████████████████████████
│ djb2                    │ 267       │ █████████████████████████████████████████████
└─────────────────────────┴───────────────────────
```

## Hash Distribution Analysis

Distribution analysis with 1,000,000 buckets (1 tag per bucket on average):

| Algorithm | Avg Bucket Size | Max Bucket Size | Collision Rate |
|-----------|-----------------|-----------------|----------------|
| BufferTag Built-in | 1.00 | 8 | 36.81% |
| XXH64 | 1.00 | 8 | 36.81% |
| djb2 | 1.00 | 8 | 36.81% |
| FNV-1a | 1.00 | 10 | 36.80% |
| MurmurHash3 | 1.00 | 10 | 37.01% |

## Conclusion

1. **Performance**: 
   - BufferTag Built-in hash is the fastest (3ms)
   - XXH64 shows the best performance among third-party algorithms (220ms)
   - MurmurHash3 and FNV-1a have similar performance
   - djb2 is the slowest among tested algorithms

2. **Distribution Quality**:
   - All algorithms show excellent distribution with average bucket size of 1.00
   - XXH64, BufferTag Built-in, and djb2 have the smallest maximum bucket size (8)
   - All algorithms have collision rates around 37%, which is expected for this test configuration

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