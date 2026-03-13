use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init(log_dir: Option<PathBuf>, env_filter: &str) -> Option<WorkerGuard> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));

    let subscriber = tracing_subscriber::registry().with(filter);

    if let Some(dir) = log_dir {
        let file_appender = tracing_appender::rolling::daily(&dir, "aistore.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        let file_layer = fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true);

        let stdout_layer = fmt::layer()
            .with_target(true)
            .with_thread_ids(false)
            .with_file(true)
            .with_line_number(true);

        subscriber.with(file_layer).with(stdout_layer).init();

        Some(guard)
    } else {
        let layer = fmt::layer()
            .with_target(true)
            .with_thread_ids(false)
            .with_file(true)
            .with_line_number(true);

        subscriber.with(layer).init();

        None
    }
}
