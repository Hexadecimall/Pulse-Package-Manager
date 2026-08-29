//! Running the underlying package-manager CLIs. System package managers have
//! no stable programmatic API, so driving their commands is the supported way
//! to use them; Pulse captures output when it needs to parse, and otherwise
//! lets it stream straight to the terminal (so prompts and progress show).

use anyhow::{Context, Result, bail};
use std::process::Command;

/// Run a command, streaming its stdout/stderr to the terminal. Errors if the
/// command can't be launched or exits non-zero.
pub fn run(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("launching `{program}`"))?;
    if !status.success() {
        bail!("`{program}` exited with status {}", status.code().unwrap_or(-1));
    }
    Ok(())
}

/// Run a command and capture its stdout as a string. Errors if the command
/// can't be launched or exits non-zero (stderr is included in the message).
pub fn output(program: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("launching `{program}`"))?;
    if !out.status.success() {
        bail!(
            "`{program}` exited with status {}: {}",
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Run a command that needs root privileges.
///
/// When Pulse is already root — either launched as root or, more usually,
/// installed setuid-root by `install.sh` — the command runs directly with no
/// `sudo`. Otherwise it falls back to prefixing `sudo`, so Pulse still works
/// when it wasn't installed setuid.
pub fn run_privileged(program: &str, args: &[&str]) -> Result<()> {
    if is_root() {
        run(program, args)
    } else {
        let mut full = Vec::with_capacity(args.len() + 1);
        full.push(program);
        full.extend_from_slice(args);
        run("sudo", &full)
    }
}

/// Run a command as the *invoking* (unprivileged) user, dropping root when
/// Pulse is setuid-root. Homebrew refuses to run as root, so its backend routes
/// every `brew` call through here.
pub fn run_as_user(program: &str, args: &[&str]) -> Result<()> {
    drop_privileges(Command::new(program).args(args))
        .status()
        .with_context(|| format!("launching `{program}`"))
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                bail!("`{program}` exited with status {}", status.code().unwrap_or(-1));
            }
        })
}

/// Like [`output`], but as the invoking user (see [`run_as_user`]).
pub fn output_as_user(program: &str, args: &[&str]) -> Result<String> {
    let out = drop_privileges(Command::new(program).args(args))
        .output()
        .with_context(|| format!("launching `{program}`"))?;
    if !out.status.success() {
        bail!(
            "`{program}` exited with status {}: {}",
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Configure a command to run as the invoking user when Pulse is setuid-root.
/// A no-op when we aren't root or the invoker is root anyway.
#[cfg(unix)]
fn drop_privileges(cmd: &mut Command) -> &mut Command {
    use std::os::unix::process::CommandExt;
    let uid = invoking_uid();
    if is_root() && uid != 0 {
        cmd.uid(uid).gid(invoking_gid());
    }
    cmd
}

#[cfg(not(unix))]
fn drop_privileges(cmd: &mut Command) -> &mut Command {
    cmd
}

/// The real (invoking) user's uid. Under a setuid-root binary the effective uid
/// is 0 but the real uid stays the invoker's; when launched via `sudo`, the
/// original user is in `SUDO_UID`.
#[cfg(unix)]
pub fn invoking_uid() -> u32 {
    std::env::var("SUDO_UID")
        .ok()
        .and_then(|uid| uid.parse().ok())
        // SAFETY: getuid is always safe to call and never fails.
        .unwrap_or_else(|| unsafe { libc::getuid() })
}

#[cfg(unix)]
fn invoking_gid() -> u32 {
    std::env::var("SUDO_GID")
        .ok()
        .and_then(|gid| gid.parse().ok())
        // SAFETY: getgid is always safe to call and never fails.
        .unwrap_or_else(|| unsafe { libc::getgid() })
}

/// Whether the current process is effectively root (euid 0) — true when
/// launched as root or installed setuid-root.
#[cfg(unix)]
pub fn is_root() -> bool {
    // SAFETY: geteuid is always safe to call and never fails.
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(unix))]
pub fn is_root() -> bool {
    false
}
