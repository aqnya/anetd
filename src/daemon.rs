use daemonize::Daemonize;

use crate::config::{BASE_DIR, PATH_ERR, PATH_OUT, PATH_PID};

pub fn start_daemon() {
    let stdout = std::fs::File::create(PATH_OUT).unwrap();
    let stderr = std::fs::File::create(PATH_ERR).unwrap();

    let daemonize = Daemonize::new()
        .pid_file(PATH_PID)
        .chown_pid_file(true)
        .working_directory(BASE_DIR)
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
