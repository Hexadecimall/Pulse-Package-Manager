//! Locations Pulse uses on disk.
//!
//! State (config, install database) always lives under `~/.pulse`. Installed
//! *binaries* go to a location that depends on the operating [`Mode`]: the
//! system prefix for a system install, `~/.local/bin` for a user install — and
//! a system target that isn't writable falls back to the user one.

use crate::mode::{self, Mode};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// The system install prefix. Per-OS default, overridable with `PULSE_PREFIX`:
/// `/usr` on Linux, `/opt/pulse` on macOS, `Program Files\Pulse` on Windows.
fn system_prefix() -> PathBuf {
    if let Some(prefix) = std::env::var_os("PULSE_PREFIX") {
        return PathBuf::from(prefix);
    }
    default_system_prefix()
}

#[cfg(target_os = "linux")]
fn default_system_prefix() -> PathBuf {
    PathBuf::from("/usr")
}

#[cfg(target_os = "macos")]
fn default_system_prefix() -> PathBuf {
    PathBuf::from("/opt/pulse")
}

#[cfg(target_os = "windows")]
fn default_system_prefix() -> PathBuf {
    std::env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"))
        .join("Pulse")
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn default_system_prefix() -> PathBuf {
    PathBuf::from("/usr/local")
}

/// System bin directory. On Windows the binary sits directly in the prefix;
/// elsewhere it's `<prefix>/bin`.
#[cfg(windows)]
fn system_bin_dir() -> PathBuf {
    system_prefix()
}

#[cfg(not(windows))]
fn system_bin_dir() -> PathBuf {
    system_prefix().join("bin")
}

/// System `libexec` directory — internal helper programs (the setuid helper).
/// `/usr/libexec/pulse` on Linux, `/opt/pulse/libexec` on macOS.
#[cfg(unix)]
pub fn system_libexec_dir() -> PathBuf {
    let prefix = system_prefix();
    if cfg!(target_os = "linux") {
        prefix.join("libexec").join("pulse")
    } else {
        prefix.join("libexec")
    }
}

/// System `lib` directory — Pulse's own support/data files.
/// `/usr/lib/pulse` on Linux, `/opt/pulse/lib` on macOS.
#[cfg(unix)]
pub fn system_lib_dir() -> PathBuf {
    let prefix = system_prefix();
    if cfg!(target_os = "linux") {
        prefix.join("lib").join("pulse")
    } else {
        prefix.join("lib")
    }
}

/// Path to the setuid helper binary (Unix only).
#[cfg(unix)]
pub fn helper_path() -> PathBuf {
    system_libexec_dir().join("pulse-helper")
}

/// Root of Pulse's own state: `~/.pulse`. Per-user regardless of mode.
pub fn home() -> Result<PathBuf> {
    let base = dirs::home_dir().context("could not determine the home directory")?;
    Ok(base.join(".pulse"))
}

/// Where user-mode binaries go: `~/.local/bin` on Linux/macOS, and
/// `%LOCALAPPDATA%\Pulse\bin` on Windows.
#[cfg(not(windows))]
pub fn user_bin_dir() -> Result<PathBuf> {
    let base = dirs::home_dir().context("could not determine the home directory")?;
    Ok(base.join(".local").join("bin"))
}

#[cfg(windows)]
pub fn user_bin_dir() -> Result<PathBuf> {
    let base = dirs::data_local_dir().context("could not determine the local app-data directory")?;
    Ok(base.join("Pulse").join("bin"))
}

/// Whether the setuid helper is installed (so system paths can be written
/// without being root ourselves).
#[cfg(unix)]
pub fn helper_available() -> bool {
    helper_path().exists()
}
#[cfg(not(unix))]
pub fn helper_available() -> bool {
    false
}

/// Where installed binaries go for the current run's mode. In system mode the
/// system bin dir is used when it's writable *or* the setuid helper is present
/// (which performs the privileged write); only if neither holds do we fall back
/// to the user directory, so an unprivileged install still works.
pub fn bin_dir() -> Result<PathBuf> {
    match mode::current() {
        Mode::User => user_bin_dir(),
        Mode::System => {
            let sys = system_bin_dir();
            if is_writable(&sys) || helper_available() {
                Ok(sys)
            } else {
                user_bin_dir()
            }
        }
    }
}

/// Where Pulse keeps its own support libraries (e.g. a bundled libwine) for the
/// current mode: the system `lib` dir in system mode, `~/.pulse/lib` in user
/// mode. A system target that isn't writable falls back to the user one.
#[cfg(unix)]
pub fn lib_dir() -> Result<PathBuf> {
    match mode::current() {
        Mode::User => Ok(home()?.join("lib")),
        Mode::System => {
            let sys = system_lib_dir();
            if is_writable(&sys) {
                Ok(sys)
            } else {
                Ok(home()?.join("lib"))
            }
        }
    }
}

/// `~/.pulse/config` — user settings (TOML).
pub fn config_file() -> Result<PathBuf> {
    Ok(home()?.join("config"))
}

/// `~/.pulse/db.json` — the record of everything Pulse has installed.
pub fn db_file() -> Result<PathBuf> {
    Ok(home()?.join("db.json"))
}

/// Create the state directory and the current mode's bin directory.
pub fn ensure() -> Result<()> {
    std::fs::create_dir_all(home()?).ok();
    let bin = bin_dir()?;
    std::fs::create_dir_all(&bin).with_context(|| format!("creating {}", bin.display()))?;
    Ok(())
}

/// Whether Pulse can create and write files under `dir` (checking the nearest
/// existing ancestor when `dir` itself doesn't exist yet).
pub fn is_writable(dir: &Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".pulse-write-test");
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}
