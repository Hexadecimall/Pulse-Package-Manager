//! Privilege detection.
//!
//! Pulse never shells out to package managers — every source is native — so
//! this module is only about telling whether we're running with root
//! privileges and who the invoking user is.

/// Whether the current process is effectively root (euid 0) — true when
/// launched as root or installed setuid-root.
#[cfg(unix)]
pub fn is_root() -> bool {
    // SAFETY: geteuid is always safe to call and never fails.
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(unix))]
pub fn is_root() -> bool {
    false
}

/// The real (invoking) user's uid. Under a setuid-root binary the effective uid
/// is 0 but the real uid stays the invoker's; when launched via `sudo`, the
/// original user is in `SUDO_UID`.
#[cfg(unix)]
pub fn invoking_uid() -> u32 {
    std::env::var("SUDO_UID")
        .ok()
        .and_then(|uid| uid.parse().ok())
        // SAFETY: getuid is always safe to call and never fails.
        .unwrap_or_else(|| unsafe { libc::getuid() })
}
