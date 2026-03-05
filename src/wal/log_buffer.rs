//! WAL Log Buffer - Group Commit Implementation

use crate::lock::TransactionId;
use crate::wal::config::WalConfig;
use crate::wal::log_file::LogFileManager;
use crate::wal::lsn::LSN;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

/// Pending record waiting for flush
pub struct PendingRecord {
    pub tx_id: TransactionId,
    pub lsn: LSN,
    pub data: Vec<u8>,
    pub waiter: Option<std::sync::mpsc::Sender<LSN>>,
}

/// Log buffer for group commit
pub struct LogBuffer {
    config: WalConfig,
    file_mgr: Arc<LogFileManager>,
    pending: Mutex<Vec<PendingRecord>>,
    batch_count: AtomicUsize,
    last_flush_time: Mutex<Instant>,
    running: AtomicBool,
    flush_tx: Mutex<Option<mpsc::Sender<()>>>,
}

impl LogBuffer {
    pub fn new(config: WalConfig, file_mgr: Arc<LogFileManager>) -> Arc<Self> {
        let buffer = Arc::new(Self {
            config: config.clone(),
            file_mgr,
            pending: Mutex::new(Vec::new()),
            batch_count: AtomicUsize::new(0),
            last_flush_time: Mutex::new(Instant::now()),
            running: AtomicBool::new(true),
            flush_tx: Mutex::new(None),
        });

        let buffer_clone = Arc::clone(&buffer);
        thread::spawn(move || {
            buffer_clone.flush_loop();
        });

        buffer
    }

    /// Append a log record
    pub fn append(
        &self,
        tx_id: TransactionId,
        data: Vec<u8>,
        waiter: Option<std::sync::mpsc::Sender<LSN>>,
    ) -> Result<LSN, String> {
        let lsn = self.file_mgr.append(&data).map_err(|e| e.to_string())?;

        let record = PendingRecord {
            tx_id,
            lsn,
            data,
            waiter,
        };

        self.pending.lock().push(record);
        self.batch_count.fetch_add(1, Ordering::Relaxed);

        // Trigger flush if batch threshold reached
        if self.batch_count.load(Ordering::Relaxed) >= self.config.group_commit_batch {
            self.trigger_flush();
        }

        Ok(lsn)
    }

    /// Trigger flush from any thread
    fn trigger_flush(&self) {
        if let Some(tx) = self.flush_tx.lock().as_ref() {
            let _ = tx.send(());
        }
    }

    /// Flush loop - runs in background
    fn flush_loop(&self) {
        let (flush_tx, flush_rx) = mpsc::channel();
        *self.flush_tx.lock() = Some(flush_tx);

        while self.running.load(Ordering::Relaxed) {
            // Check if we should flush based on batch count OR timeout
            let should_flush = {
                let count = self.batch_count.load(Ordering::Relaxed);
                let elapsed = self.last_flush_time.lock().elapsed().as_millis() as u64;
                count >= self.config.group_commit_batch
                    || elapsed >= self.config.group_commit_timeout_ms
            };

            if should_flush {
                let _ = self.flush();
            }

            // Wait for flush trigger or timeout
            if should_flush {
                // Already flushed, short sleep
                thread::sleep(std::time::Duration::from_millis(1));
            } else {
                // Wait for either flush trigger or timeout
                match flush_rx.recv_timeout(std::time::Duration::from_millis(10)) {
                    Ok(_) => {
                        // Flush triggered, continue to flush
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // Timeout, loop will check flush condition
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        // Channel closed, exit
                        break;
                    }
                }
            }
        }
    }

    /// Force flush all pending records
    pub fn flush(&self) -> Result<(), String> {
        let records = {
            let mut pending = self.pending.lock();
            if pending.is_empty() {
                return Ok(());
            }
            std::mem::take(&mut *pending)
        };

        self.file_mgr.flush().map_err(|e| e.to_string())?;
        self.batch_count.store(0, Ordering::Relaxed);
        *self.last_flush_time.lock() = Instant::now();

        for record in records {
            if let Some(sender) = record.waiter {
                let _ = sender.send(record.lsn);
            }
        }

        Ok(())
    }

    /// Wait for a specific LSN to be flushed
    pub fn wait_for_flush(&self, tx_id: TransactionId) -> LSN {
        let (sender, receiver) = mpsc::channel();

        if let Err(_) = self.append(tx_id, vec![], Some(sender)) {
            return LSN::invalid();
        }

        // Now trigger flush to ensure this record gets flushed
        self.trigger_flush();

        receiver.recv().unwrap_or(LSN::invalid())
    }

    /// Stop the flush loop
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        // Trigger one more time to wake up the thread
        self.trigger_flush();
    }
}
