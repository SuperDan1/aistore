# Aistore

Building a high-performance, high-resilience, high-availability storage engine

## Project Overview

Aistore is a modern storage engine developed in Rust, designed to provide:

- **High Performance**: Leveraging Rust's memory safety features and zero-cost abstractions for maximum performance
- **High Resilience**: Automatic failure detection, recovery, and data redundancy capabilities
- **High Availability**: Support for distributed deployment to ensure continuous service availability

## Core Features

- Key-value storage interface
- Transaction support
- Data persistence
- Horizontal scaling capability
- Monitoring and management interface

## Technology Stack

- **Language**: Rust
- **Build Tool**: Cargo
- **Version Control**: Git

## Getting Started

### Build the Project

```bash
cargo build --release
```

### Run the Project

```bash
cargo run --release
```

### Run Tests

```bash
cargo test
```

## Project Structure

```
aistore/
├── src/                # Source code directory
│   └── main.rs        # Main program entry
├── Cargo.toml         # Cargo configuration file
├── README.md          # Project documentation
└── .gitignore         # Git ignore file
```

## Contributing

Contributions are welcome! Please check the CONTRIBUTING.md file for contribution guidelines.

## Performance Benchmarks

The following are the performance results of various hash algorithms implemented in Aistore (measured in milliseconds for 1,000,000 operations):

| Algorithm | Time (ms) |
|-----------|-----------|
| BufferTag's hash | 0 |
| FNV-1a | 53 |
| MurmurHash3 | 57 |
| XXH64 | 56 |
| CityHash64 | 52 |
| CRC32 | 60 |

## License

This project is licensed under the MIT License. See the LICENSE file for details.
