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
use crate::{networking, paths};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

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
        let path = paths::bin_dir()?.join(exe_name(name));
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

        let binary_src = if is_tar_gz(&filename) {
            let extract = work.join("extract");
            fs::create_dir_all(&extract)?;
            extract_tar_gz(&download_path, &extract)?;
            find_binary(&extract, &name)?
        } else if is_zip(&filename) {
            let extract = work.join("extract");
            fs::create_dir_all(&extract)?;
            extract_zip(&download_path, &extract)?;
            find_binary(&extract, &name)?
        } else {
            download_path.clone()
        };

        let dest = paths::bin_dir()?.join(exe_name(&name));
        fs::copy(&binary_src, &dest)
            .with_context(|| format!("installing binary to {}", dest.display()))?;
        set_executable(&dest)?;
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

    let asset = select_asset(assets).with_context(|| {
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

/// Choose the release asset that best fits the current platform: it must match
/// the OS and (ideally) the architecture, and not be a checksum or signature.
fn select_asset(assets: &[Value]) -> Option<Value> {
    let names: Vec<(String, &Value)> = assets
        .iter()
        .filter_map(|a| Some((a["name"].as_str()?.to_lowercase(), a)))
        .collect();

    let os = os_tokens();
    let arch = arch_tokens();
    let is_aux = |n: &str| {
        [".sha256", ".sha512", ".sha1", ".md5", ".asc", ".sig", ".pem", ".txt", ".sbom", ".json"]
            .iter()
            .any(|ext| n.ends_with(ext))
    };

    let os_match = |n: &str| os.iter().any(|o| n.contains(o));
    let arch_match = |n: &str| arch.iter().any(|a| n.contains(a));

    // Best: matches OS and arch. Next: matches OS. Last resort: a lone asset.
    let mut candidates: Vec<&(String, &Value)> = names
        .iter()
        .filter(|(n, _)| !is_aux(n) && os_match(n) && arch_match(n))
        .collect();
    if candidates.is_empty() {
        candidates = names
            .iter()
            .filter(|(n, _)| !is_aux(n) && os_match(n))
            .collect();
    }
    if candidates.is_empty() && names.len() == 1 {
        candidates = names.iter().collect();
    }

    // Prefer archives (they usually hold just the binary) over bare files.
    candidates.sort_by_key(|(n, _)| {
        if n.ends_with(".tar.gz") || n.ends_with(".tgz") || n.ends_with(".zip") {
            0
        } else {
            1
        }
    });

    candidates.first().map(|(_, a)| (*a).clone())
}

fn os_tokens() -> Vec<&'static str> {
    match std::env::consts::OS {
        "macos" => vec!["macos", "darwin", "apple", "osx", "mac"],
        "linux" => vec!["linux"],
        "windows" => vec!["windows", "win"],
        other => vec![other],
    }
}

fn arch_tokens() -> Vec<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => vec!["x86_64", "amd64", "x64", "x86-64"],
        "aarch64" => vec!["aarch64", "arm64"],
        other => vec![other],
    }
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

fn is_tar_gz(filename: &str) -> bool {
    filename.ends_with(".tar.gz") || filename.ends_with(".tgz")
}

fn is_zip(filename: &str) -> bool {
    filename.ends_with(".zip")
}

#[cfg(windows)]
fn exe_name(name: &str) -> String {
    if name.ends_with(".exe") {
        name.to_string()
    } else {
        format!("{name}.exe")
    }
}

#[cfg(not(windows))]
fn exe_name(name: &str) -> String {
    name.to_string()
}

fn extract_tar_gz(src: &Path, dest: &Path) -> Result<()> {
    let file = File::open(src).with_context(|| format!("opening {}", src.display()))?;
    let decoder = flate2::read::GzDecoder::new(file);
    tar::Archive::new(decoder)
        .unpack(dest)
        .with_context(|| format!("extracting {}", src.display()))
}

fn extract_zip(src: &Path, dest: &Path) -> Result<()> {
    let file = File::open(src).with_context(|| format!("opening {}", src.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).with_context(|| format!("reading zip {}", src.display()))?;
    archive
        .extract(dest)
        .with_context(|| format!("extracting {}", src.display()))
}

/// Find the executable inside an extracted archive. Prefers an exact name
/// match, then anything that looks executable, then the largest file.
fn find_binary(dir: &Path, name: &str) -> Result<PathBuf> {
    let mut files = Vec::new();
    collect_files(dir, &mut files)?;
    if files.is_empty() {
        bail!("the archive contained no files");
    }

    let want = exe_name(name);
    if let Some(exact) = files
        .iter()
        .find(|p| p.file_name().and_then(|n| n.to_str()) == Some(want.as_str()))
    {
        return Ok(exact.clone());
    }

    let mut candidates: Vec<&PathBuf> = files.iter().filter(|p| looks_executable(p)).collect();
    if candidates.is_empty() {
        if files.len() == 1 {
            return Ok(files[0].clone());
        }
        candidates = files.iter().collect();
    }

    candidates.sort_by_key(|p| fs::metadata(p).map(|m| m.len()).unwrap_or(0));
    Ok(candidates
        .last()
        .expect("candidates is non-empty here")
        .to_path_buf())
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(unix)]
fn looks_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn looks_executable(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("exe")
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).with_context(|| format!("chmod {}", path.display()))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}
