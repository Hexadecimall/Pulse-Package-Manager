//! The shared, native package-grabbing core.
//!
//! Every source (Homebrew, apt, pacman, …) resolves a package name to a
//! [`PackageFile`] — a download URL plus how to unpack it — and then hands it
//! to [`install_package`], which does the same work for all of them: download
//! (with any needed auth headers), verify the checksum, unpack by format, place
//! the executables into Pulse's bin directory, and return the record.
//!
//! This is the "the lib doesn't reinvent it for every manager" part: a source
//! only has to know how to *find* a package, not how to fetch and install one.

use crate::db::InstalledPackage;
use crate::{archive, networking, paths};
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};

/// How a downloaded package is packed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkgFormat {
    /// A gzip-compressed tarball (`.tar.gz`) — Homebrew bottles, many others.
    TarGz,
    /// A zip archive.
    Zip,
    /// The download is the executable itself, no container.
    Raw,
    /// xz-compressed tarball (`.tar.xz`). Not unpacked yet.
    TarXz,
    /// zstd-compressed tarball (`.pkg.tar.zst`) — Arch/MSYS2. Not unpacked yet.
    TarZst,
    /// bzip2-compressed tarball (`.tbz2`/`.tar.bz2`) — MacPorts. Not unpacked yet.
    TarBz2,
    /// Debian package (`.deb`). Not unpacked yet.
    Deb,
    /// RPM package (`.rpm`). Not unpacked yet.
    Rpm,
}

/// A concrete package to fetch and install, produced by a source's resolver.
#[derive(Debug, Clone)]
pub struct PackageFile {
    pub name: String,
    pub version: Option<String>,
    pub url: String,
    /// Hex-encoded SHA-256 of the download, verified when present.
    pub sha256: Option<String>,
    pub format: PkgFormat,
    /// The source this came from, recorded in the database (e.g. `"homebrew"`).
    pub source: String,
    /// Extra request headers for the download (e.g. a container-registry token).
    pub headers: Vec<(String, String)>,
    /// Names of other packages this one needs. Resolution/installation of
    /// dependencies is the caller's concern.
    pub dependencies: Vec<String>,
}

/// Download, verify, unpack, and install a resolved package. Returns the record
/// to persist. Does not handle dependencies — the caller resolves those.
pub fn install_package(pkg: &PackageFile) -> Result<InstalledPackage> {
    paths::ensure()?;
    let work = paths::home()?.join("tmp").join("native").join(&pkg.name);
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work).with_context(|| format!("creating {}", work.display()))?;

    let download = work.join("download");
    networking::download_with_headers(&pkg.url, &download, &pkg.headers)?;

    if let Some(expected) = &pkg.sha256 {
        verify_sha256(&download, expected)?;
    }

    let installed_path = match pkg.format {
        PkgFormat::Raw => install_single(&download, &pkg.name)?,
        PkgFormat::TarGz => {
            let extract = work.join("x");
            fs::create_dir_all(&extract)?;
            archive::extract_tar_gz(&download, &extract)?;
            place_tree(&extract, &pkg.name)?
        }
        PkgFormat::Zip => {
            let extract = work.join("x");
            fs::create_dir_all(&extract)?;
            archive::extract_zip(&download, &extract)?;
            place_tree(&extract, &pkg.name)?
        }
        other => {
            bail!("package format {other:?} isn't supported yet (installing {})", pkg.name);
        }
    };

    let _ = fs::remove_dir_all(&work);

    Ok(InstalledPackage {
        name: pkg.name.clone(),
        version: pkg.version.clone(),
        source: pkg.source.clone(),
        spec: Some(pkg.name.clone()),
        path: Some(installed_path.to_string_lossy().into_owned()),
    })
}

/// Copy `src` into place as an executable at `dest`. When the destination
/// directory isn't directly writable (a system install running unprivileged),
/// the privileged copy is done through the setuid helper. Falls back to an
/// error only when neither is possible.
pub fn place_file(src: &Path, dest: &Path) -> Result<()> {
    let parent = dest.parent().context("destination has no parent directory")?;
    if paths::is_writable(parent) {
        fs::create_dir_all(parent).ok();
        fs::copy(src, dest).with_context(|| format!("installing {}", dest.display()))?;
        archive::set_executable(dest)?;
        return Ok(());
    }
    #[cfg(unix)]
    if paths::helper_available() {
        let helper = paths::helper_path();
        let status = std::process::Command::new(&helper)
            .arg("install")
            .arg(src)
            .arg(dest)
            .status()
            .with_context(|| format!("running the setuid helper at {}", helper.display()))?;
        if status.success() {
            return Ok(());
        }
        bail!("the setuid helper failed to install {}", dest.display());
    }
    bail!(
        "cannot write {} — re-run as root, or install with --as-user",
        dest.display()
    );
}

/// Remove a natively-installed binary from the bin directory, via the setuid
/// helper when the directory isn't directly writable.
pub fn remove(name: &str) -> Result<()> {
    let path = paths::bin_dir()?.join(archive::exe_name(name));
    if !path.exists() {
        return Ok(());
    }
    let parent = path.parent().context("no parent directory")?;
    if paths::is_writable(parent) {
        return fs::remove_file(&path).with_context(|| format!("removing {}", path.display()));
    }
    #[cfg(unix)]
    if paths::helper_available() {
        let helper = paths::helper_path();
        let status = std::process::Command::new(&helper)
            .arg("remove")
            .arg(&path)
            .status()
            .with_context(|| format!("running the setuid helper at {}", helper.display()))?;
        if status.success() {
            return Ok(());
        }
        bail!("the setuid helper failed to remove {}", path.display());
    }
    bail!("cannot remove {} — re-run as root", path.display())
}

/// Install a single downloaded binary as `<bin_dir>/<name>`.
fn install_single(src: &Path, name: &str) -> Result<PathBuf> {
    let dest = paths::bin_dir()?.join(archive::exe_name(name));
    place_file(src, &dest)?;
    Ok(dest)
}

/// Place the executables from an unpacked package tree into the bin directory.
/// Prefers files that live in a `bin/` directory (the usual layout for bottles
/// and release archives); if there are none, falls back to the single-binary
/// heuristic. Returns the path of the primary installed binary.
fn place_tree(root: &Path, name: &str) -> Result<PathBuf> {
    let bin_dir = paths::bin_dir()?;

    let mut bin_files = Vec::new();
    collect_bin_files(root, &mut bin_files)?;

    if bin_files.is_empty() {
        // No bin/ directory — treat the tree as holding one loose binary.
        let found = archive::find_binary(root, name)?;
        return install_single(&found, name);
    }

    let want = archive::exe_name(name);
    let mut primary: Option<PathBuf> = None;
    for file in bin_files {
        let file_name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name.is_empty() {
            continue;
        }
        let dest = bin_dir.join(file_name);
        place_file(&file, &dest)?;
        if file_name == want || primary.is_none() {
            primary = Some(dest);
        }
    }
    primary.context("package contained a bin/ directory but no files in it")
}

/// Collect regular files that sit directly inside a directory named `bin`.
fn collect_bin_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let in_bin = dir.file_name().and_then(|n| n.to_str()) == Some("bin");
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            collect_bin_files(&path, out)?;
        } else if in_bin && path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

/// Verify a file's SHA-256 against an expected hex digest.
fn verify_sha256(path: &Path, expected: &str) -> Result<()> {
    use sha2::{Digest, Sha256};
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual = hex(&hasher.finalize());
    if !actual.eq_ignore_ascii_case(expected) {
        bail!("checksum mismatch: expected {expected}, got {actual}");
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
