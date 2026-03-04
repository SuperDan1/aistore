//! WAL Log Buffer - Group Commit Implementation

use crate::lock::TransactionId;
use crate::wal::config::WalConfig;
use crate::wal::log_file::LogFileManager;
use crate::wal::lsn::LSN;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Pending record waiting for flush
pub struct PendingRecord {
    pub tx_id: TransactionId,
    pub lsn: LSN,
    pub data: Vec<u8>,
    pub waiter: Option<oneshot::Sender<LSN>>,
}

/// Log buffer for group commit
pub struct LogBuffer {
    config: WalConfig,
    file_mgr: Arc<LogFileManager>,
    pending: Mutex<Vec<PendingRecord>>,
    batch_count: AtomicUsize,
    running: AtomicBool,
}

impl LogBuffer {
    pub fn new(config: WalConfig, file_mgr: Arc<LogFileManager>) -> Arc<Self> {
        let buffer = Arc::new(Self {
            config: config.clone(),
            file_mgr,
            pending: Mutex::new(Vec::new()),
            batch_count: AtomicUsize::new(0),
            running: AtomicBool::new(true),
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
        waiter: Option<oneshot::Sender<LSN>>,
    ) -> LSN {
        let lsn = self
            .file_mgr
            .append(&data)
            .unwrap_or_else(|e| LSN::invalid());

        let record = PendingRecord {
            tx_id,
            lsn,
            data,
            waiter,
        };

        self.pending.lock().push(record);
        self.batch_count.fetch_add(1, Ordering::Relaxed);

        lsn
    }

    /// Flush loop - runs in background
    fn flush_loop(&self) {
        while self.running.load(Ordering::Relaxed) {
            let should_flush = {
                let count = self.batch_count.load(Ordering::Relaxed);
                count >= self.config.group_commit_batch
            };

            if should_flush {
                let _ = self.flush();
            } else {
                thread::sleep(Duration::from_millis(1));
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

        for record in records {
            if let Some(sender) = record.waiter {
                let _ = sender.send(record.lsn);
            }
        }

        Ok(())
    }

    /// Wait for a specific LSN to be flushed
    pub fn wait_for_flush(&self, tx_id: TransactionId) -> LSN {
        let (sender, receiver) = oneshot::channel();

        self.append(tx_id, vec![], Some(sender));

        receiver.recv().unwrap_or(LSN::invalid())
    }

    /// Stop the flush loop
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

/// Simple oneshot channel implementation
mod oneshot {
    use std::sync::mpsc;
    use std::thread;

    pub struct Sender<T>(mpsc::Sender<T>);
    pub struct Receiver<T>(mpsc::Receiver<T>);

    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        let (tx, rx) = mpsc::channel();
        (Sender(tx), Receiver(rx))
    }

    impl<T> Sender<T> {
        pub fn send(&self, t: T) -> Result<(), ()> {
            self.0.send(t).map_err(|_| ())
        }
    }

    impl<T> Receiver<T> {
        pub fn recv(&self) -> Result<T, ()> {
            self.0.recv().map_err(|_| ())
        }
    }
}
