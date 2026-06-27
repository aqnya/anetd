mod dns;
mod handlers;
mod rules;
mod server;

pub mod protocol;
pub mod proxy;
pub mod signal;

use clap::Parser;
use daemonize::Daemonize;
use log::error;
use tracing_subscriber::fmt;

macro_rules! BASE_DIR {
    () => {
        "/data/adb/anetd"
    };
}
macro_rules! LOG_DIR {
    () => {
        concat!(BASE_DIR!(), "/log")
    };
}

macro_rules! PATH_OUT {
    () => {
        concat!(LOG_DIR!(), "/anetd.out")
    };
}
macro_rules! PATH_ERR {
    () => {
        concat!(LOG_DIR!(), "/anetd.err")
    };
}
macro_rules! PATH_PID {
    () => {
        concat!(LOG_DIR!(), "/anetd.pid")
    };
}

#[derive(Parser, Debug)]
struct Args {
    /// Path to rule file(s) or directory. Supports comma-separated values
    #[arg(short, long, value_name = "PATH")]
    config: String,

    /// Run as a background daemon and log to file
    #[arg(short, long, default_value_t = false)]
    standalone: bool,

    /// enable multi thread
    #[arg(short, long, default_value_t = false)]
    multi_thread: bool,
}

fn main() -> std::io::Result<()> {
    std::fs::create_dir_all(LOG_DIR!())?;
    let args = Args::parse();

    if args.standalone {
        start_daemon();
    }
    let runtime = if args.multi_thread {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
    } else {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
    };
    runtime.block_on(async {
        let _log_guard = init_logger(args.standalone);

        if let Err(e) = server::init(args.config).await {
            error!("Critical failure: {}", e);
            std::process::exit(1);
        }
    });

    Ok(())
}

fn start_daemon() {
    let stdout = std::fs::File::create(PATH_OUT!()).unwrap();
    let stderr = std::fs::File::create(PATH_ERR!()).unwrap();

    let daemonize = Daemonize::new()
        .pid_file(PATH_PID!())
        .chown_pid_file(true)
        .working_directory(BASE_DIR!())
        .stdout(stdout)
        .stderr(stderr);

    match daemonize.start() {
        Ok(_) => println!("Successfully daemonized"),
        Err(e) => {
            eprintln!("Error daemonizing, {}", e);
            std::process::exit(1);
        }
    }
}

fn init_logger(standalone: bool) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    if standalone {
        let file_appender = tracing_appender::rolling::Builder::new()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .filename_prefix("app.log")
            .max_log_files(7)
            .build(LOG_DIR!())
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
