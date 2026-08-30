//! Unpacking downloaded release archives and locating the executable inside.
//! Shared by the direct installer and the self-updater.

use anyhow::{Context, Result, bail};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

/// Whether a filename is an archive Pulse can unpack.
pub fn is_archive(filename: &str) -> bool {
    is_tar_gz(filename) || is_zip(filename)
}

fn is_tar_gz(filename: &str) -> bool {
    filename.ends_with(".tar.gz") || filename.ends_with(".tgz")
}

fn is_zip(filename: &str) -> bool {
    filename.ends_with(".zip")
}

/// Unpack an archive (`.tar.gz`/`.tgz`/`.zip`) into `dest`.
pub fn extract(archive_path: &Path, dest: &Path) -> Result<()> {
    let filename = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    if is_tar_gz(filename) {
        extract_tar_gz(archive_path, dest)
    } else if is_zip(filename) {
        extract_zip(archive_path, dest)
    } else {
        bail!("don't know how to extract {}", archive_path.display());
    }
}

pub fn extract_tar_gz(src: &Path, dest: &Path) -> Result<()> {
    let file = File::open(src).with_context(|| format!("opening {}", src.display()))?;
    let decoder = flate2::read::GzDecoder::new(file);
    tar::Archive::new(decoder)
        .unpack(dest)
        .with_context(|| format!("extracting {}", src.display()))
}

pub fn extract_zip(src: &Path, dest: &Path) -> Result<()> {
    let file = File::open(src).with_context(|| format!("opening {}", src.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).with_context(|| format!("reading zip {}", src.display()))?;
    archive
        .extract(dest)
        .with_context(|| format!("extracting {}", src.display()))
}

/// Find the executable inside an extracted archive. Prefers an exact name
/// match, then anything that looks executable, then the largest file.
pub fn find_binary(dir: &Path, name: &str) -> Result<PathBuf> {
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

/// The on-disk executable name for `name` — appends `.exe` on Windows.
#[cfg(windows)]
pub fn exe_name(name: &str) -> String {
    if name.ends_with(".exe") {
        name.to_string()
    } else {
        format!("{name}.exe")
    }
}

#[cfg(not(windows))]
pub fn exe_name(name: &str) -> String {
    name.to_string()
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

/// Mark a file as executable (0755). No-op on non-Unix.
#[cfg(unix)]
pub fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).with_context(|| format!("chmod {}", path.display()))
}

#[cfg(not(unix))]
pub fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}
