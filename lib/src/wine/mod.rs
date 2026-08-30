//! Running Windows executables through libwine, loaded as a dynamic library.
//!
//! Pulse is multilanguage here: the actual work is a small C shim
//! (`wine_run.c`) that `dlopen`s libwine and calls `wine_init` — the same entry
//! Wine's own loader uses — so Pulse drives Wine without shelling out to the
//! `wine` binary. libwine is loaded at runtime, so nothing about the build
//! depends on Wine being installed.

use crate::paths;
use anyhow::{Result, bail};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::{Path, PathBuf};

unsafe extern "C" {
    fn pulse_wine_run(
        wine_lib: *const c_char,
        exe_path: *const c_char,
        err: *mut c_char,
        err_size: c_int,
    ) -> c_int;
}

/// Whether a filename is a libwine shared library (any version suffix).
fn is_libwine(name: &str) -> bool {
    name.starts_with("libwine") && (name.ends_with(".dylib") || name.contains(".so"))
}

/// Directories to search for a *full* Wine installation (a libwine that has its
/// support files — ntdll, the loader — next to it, which a lone dylib doesn't).
fn wine_search_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();

    if let Ok(root) = std::env::var("PULSE_WINE_ROOT") {
        roots.push(PathBuf::from(root));
    }
    // A Wine bundle installed by Pulse into its lib dir.
    #[cfg(unix)]
    if let Ok(lib) = paths::lib_dir() {
        roots.push(lib.join("wine"));
        roots.push(lib);
    }
    if let Ok(home) = std::env::var("HOME") {
        roots.push(PathBuf::from(&home).join(".pulse").join("wine"));
    }

    #[cfg(target_os = "macos")]
    {
        for cellar in [
            "/opt/homebrew/Cellar/wine-stable",
            "/opt/homebrew/Cellar/wine",
            "/usr/local/Cellar/wine-stable",
            "/usr/local/Cellar/wine",
        ] {
            roots.push(PathBuf::from(cellar));
        }
        if let Ok(apps) = std::fs::read_dir("/Applications") {
            for entry in apps.flatten() {
                let p = entry.path();
                if p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.contains("Wine"))
                    .unwrap_or(false)
                {
                    roots.push(p);
                }
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        for dir in [
            "/opt/wine-stable",
            "/opt/wine-devel",
            "/opt/wine-staging",
            "/usr/lib/wine",
            "/usr/lib64/wine",
            "/usr/lib/x86_64-linux-gnu/wine",
        ] {
            roots.push(PathBuf::from(dir));
        }
    }

    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        roots.push(parent.to_path_buf());
    }
    roots
}

/// Recursively find a libwine under `dir`, bounded in depth.
fn find_libwine(dir: &std::path::Path, depth: u32) -> Option<PathBuf> {
    if depth == 0 {
        return None;
    }
    let mut subdirs = Vec::new();
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let p = entry.path();
        if p.is_file()
            && p.file_name()
                .and_then(|n| n.to_str())
                .map(is_libwine)
                .unwrap_or(false)
        {
            return Some(p);
        } else if p.is_dir() {
            subdirs.push(p);
        }
    }
    for sub in subdirs {
        if let Some(found) = find_libwine(&sub, depth - 1) {
            return Some(found);
        }
    }
    None
}

/// libwine paths to try, in order, by searching real Wine install locations.
fn bundled_candidates() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for root in wine_search_roots() {
        if let Some(found) = find_libwine(&root, 6)
            && !out.contains(&found)
        {
            out.push(found);
        }
    }
    out
}

/// Run a Windows executable via libwine.
///
/// libwine is looked for in this order: an explicit `wine_lib`, then Pulse's own
/// bundled locations ([`bundled_candidates`]), then the system defaults the C
/// shim knows. On success this **does not return** — the process becomes the
/// Wine process running the program. It only returns on failure, as an error.
pub fn run(exe_path: &Path, wine_lib: Option<&str>) -> Result<()> {
    let exe = CString::new(exe_path.to_string_lossy().as_bytes())
        .map_err(|_| anyhow::anyhow!("executable path contains a NUL byte"))?;

    // Build the ordered list of libwine paths to try. `None` means "let the C
    // shim search its built-in system locations".
    let mut attempts: Vec<Option<String>> = Vec::new();
    if let Some(explicit) = wine_lib {
        attempts.push(Some(explicit.to_string()));
    } else {
        for path in bundled_candidates() {
            attempts.push(Some(path.to_string_lossy().into_owned()));
        }
        attempts.push(None);
    }

    let mut errors: Vec<String> = Vec::new();
    for attempt in attempts {
        let lib = match &attempt {
            Some(s) => {
                Some(CString::new(s.as_str()).map_err(|_| anyhow::anyhow!("libwine path contains a NUL byte"))?)
            }
            None => None,
        };
        let lib_ptr = lib.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());

        let mut err = vec![0 as c_char; 1024];
        // On success this call does not return — the process becomes Wine.
        unsafe { pulse_wine_run(lib_ptr, exe.as_ptr(), err.as_mut_ptr(), err.len() as c_int) };
        let message = unsafe { CStr::from_ptr(err.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        let where_ = attempt.as_deref().unwrap_or("system search");
        errors.push(format!("{where_}: {message}"));
    }

    bail!("wine-run failed; no libwine could be loaded:\n  {}", errors.join("\n  "));
}
