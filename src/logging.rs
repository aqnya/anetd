use tracing_subscriber::fmt;

use crate::config::LOG_DIR;

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
            .init();

        Some(guard)
    } else {
        fmt().with_target(true).with_writer(std::io::stdout).init();

        None
    }
}
