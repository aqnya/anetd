use std::sync::atomic::{AtomicBool, Ordering};

// NUL-terminated byte-string duplicates of PROXY_SOCKET / REAL_SOCKET
// (see crate::config).
// We avoid CString in the signal handler because both panic and allocator-free
// are not async-signal-safe.
//
// These must stay in sync with the &str constants in config.rs.  If those
// change these will need updating — the mismatch will be obvious when the
// socket paths don't match at runtime.
const PROXY_SOCKET_C: &[u8] = b"/dev/socket/dnsproxyd\0";
const REAL_SOCKET_C: &[u8] = b"/dev/socket/dnsproxyd_real\0";

pub static ORIGINAL_SOCKET_RENAMED: AtomicBool = AtomicBool::new(false);

/// Async-signal-safe cleanup handler: restores the original dnsproxyd socket
/// and exits.  Every call in this function (unlink, rename, _exit) is
/// async-signal-safe per POSIX.1-2001.  No allocation, no formatting, no lock.
unsafe extern "C" fn on_exit_signal(_: libc::c_int) {
    if ORIGINAL_SOCKET_RENAMED.load(Ordering::SeqCst) {
        unsafe {
            libc::unlink(PROXY_SOCKET_C.as_ptr() as *const libc::c_char);
            libc::rename(
                REAL_SOCKET_C.as_ptr() as *const libc::c_char,
                PROXY_SOCKET_C.as_ptr() as *const libc::c_char,
            );
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
