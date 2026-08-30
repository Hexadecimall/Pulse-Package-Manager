//! System vs user operating mode, resolved once per run.
//!
//! The default follows how Pulse was *installed*, not the current privilege: a
//! system install (setuid-root in a system prefix) operates system-wide by
//! default; a user install (in `~/.local`) defaults to `--as-user`. A flag
//! overrides per-command, and if a system operation can't write its target at
//! runtime, callers fall back to the user location.

use crate::config::Config;
use crate::process;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// System-wide: binaries in the system prefix, needs root/setuid.
    System,
    /// Per-user: binaries under the user's home, no privileges.
    User,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::System => "system",
            Mode::User => "user",
        }
    }

    pub fn from_str_opt(s: &str) -> Option<Mode> {
        match s {
            "system" => Some(Mode::System),
            "user" => Some(Mode::User),
            _ => None,
        }
    }
}

static MODE: OnceLock<Mode> = OnceLock::new();

/// Fix the operating mode for this run (from a `--as-root`/`--as-user` flag).
/// The first call wins; later calls are ignored.
pub fn set(mode: Mode) {
    let _ = MODE.set(mode);
}

/// The operating mode for this run: the flag if one was set, otherwise the
/// recorded install mode, otherwise inferred, otherwise user.
pub fn current() -> Mode {
    *MODE.get_or_init(default_mode)
}

fn default_mode() -> Mode {
    if let Some(recorded) = Config::load().ok().and_then(|c| c.mode()) {
        return recorded;
    }
    infer_mode()
}

/// When nothing is recorded, guess from privilege: an effectively-root process
/// (e.g. a setuid-root system install) means system; anything else, user.
fn infer_mode() -> Mode {
    if process::is_root() {
        Mode::System
    } else {
        Mode::User
    }
}
