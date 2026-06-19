mod dns;
mod handlers;
mod rules;
mod server;

use clap::Parser;
use std::fs::OpenOptions;
use std::io::BufWriter;
use tracing_subscriber::fmt;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long, default_value = "/data/local/tmp/anetd/rules.conf")]
    config: String,
}

fn init_logger() -> tracing_appender::non_blocking::WorkerGuard {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("app.log")
        .expect("can't create app.log");

    let buffered_writer = BufWriter::with_capacity(2 * 1024 * 1024, file);

    let (non_blocking_writer, _guard) = tracing_appender::non_blocking(buffered_writer);

    fmt()
        .with_target(false)
        .with_writer(non_blocking_writer)
        .init();

    _guard
}

fn main() -> std::io::Result<()> {
    let _log_guard = init_logger();
    let args = Args::parse();

    server::init(args.config)?;

    Ok(())
}
