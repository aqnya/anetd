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
use std::fs::OpenOptions;
use std::io::BufWriter;
use tracing_subscriber::fmt;

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
    let stdout = std::fs::File::create("/data/adb/modules/anetd/log/anetd.out").unwrap();
    let stderr = std::fs::File::create("/data/adb/modules/anetd/log/anetd.err").unwrap();

    let daemonize = Daemonize::new()
        .pid_file("/data/adb/modules/anetd/log/anetd.pid")
        .chown_pid_file(true)
        .working_directory("/data/adb/modules/anetd")
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
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("/data/adb/modules/anetd/log/app.log")
            .expect("can't create app.log");

        let buffered_writer = BufWriter::with_capacity(2 * 1024 * 1024, file);
        let (non_blocking_writer, guard) = tracing_appender::non_blocking(buffered_writer);

        fmt()
            .with_target(false)
            .with_writer(non_blocking_writer)
            .with_ansi(false)
            .init();

        Some(guard)
    } else {
        fmt().with_target(false).with_writer(std::io::stdout).init();

        None
    }
}
