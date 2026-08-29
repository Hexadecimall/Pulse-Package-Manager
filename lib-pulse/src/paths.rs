//! Locations Pulse uses on disk. Everything lives under `~/.pulse`.

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Root of Pulse's own state: `~/.pulse`.
pub fn home() -> Result<PathBuf> {
    let base = dirs::home_dir().context("could not determine the home directory")?;
    Ok(base.join(".pulse"))
}

/// `~/.pulse/bin` — where directly-installed binaries are placed. This is the
/// directory a user adds to their `PATH`.
pub fn bin_dir() -> Result<PathBuf> {
    Ok(home()?.join("bin"))
}

/// `~/.pulse/config.toml` — user settings.
pub fn config_file() -> Result<PathBuf> {
    Ok(home()?.join("config.toml"))
}

/// `~/.pulse/db.json` — the record of everything Pulse has installed.
pub fn db_file() -> Result<PathBuf> {
    Ok(home()?.join("db.json"))
}

/// Create the Pulse state directory (and `bin/`) if they don't already exist.
pub fn ensure() -> Result<()> {
    let bin = bin_dir()?;
    std::fs::create_dir_all(&bin).with_context(|| format!("creating {}", bin.display()))?;
    Ok(())
}
