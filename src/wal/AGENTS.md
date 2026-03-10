# wal/ AGENTS.md

**Generated:** Write-Ahead Log with group commit and crash recovery

## OVERVIEW
Write-Ahead Log for durability: LSN-based logging, group commit, incremental checkpoints, crash recovery. MVCC integration.

## WHERE TO LOOK
| Task | File | Notes |
|------|------|-------|
| Manager | mod.rs | `WalManager` - append, commit, flush, checkpoint |
| LSN | lsn.rs | `[file_id(16bit)][offset(48bit)]` = 64-bit |
| Log format | log_record.rs | 32-byte header, CRC32 checksum |
| Group commit | log_buffer.rs | 8MB buffer, batch≥4 or timeout≥10ms |
| Recovery | recovery.rs | Load checkpoint → redo → rollback |
| Checkpoint | checkpoint.rs | 60s interval, dirty pages + tx list |

## MODULE STRUCTURE
```
wal/
├── mod.rs           # WalManager, append/commit/flush
├── config.rs        # WalConfig (1GB files, 8MB buffer)
├── lsn.rs           # LSN(u64) with file_id + offset
├── log_record.rs    # LogRecordHeader, LogType enums
├── log_file.rs      # LogFile rotation
├── log_buffer.rs    # GroupCommit, RingBuffer
├── checkpoint.rs    # IncrementalCheckpointManager
└── recovery.rs      # Crash recovery流程
```

## CONVENTIONS (deviations from root)

### LSN Encoding
```rust
// [file_id(16bit)][offset(48bit)] = 64-bit
LSN::new(file_id: u16, offset: u64) -> LSN
lsn.file_id()  // Extract file number
lsn.offset()   // Extract file offset
```

### Group Commit
```rust
// Async append, sync commit
let lsn = wal.append(tx_id, record)?;  // Returns immediately
wal.commit(tx_id)?;  // Waits for LSN flush
```

### Checkpoint Trigger
- Interval: 60 seconds
- Records: dirty pages, active transactions, catalog snapshot

## ANTI-PATTERNS
- NEVER commit without waiting for WAL flush
- NEVER skip LSN tracking in transactions
- NEVER skip checkpoint on shutdown
- NEVER skip recovery test after crash simulation

## COMMANDS
```bash
cargo test --lib wal         # WAL tests
cargo test recovery         # Crash recovery scenarios
cargo fmt                  # Required before commit
```
