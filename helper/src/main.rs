//! `pulse-helper` — the setuid-root privilege helper for Pulse.
//!
//! The main `pulse` binary runs unprivileged. When a system operation needs
//! root (installing a binary into `/usr/local/bin`, removing one), it invokes
//! this helper, which is installed setuid-root. Keeping the privileged code in
//! a tiny, dependency-free binary keeps the attack surface small.
//!
//! The helper only ever writes inside a fixed set of allowed prefixes, and
//! canonicalizes paths first so `..` and symlinks can't escape them.
//!
//! Commands:
//!   pulse-helper install <src> <dest>   copy <src> to <dest> (mode 0755)
//!   pulse-helper remove  <path>         delete <path>

use std::path::Path;
use std::process::ExitCode;

/// System prefixes the helper is willing to modify. These match Pulse's per-OS
/// system bin directories (`/usr/bin` on Linux, `/opt/pulse` on macOS), plus
/// `/usr/local` for compatibility.
const ALLOWED_PREFIXES: &[&str] = &["/usr/bin/", "/opt/pulse/", "/usr/local/"];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(String::as_str) {
        Some("install") => match (args.get(2), args.get(3)) {
            (Some(src), Some(dest)) => install(Path::new(src), Path::new(dest)),
            _ => Err("usage: pulse-helper install <src> <dest>".to_string()),
        },
        Some("remove") => match args.get(2) {
            Some(path) => remove(Path::new(path)),
            None => Err("usage: pulse-helper remove <path>".to_string()),
        },
        _ => Err("usage: pulse-helper <install|remove> ...".to_string()),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("pulse-helper: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Whether `dest` resolves to a location under an allowed prefix. The parent is
/// canonicalized (it must exist), then the final component is re-appended, so a
/// not-yet-existing target file is still checked against the real parent.
fn is_allowed(dest: &Path) -> bool {
    let resolved = match dest.parent().map(Path::canonicalize) {
        Some(Ok(parent)) => match dest.file_name() {
            Some(name) => parent.join(name),
            None => return false,
        },
        _ => return false,
    };
    let s = resolved.to_string_lossy();
    ALLOWED_PREFIXES.iter().any(|p| s.starts_with(p))
}

fn install(src: &Path, dest: &Path) -> Result<(), String> {
    if !is_allowed(dest) {
        return Err(format!(
            "refusing to write outside allowed prefixes: {}",
            dest.display()
        ));
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("creating {}: {e}", parent.display()))?;
    }
    std::fs::copy(src, dest).map_err(|e| format!("copying to {}: {e}", dest.display()))?;
    set_mode(dest, 0o755)?;
    Ok(())
}

fn remove(path: &Path) -> Result<(), String> {
    if !is_allowed(path) {
        return Err(format!(
            "refusing to remove outside allowed prefixes: {}",
            path.display()
        ));
    }
    std::fs::remove_file(path).map_err(|e| format!("removing {}: {e}", path.display()))
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(|e| format!("chmod {}: {e}", path.display()))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<(), String> {
    Ok(())
}
