use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::{PROXY_SOCKET, REAL_SOCKET};

pub static ORIGINAL_SOCKET_RENAMED: AtomicBool = AtomicBool::new(false);

/// Async-signal-safe cleanup handler: restores the original dnsproxyd socket
/// and exits.  Uses only `libc::unlink`, `libc::rename` and `libc::_exit` —
/// all of which are async-signal-safe per POSIX.1-2001.
unsafe extern "C" fn on_exit_signal(_: libc::c_int) {
    if ORIGINAL_SOCKET_RENAMED.load(Ordering::SeqCst) {
        // SAFETY: PROXY_SOCKET and REAL_SOCKET are constants without interior NULs.
        let proxy = CString::new(PROXY_SOCKET).unwrap();
        let real = CString::new(REAL_SOCKET).unwrap();
        unsafe {
            libc::unlink(proxy.as_ptr());
            libc::rename(real.as_ptr(), proxy.as_ptr());
        }
    }
    unsafe {
        libc::_exit(0);
    }
}

/// Install signal handlers using `sigaction` (POSIX-recommended over `signal`).
///
/// * `SIGPIPE` → ignored (avoid crash on broken pipe).
/// * `SIGINT` / `SIGTERM` → restore original dnsproxyd socket, then exit.
pub fn setup_signals() {
    unsafe {
        // sa_flags = 0 means NO SA_RESTART — interrupted syscalls return
        // EINTR.  This is harmless here because on_exit_signal calls _exit(0)
        // and never returns, so there is nothing to "restart".
        let mut sa: libc::sigaction = std::mem::zeroed();

        // On Android/Bionic the handler field is `sa_sigaction` (cf. Linux kernel
        // view), not `sa_handler`.
        #[cfg(target_os = "android")]
        {
            sa.sa_sigaction = libc::SIG_IGN as libc::sighandler_t;
            libc::sigaction(libc::SIGPIPE, &sa, std::ptr::null_mut());

            sa.sa_sigaction = on_exit_signal as *const () as libc::sighandler_t;
            libc::sigaction(libc::SIGINT, &sa, std::ptr::null_mut());
            libc::sigaction(libc::SIGTERM, &sa, std::ptr::null_mut());
        }
        #[cfg(not(target_os = "android"))]
        {
            sa.sa_handler = libc::SIG_IGN;
            libc::sigaction(libc::SIGPIPE, &sa, std::ptr::null_mut());

            sa.sa_handler = on_exit_signal as libc::sighandler_t;
            libc::sigaction(libc::SIGINT, &sa, std::ptr::null_mut());
            libc::sigaction(libc::SIGTERM, &sa, std::ptr::null_mut());
        }
    }
}
