//! Background page flusher with checkpoint support

use crate::buffer::BufferMgr;
use crate::wal::WalManager;
use crate::wal::checkpoint::TRX_INFO_PAGE_ID;
use parking_lot::RwLock;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

pub struct PageFlusher {
    buffer_mgr: Arc<RwLock<BufferMgr>>,
    wal: Option<Arc<WalManager>>,
    running: AtomicBool,
    flush_interval_ms: u64,
    dirty_ratio_threshold: f64,
    last_flush: AtomicU64,
}

impl PageFlusher {
    pub fn new(
        buffer_mgr: Arc<RwLock<BufferMgr>>,
        wal: Option<Arc<WalManager>>,
        flush_interval_ms: u64,
        dirty_ratio_threshold: f64,
    ) -> Self {
        Self {
            buffer_mgr,
            wal,
            running: AtomicBool::new(false),
            flush_interval_ms,
            dirty_ratio_threshold,
            last_flush: AtomicU64::new(0),
        }
    }

    pub fn start(&self) {
        self.running.store(true, Ordering::SeqCst);
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn try_flush(&self) -> bool {
        if !self.running.load(Ordering::SeqCst) {
            return false;
        }

        let now_ms = Instant::now().elapsed().as_millis() as u64;
        let last = self.last_flush.load(Ordering::SeqCst);

        if now_ms.saturating_sub(last) < self.flush_interval_ms {
            return false;
        }

        let buffer_len = {
            let buf = self.buffer_mgr.read();
            buf.buffer_size()
        };

        if buffer_len == 0 {
            return false;
        }

        let dirty_count = {
            let buf = self.buffer_mgr.read();
            buf.get_dirty_pages().len()
        };

        let dirty_ratio = dirty_count as f64 / buffer_len as f64;

        if dirty_ratio >= self.dirty_ratio_threshold {
            self.flush_now();
            true
        } else {
            false
        }
    }

    pub fn flush_now(&self) {
        let dirty_pages = {
            let mut buf = self.buffer_mgr.write();
            if let Err(e) = buf.flush_all() {
                eprintln!("PageFlusher: flush_now failed: {}", e);
                return;
            }
            buf.get_dirty_pages()
        };

        self.last_flush.store(
            Instant::now().elapsed().as_millis() as u64,
            Ordering::SeqCst,
        );

        if let Some(ref wal) = self.wal {
            let _ = wal.checkpoint(dirty_pages, Vec::<u64>::new(), TRX_INFO_PAGE_ID);
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Drop for PageFlusher {
    fn drop(&mut self) {
        self.stop();
    }
}
