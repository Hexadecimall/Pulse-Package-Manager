//! User settings, persisted to `~/.pulse/config` (TOML).

use crate::mode::Mode;
use crate::paths;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    /// How Pulse was installed: `"system"` or `"user"`. Determines the default
    /// operating mode when no `--as-root`/`--as-user` flag is given. Written by
    /// the installer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_mode: Option<String>,
    /// The default platform (package source, e.g. `"homebrew"`) that a plain
    /// `pulse install <name>` uses when no `--platform` is given. Unset means
    /// Pulse picks the one native to this machine.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_platform: Option<String>,
    /// The update channel last used by `--update`: `"stable"`, `"beta"`, or
    /// `"dev"`. Lets a bare `--update` reuse the channel you're on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// The release tag currently installed, so `--update` can recognize when
    /// there's nothing new to fetch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_tag: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = paths::config_file()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let data =
            std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&data).context("parsing config")
    }

    pub fn save(&self) -> Result<()> {
        paths::ensure()?;
        let path = paths::config_file()?;
        let data = toml::to_string_pretty(self).context("serializing config")?;
        std::fs::write(&path, data).with_context(|| format!("writing {}", path.display()))
    }

    /// The recorded install mode, parsed.
    pub fn mode(&self) -> Option<Mode> {
        self.install_mode.as_deref().and_then(Mode::from_str_opt)
    }
}
