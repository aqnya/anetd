mod cli;
mod config;
mod daemon;
mod dns;
mod dns_server;
mod handlers;
mod logging;
mod network;
mod rules;
mod server;

pub mod protocol;
pub mod session;
pub mod signal;

use tracing::error;

use crate::cli::parse_args;
use crate::config::LOG_DIR;
use crate::daemon::start_daemon;
use crate::logging::init_logger;

fn main() -> std::io::Result<()> {
    let args = parse_args();
    std::fs::create_dir_all(LOG_DIR)?;

    if args.standalone {
        start_daemon();
    }
    let runtime = if args.multi_thread {
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        builder.enable_all();

        // On Android, limiting worker threads reduces CPU wake-ups and
        // power consumption.  In battery-saver mode we use a single
        // worker; otherwise cap at 2 (DNS proxy workload is I/O-bound,
        // not CPU-bound).
        if args.battery_saver {
            builder.worker_threads(1);
        } else {
            builder.worker_threads(2);
        }

        builder.build()?
    } else {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
    };
    runtime.block_on(async {
        let _log_guard = init_logger(args.standalone);

        if let Err(e) = server::init(&args).await {
            error!("Critical failure: {}", e);
            std::process::exit(1);
        }
    });

    Ok(())
}
