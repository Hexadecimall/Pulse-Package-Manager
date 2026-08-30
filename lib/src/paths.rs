//! Locations Pulse uses on disk.
//!
//! State (config, install database) always lives under `~/.pulse`. Installed
//! *binaries* go to a location that depends on the operating [`Mode`]: the
//! system prefix for a system install, `~/.local/bin` for a user install — and
//! a system target that isn't writable falls back to the user one.

use crate::mode::{self, Mode};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Where system-mode binaries are installed. Overridable with `PULSE_PREFIX`
/// (which is treated as a prefix; `/bin` is appended). Defaults are per-OS:
/// `/usr/bin` on Linux, `/opt/pulse/bin` on macOS, `Program Files\Pulse` on
/// Windows.
fn system_bin_dir() -> PathBuf {
    if let Some(prefix) = std::env::var_os("PULSE_PREFIX") {
        return PathBuf::from(prefix).join("bin");
    }
    default_system_bin_dir()
}

#[cfg(target_os = "linux")]
fn default_system_bin_dir() -> PathBuf {
    PathBuf::from("/usr/bin")
}

#[cfg(target_os = "macos")]
fn default_system_bin_dir() -> PathBuf {
    PathBuf::from("/opt/pulse/bin")
}

#[cfg(target_os = "windows")]
fn default_system_bin_dir() -> PathBuf {
    let program_files = std::env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
    program_files.join("Pulse")
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn default_system_bin_dir() -> PathBuf {
    PathBuf::from("/usr/local/bin")
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

/// Where installed binaries go for the current run's mode. A system target that
/// can't be written falls back to the user directory, so an unprivileged
/// invocation still works.
pub fn bin_dir() -> Result<PathBuf> {
    match mode::current() {
        Mode::User => user_bin_dir(),
        Mode::System => {
            let sys = system_bin_dir();
            if is_writable(&sys) {
                Ok(sys)
            } else {
                user_bin_dir()
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
fn is_writable(dir: &Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".pulse-write-test");
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}
