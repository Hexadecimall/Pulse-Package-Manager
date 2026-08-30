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

#[cfg(target_os = "macos")]
const LIBWINE_NAMES: &[&str] = &["libwine.1.dylib", "libwine.dylib"];
#[cfg(not(target_os = "macos"))]
const LIBWINE_NAMES: &[&str] = &["libwine.so.1", "libwine.so"];

/// Where Pulse looks for a bundled libwine, in order: its own `lib` dir (the
/// install location — `/usr/lib/pulse`, `/opt/pulse/lib`, or `~/.pulse/lib`),
/// then next to the running binary (handy while debugging).
fn bundled_candidates() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    #[cfg(unix)]
    if let Ok(lib) = paths::lib_dir() {
        dirs.push(lib);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        dirs.push(parent.to_path_buf());
    }
    let mut out = Vec::new();
    for dir in dirs {
        for name in LIBWINE_NAMES {
            let candidate = dir.join(name);
            if candidate.exists() {
                out.push(candidate);
            }
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
