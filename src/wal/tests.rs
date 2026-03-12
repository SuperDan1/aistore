#![cfg(test)]

use crate::wal::{
    config::WalConfig,
    log_record::{LogRecord, LogType},
    lsn::LSN,
};

mod lsn_tests {
    use super::*;

    #[test]
    fn test_lsn_new() {
        let lsn = LSN::new(100);
        assert_eq!(lsn.raw(), 100);
    }

    #[test]
    fn test_lsn_invalid() {
        let lsn = LSN::new(0);
        assert!(!lsn.is_valid());
    }

    #[test]
    fn test_lsn_raw() {
        let lsn = LSN::new(100);
        let raw = lsn.raw();
        assert!(raw > 0);
    }

    #[test]
    fn test_lsn_from_raw() {
        let lsn = LSN::new(100);
        let raw = lsn.raw();
        let recovered = LSN::from_raw(raw);
        assert_eq!(lsn, recovered);
    }

    #[test]
    fn test_lsn_comparison() {
        let lsn1 = LSN::new(100);
        let lsn2 = LSN::new(200);
        let lsn3 = LSN::new(300);

        assert!(lsn1 < lsn2);
        assert!(lsn2 < lsn3);
        assert!(lsn1 < lsn3);
    }

    #[test]
    fn test_lsn_ordering() {
        let lsn1 = LSN::new(100);
        let lsn2 = LSN::new(200);
        assert!(lsn1 < lsn2);
    }
}

mod log_record_tests {
    use super::*;

    #[test]
    fn test_log_record_tx_begin() {
        let record = LogRecord::tx_begin(1, LSN::new(0));
        assert_eq!(record.header.tx_id, 1);
        assert_eq!(record.header.log_type, LogType::TxBegin);
    }

    #[test]
    fn test_log_record_tx_commit() {
        let record = LogRecord::tx_commit(1, LSN::new(100));
        assert_eq!(record.header.tx_id, 1);
        assert_eq!(record.header.log_type, LogType::TxCommit);
    }

    #[test]
    fn test_log_record_tx_abort() {
        let record = LogRecord::tx_abort(1, LSN::new(100));
        assert_eq!(record.header.tx_id, 1);
        assert_eq!(record.header.log_type, LogType::TxAbort);
    }

    #[test]
    fn test_log_record_serialize() {
        let record = LogRecord::tx_begin(1, LSN::new(0));
        let serialized = record.serialize();
        assert!(!serialized.is_empty());
    }

    #[test]
    fn test_log_record_serialized_size() {
        let record = LogRecord::tx_begin(1, LSN::new(0));
        let size = record.serialized_size();
        assert!(size > 0);
    }
}

mod config_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_default_config() {
        let config = WalConfig::default();
        assert_eq!(config.max_file_size, 1 << 30);
        assert_eq!(config.buffer_size, 8 << 20);
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

        assert_eq!(config.max_file_size, 512 << 20);
        assert!(!config.enabled);
    }

    #[test]
    fn test_config_builder() {
        let config = WalConfig::new()
            .with_group_commit_batch(16)
            .with_group_commit_timeout(50);

        assert_eq!(config.group_commit_batch, 16);
        assert_eq!(config.group_commit_timeout_ms, 50);
    }
}

mod recovery_tests {
    use super::*;

    #[test]
    fn test_lsn_increment_order() {
        let lsns: Vec<LSN> = (0..100).map(|i| LSN::new(i * 100)).collect();

        for i in 1..lsns.len() {
            assert!(lsns[i - 1] < lsns[i], "LSN should be ordered");
        }
    }

    #[test]
    fn test_lsn_monotonic() {
        let lsn1 = LSN::new(u64::MAX - 1);
        let lsn2 = LSN::new(u64::MAX);

        assert!(lsn1 < lsn2, "LSN should be monotonic");
    }

    #[test]
    fn test_log_record_max_size() {
        let record = LogRecord::tx_begin(1, LSN::new(0));
        let size = record.serialized_size();

        assert!(size > 0, "Log record should have valid size");
        assert!(size < 4096, "Log record should fit in one page");
    }

    #[test]
    fn test_lsn_zero_invalid() {
        let lsn = LSN::new(0);
        assert!(!lsn.is_valid(), "LSN(0) should be invalid");
    }

    #[test]
    fn test_lsn_raw_round_trip() {
        let original = LSN::new(12345);
        let raw = original.raw();
        let recovered = LSN::from_raw(raw);

        assert_eq!(original, recovered, "LSN round-trip should preserve value");
    }
}
