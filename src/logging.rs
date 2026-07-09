use crate::config::LOG_DIR;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt;

/// Build an `EnvFilter` by checking `ANETD_LOG` first, then falling back to
/// `RUST_LOG`, and finally defaulting to `"info"` when neither is set.
fn build_env_filter() -> EnvFilter {
    EnvFilter::try_from_env("ANETD_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("info"))
}

pub fn init_logger(standalone: bool) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    if standalone {
        let file_appender = tracing_appender::rolling::Builder::new()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .filename_prefix("app.log")
            .max_log_files(7)
            .build(LOG_DIR)
            .expect("failed to create log appender");

        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        fmt()
            .with_target(false)
            .with_ansi(false)
            .with_level(true)
            .with_thread_ids(false)
            .with_writer(non_blocking)
            .with_env_filter(build_env_filter())
            .init();

        Some(guard)
    } else {
        fmt()
            .with_target(true)
            .with_writer(std::io::stdout)
            .with_env_filter(build_env_filter())
            .init();

        None
    }
}
