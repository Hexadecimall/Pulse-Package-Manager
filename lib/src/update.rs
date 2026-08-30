//! Updating Pulse itself from its GitHub releases, so users don't have to
//! build from source. Three channels:
//!
//! - **stable** — the latest thoroughly-tested release.
//! - **beta** — confirmed features, lightly tested.
//! - **dev** — the newest, experimental build.
//!
//! Everything here is native (HTTP + archive extraction); nothing is shelled
//! out. When Pulse is installed setuid-root it can replace the system binary
//! with no `sudo`; `--as-user` instead installs into `~/.pulse/bin`.

use crate::config::Config;
use crate::{archive, networking, paths, platform, process};
use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::str::FromStr;

/// The repository Pulse updates itself from.
pub const OWNER: &str = "Hexadecimall";
pub const REPO: &str = "Pulse-Package-Manager";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Beta,
    Dev,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Beta => "beta",
            Channel::Dev => "dev",
        }
    }
}

impl FromStr for Channel {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "stable" => Ok(Channel::Stable),
            "beta" => Ok(Channel::Beta),
            "dev" => Ok(Channel::Dev),
            other => bail!("unknown channel '{other}' (expected stable, beta, or dev)"),
        }
    }
}

pub struct UpdateOutcome {
    pub channel: Channel,
    pub tag: String,
    pub path: PathBuf,
    pub already_current: bool,
}

/// Update Pulse to the latest release on `channel`. With `channel` unset, reuse
/// the last channel from config (defaulting to stable). In user mode the new
/// binary goes to the user bin dir; in system mode it replaces the running
/// (system) binary in place.
pub fn self_update(channel: Option<Channel>) -> Result<UpdateOutcome> {
    let as_user = crate::mode::current() == crate::mode::Mode::User;
    let cfg = Config::load().unwrap_or_default();
    let channel = channel.unwrap_or_else(|| {
        cfg.channel
            .as_deref()
            .and_then(|c| Channel::from_str(c).ok())
            .unwrap_or(Channel::Stable)
    });

    let release = resolve_release(channel)?;
    let tag = release["tag_name"].as_str().unwrap_or_default().to_string();

    let dest = if as_user {
        paths::bin_dir()?.join(archive::exe_name("pulse"))
    } else {
        std::env::current_exe().context("finding the current executable")?
    };

    // Nothing to do if we're already on this tag — except dev, which is a
    // moving target that can change without the tag name changing.
    if !as_user
        && channel != Channel::Dev
        && cfg.installed_tag.as_deref() == Some(tag.as_str())
    {
        return Ok(UpdateOutcome {
            channel,
            tag,
            path: dest,
            already_current: true,
        });
    }

    let assets = release["assets"]
        .as_array()
        .context("that release has no downloadable assets")?;
    let want = platform::asset_basename();
    let asset = assets
        .iter()
        .find(|a| {
            a["name"]
                .as_str()
                .map(|n| n.starts_with(&want))
                .unwrap_or(false)
        })
        .with_context(|| format!("no '{want}' asset in the {} release", channel.as_str()))?;
    let url = asset["browser_download_url"]
        .as_str()
        .context("selected asset has no download URL")?;
    let filename = asset["name"].as_str().unwrap_or("pulse.tar.gz");

    // Download and extract into a scratch dir under ~/.pulse.
    paths::ensure()?;
    let work = paths::home()?.join("tmp").join("update");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).with_context(|| format!("creating {}", work.display()))?;
    let download_path = work.join(filename);
    networking::download(url, &download_path)?;

    let binary_src = if archive::is_archive(filename) {
        let extract = work.join("extract");
        std::fs::create_dir_all(&extract)?;
        archive::extract(&download_path, &extract)?;
        archive::find_binary(&extract, "pulse")?
    } else {
        download_path.clone()
    };

    // Stage next to the destination, then rename over it — renaming over a
    // running executable is fine on Unix, and keeps the swap atomic.
    let staged = dest.with_extension("new");
    std::fs::copy(&binary_src, &staged)
        .with_context(|| format!("staging new binary at {}", staged.display()))?;
    set_install_mode(&staged, as_user)?;
    std::fs::rename(&staged, &dest).with_context(|| format!("replacing {}", dest.display()))?;
    let _ = std::fs::remove_dir_all(&work);

    // Remember the channel and tag for next time.
    let mut cfg = cfg;
    cfg.channel = Some(channel.as_str().to_string());
    cfg.installed_tag = Some(tag.clone());
    cfg.save().ok();

    Ok(UpdateOutcome {
        channel,
        tag,
        path: dest,
        already_current: false,
    })
}

/// Fetch the release JSON for a channel:
/// - stable: the latest non-prerelease release,
/// - dev: the release tagged `dev`,
/// - beta: the newest prerelease whose tag mentions "beta".
fn resolve_release(channel: Channel) -> Result<serde_json::Value> {
    match channel {
        Channel::Stable => networking::get_json(&format!(
            "https://api.github.com/repos/{OWNER}/{REPO}/releases/latest"
        ))
        .context("looking up the latest stable release"),
        Channel::Dev => networking::get_json(&format!(
            "https://api.github.com/repos/{OWNER}/{REPO}/releases/tags/dev"
        ))
        .context("looking up the dev release"),
        Channel::Beta => {
            let list = networking::get_json(&format!(
                "https://api.github.com/repos/{OWNER}/{REPO}/releases?per_page=30"
            ))?;
            let releases = list.as_array().context("unexpected releases response")?;
            releases
                .iter()
                .find(|r| {
                    r["prerelease"].as_bool().unwrap_or(false)
                        && r["tag_name"]
                            .as_str()
                            .map(|t| t.to_lowercase().contains("beta"))
                            .unwrap_or(false)
                })
                .cloned()
                .context("no beta release is available yet")
        }
    }
}

/// Give the freshly-installed binary the right permissions: a system install
/// keeps the setuid-root bit (so it stays password-less); an `--as-user`
/// install is a plain executable.
#[cfg(unix)]
fn set_install_mode(path: &std::path::Path, as_user: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    // 4755 = setuid + rwxr-xr-x; 755 = plain executable.
    let mode = if !as_user && process::is_root() {
        0o4755
    } else {
        0o755
    };
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(mode);
    std::fs::set_permissions(path, perms).with_context(|| format!("chmod {}", path.display()))
}

#[cfg(not(unix))]
fn set_install_mode(_path: &std::path::Path, _as_user: bool) -> Result<()> {
    Ok(())
}
