mod dns;
mod handlers;
mod rules;
mod server;

pub mod protocol;
pub mod proxy;
pub mod signal;

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

#[derive(Debug)]
struct Args {
    /// Path to rule file(s) or directory. Supports comma-separated values
    config: String,

    /// Run as a background daemon and log to file
    standalone: bool,

    /// enable multi thread
    multi_thread: bool,
}

fn print_help() {
    println!(
        "Usage: anetd --config <PATH> [OPTIONS]

Options:
  -c, --config <PATH>     Path to rule file(s) or directory. Supports comma-separated values
  -s, --standalone        Run as a background daemon and log to file
  -m, --multi-thread      Enable multi thread
  -h, --help              Print help"
    );
}

fn parse_args() -> Args {
    let mut config: Option<String> = None;
    let mut standalone = false;
    let mut multi_thread = false;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-c" | "--config" => {
                let val = it.next().unwrap_or_else(|| {
                    eprintln!("error: --config requires a value");
                    std::process::exit(1);
                });
                config = Some(val);
            }
            s if s.starts_with("--config=") => {
                config = Some(s["--config=".len()..].to_string());
            }
            s if s.starts_with("-c=") => {
                config = Some(s["-c=".len()..].to_string());
            }
            "-s" | "--standalone" => standalone = true,
            "-m" | "--multi-thread" => multi_thread = true,
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => {
                eprintln!("error: unrecognized argument '{}'", other);
                print_help();
                std::process::exit(1);
            }
        }
    }

    let config = config.unwrap_or_else(|| {
        eprintln!("error: the following required argument was not provided: --config <PATH>");
        print_help();
        std::process::exit(1);
    });

    Args {
        config,
        standalone,
        multi_thread,
    }
}

fn main() -> std::io::Result<()> {
    let args = parse_args();
    std::fs::create_dir_all(LOG_DIR!())?;

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