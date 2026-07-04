use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::{PROXY_SOCKET, REAL_SOCKET};

pub static ORIGINAL_SOCKET_RENAMED: AtomicBool = AtomicBool::new(false);

unsafe extern "C" fn on_exit_signal(_: libc::c_int) {
    if ORIGINAL_SOCKET_RENAMED.load(Ordering::SeqCst) {
        let _ = std::fs::remove_file(PROXY_SOCKET);
        let _ = std::fs::rename(REAL_SOCKET, PROXY_SOCKET);
    }
    unsafe {
        libc::_exit(0);
    }
}

pub fn setup_signals() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
        libc::signal(
            libc::SIGINT,
            on_exit_signal as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGTERM,
            on_exit_signal as *const () as libc::sighandler_t,
        );
    }
}
