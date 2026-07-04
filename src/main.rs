mod cli;
mod config;
mod daemon;
mod dns;
mod dns_server;
mod handlers;
mod logging;
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

        if let Err(e) = server::init(&args).await {
            error!("Critical failure: {}", e);
            std::process::exit(1);
        }
    });

    Ok(())
}
