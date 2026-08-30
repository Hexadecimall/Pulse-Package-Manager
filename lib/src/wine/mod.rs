//! Running Windows executables through libwine, loaded as a dynamic library.
//!
//! Pulse is multilanguage here: the actual work is a small C shim
//! (`wine_run.c`) that `dlopen`s libwine and calls `wine_init` — the same entry
//! Wine's own loader uses — so Pulse drives Wine without shelling out to the
//! `wine` binary. libwine is loaded at runtime, so nothing about the build
//! depends on Wine being installed.

use anyhow::{Result, bail};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::Path;

unsafe extern "C" {
    fn pulse_wine_run(
        wine_lib: *const c_char,
        exe_path: *const c_char,
        err: *mut c_char,
        err_size: c_int,
    ) -> c_int;
}

/// Run a Windows executable via libwine.
///
/// `wine_lib` is an optional explicit path to libwine; when `None`, common
/// locations are tried. On success this **does not return** — the process
/// becomes the Wine process running the program. It only returns on failure,
/// as an error.
pub fn run(exe_path: &Path, wine_lib: Option<&str>) -> Result<()> {
    let exe = CString::new(exe_path.to_string_lossy().as_bytes())
        .map_err(|_| anyhow::anyhow!("executable path contains a NUL byte"))?;
    let lib = match wine_lib {
        Some(s) => Some(CString::new(s).map_err(|_| anyhow::anyhow!("libwine path contains a NUL byte"))?),
        None => None,
    };
    let lib_ptr = lib.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());

    let mut err = vec![0 as c_char; 1024];
    let code = unsafe { pulse_wine_run(lib_ptr, exe.as_ptr(), err.as_mut_ptr(), err.len() as c_int) };

    // Reaching here means wine_init didn't take over the process — i.e. failure.
    let message = unsafe { CStr::from_ptr(err.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    bail!("wine-run failed (code {code}): {message}");
}
