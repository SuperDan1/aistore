//! WAL Configuration

use std::path::PathBuf;

/// WAL configuration
#[derive(Debug, Clone)]
pub struct WalConfig {
    /// WAL directory
    pub log_dir: PathBuf,
    /// Maximum size of a single log file (default 1GB)
    pub max_file_size: u64,
    /// Memory buffer size (default 8MB)
    pub buffer_size: usize,
    /// Group commit minimum batch size (default 4)
    pub group_commit_batch: usize,
    /// Group commit maximum wait time in milliseconds (default 10ms)
    pub group_commit_timeout_ms: u64,
    /// Checkpoint interval in seconds (default 60s)
    pub checkpoint_interval_sec: u64,
    /// Whether WAL is enabled
    pub enabled: bool,
}

impl Default for WalConfig {
    fn default() -> Self {
        Self {
            log_dir: PathBuf::from("./wal"),
            max_file_size: 1 << 30, // 1GB
            buffer_size: 8 << 20,   // 8MB
            group_commit_batch: 4,
            group_commit_timeout_ms: 10,
            checkpoint_interval_sec: 60,
            enabled: true,
        }
    }
}

impl WalConfig {
    /// Create a new config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a config with custom log directory
    pub fn with_log_dir(mut self, log_dir: PathBuf) -> Self {
        self.log_dir = log_dir;
        self
    }

    /// Create a config with custom max file size
    pub fn with_max_file_size(mut self, size: u64) -> Self {
        self.max_file_size = size;
        self
    }

    /// Create a config with custom buffer size
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Create a config with custom group commit batch size
    pub fn with_group_commit_batch(mut self, batch: usize) -> Self {
        self.group_commit_batch = batch;
        self
    }

    /// Create a config with custom group commit timeout
    pub fn with_group_commit_timeout(mut self, timeout_ms: u64) -> Self {
        self.group_commit_timeout_ms = timeout_ms;
        self
    }

    /// Create a config with custom checkpoint interval
    pub fn with_checkpoint_interval(mut self, interval_sec: u64) -> Self {
        self.checkpoint_interval_sec = interval_sec;
        self
    }

    /// Enable or disable WAL
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WalConfig::default();
        assert_eq!(config.max_file_size, 1 << 30);
        assert_eq!(config.buffer_size, 8 << 20);
        assert_eq!(config.group_commit_batch, 4);
        assert_eq!(config.group_commit_timeout_ms, 10);
        assert_eq!(config.checkpoint_interval_sec, 60);
        assert!(config.enabled);
    }

    #[test]
    fn test_custom_config() {
        let config = WalConfig::new()
            .with_log_dir(PathBuf::from("/tmp/wal"))
            .with_max_file_size(512 << 20)
            .with_buffer_size(4 << 20)
            .with_group_commit_batch(8)
            .with_group_commit_timeout(20)
            .with_checkpoint_interval(30)
            .with_enabled(false);

        assert_eq!(config.log_dir, PathBuf::from("/tmp/wal"));
        assert_eq!(config.max_file_size, 512 << 20);
        assert_eq!(config.buffer_size, 4 << 20);
        assert_eq!(config.group_commit_batch, 8);
        assert_eq!(config.group_commit_timeout_ms, 20);
        assert_eq!(config.checkpoint_interval_sec, 30);
        assert!(!config.enabled);
    }
}
