//! The direct-binary installer: install any executable straight from a URL or
//! a GitHub `owner/repo`, with no system package manager involved. This is a
//! first-class way to install software Pulse's other backends can't reach.
//!
//! Given `owner/repo`, Pulse looks at the latest GitHub release and picks the
//! asset matching this machine's OS and architecture. Given a URL, it fetches
//! it directly. Archives (`.tar.gz`, `.tgz`, `.zip`) are unpacked and the
//! executable inside is found heuristically; anything else is treated as the
//! binary itself. The result lands in `~/.pulse/bin`.

use crate::backend::{Backend, Package};
use crate::db::InstalledPackage;
use crate::{archive, networking, paths, platform};
use anyhow::{Context, Result, bail};
use std::fs;

pub struct Direct;

impl Backend for Direct {
    fn name(&self) -> &'static str {
        "direct"
    }

    /// Always usable — that's the point.
    fn is_available(&self) -> bool {
        true
    }

    fn search(&self, _query: &str) -> Result<Vec<Package>> {
        bail!("direct installs are addressed by URL or owner/repo, not searched");
    }

    fn install(&self, target: &str) -> Result<InstalledPackage> {
        self.install_spec(target, None)
    }

    fn remove(&self, name: &str) -> Result<()> {
        let path = paths::bin_dir()?.join(archive::exe_name(name));
        if !path.exists() {
            bail!("no directly-installed binary named '{name}' in ~/.pulse/bin");
        }
        fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))
    }
}

impl Direct {
    /// Install from `target` (a URL or `owner/repo`), optionally overriding the
    /// installed binary's name.
    pub fn install_spec(&self, target: &str, name_override: Option<&str>) -> Result<InstalledPackage> {
        paths::ensure()?;
        let resolved = resolve(target)?;
        let name = name_override
            .map(str::to_string)
            .unwrap_or_else(|| resolved.name.clone());

        // A private scratch directory for this install; cleaned up at the end.
        let work = paths::home()?.join("tmp").join(&name);
        let _ = fs::remove_dir_all(&work);
        fs::create_dir_all(&work).with_context(|| format!("creating {}", work.display()))?;

        let filename = url_filename(&resolved.url);
        let download_path = work.join(&filename);
        networking::download(&resolved.url, &download_path)?;

        let binary_src = if archive::is_archive(&filename) {
            let extract = work.join("extract");
            fs::create_dir_all(&extract)?;
            archive::extract(&download_path, &extract)?;
            archive::find_binary(&extract, &name)?
        } else {
            download_path.clone()
        };

        let dest = paths::bin_dir()?.join(archive::exe_name(&name));
        crate::native::place_file(&binary_src, &dest)?;
        let _ = fs::remove_dir_all(&work);

        Ok(InstalledPackage {
            name,
            version: resolved.version,
            source: "direct".to_string(),
            spec: Some(target.to_string()),
            path: Some(dest.to_string_lossy().into_owned()),
        })
    }
}

/// A download target resolved to a concrete URL, plus what to call it.
struct Resolved {
    url: String,
    name: String,
    version: Option<String>,
}

/// Turn an install target into a concrete download URL. A URL is used as-is; an
/// `owner/repo` slug is resolved against GitHub's latest release.
fn resolve(target: &str) -> Result<Resolved> {
    if target.contains("://") {
        let name = strip_archive_ext(&url_filename(target));
        return Ok(Resolved {
            url: target.to_string(),
            name,
            version: None,
        });
    }
    if let Some((owner, repo)) = parse_slug(target) {
        return resolve_github(owner, repo);
    }
    bail!("'{target}' is neither a URL nor an owner/repo slug — a direct install needs one of those");
}

fn parse_slug(target: &str) -> Option<(&str, &str)> {
    let (owner, repo) = target.split_once('/')?;
    if owner.is_empty() || repo.is_empty() || repo.contains('/') || target.contains(char::is_whitespace)
    {
        return None;
    }
    Some((owner, repo))
}

/// Resolve `owner/repo` to the best release asset for this OS/architecture.
fn resolve_github(owner: &str, repo: &str) -> Result<Resolved> {
    let api = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let release = networking::get_json(&api)
        .with_context(|| format!("looking up the latest release of {owner}/{repo}"))?;

    let version = release["tag_name"]
        .as_str()
        .map(|t| t.trim_start_matches('v').to_string());

    let assets = release["assets"]
        .as_array()
        .filter(|a| !a.is_empty())
        .context("that release has no downloadable assets")?;

    let asset = platform::select_asset(assets).with_context(|| {
        format!(
            "no asset in {owner}/{repo}'s latest release matched {}/{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })?;

    let url = asset["browser_download_url"]
        .as_str()
        .context("selected asset has no download URL")?
        .to_string();

    Ok(Resolved {
        url,
        name: repo.to_string(),
        version,
    })
}

// --- filesystem helpers -------------------------------------------------

fn url_filename(url: &str) -> String {
    url.split('?')
        .next()
        .unwrap_or(url)
        .rsplit('/')
        .next()
        .unwrap_or(url)
        .to_string()
}

fn strip_archive_ext(filename: &str) -> String {
    for ext in [".tar.gz", ".tgz", ".tar", ".zip", ".gz", ".exe"] {
        if let Some(stripped) = filename.strip_suffix(ext) {
            return stripped.to_string();
        }
    }
    filename.to_string()
}
