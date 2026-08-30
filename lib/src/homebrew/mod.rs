//! Homebrew backend (macOS and Linux). Driven through the `brew` CLI.

use crate::backend::{Backend, Package, command_exists};
use crate::db::InstalledPackage;
use crate::process;
use anyhow::Result;

pub struct Homebrew;

impl Backend for Homebrew {
    fn name(&self) -> &'static str {
        "homebrew"
    }

    fn is_available(&self) -> bool {
        command_exists("brew")
    }

    fn search(&self, query: &str) -> Result<Vec<Package>> {
        let out = process::output_as_user("brew", &["search", query])?;
        Ok(out
            .lines()
            .map(str::trim)
            // `brew search` prints section headers like "==> Formulae".
            .filter(|line| !line.is_empty() && !line.starts_with("==>"))
            .map(|name| Package {
                name: name.to_string(),
                version: None,
                description: None,
                source: "homebrew".to_string(),
            })
            .collect())
    }

    fn install(&self, package: &str) -> Result<InstalledPackage> {
        // Homebrew refuses to run as root, so `brew` always runs as the
        // invoking user even when Pulse itself is setuid-root.
        process::run_as_user("brew", &["install", package])?;
        let version = installed_version(package);
        Ok(InstalledPackage::from_backend(
            package, "homebrew", package, version,
        ))
    }

    fn remove(&self, package: &str) -> Result<()> {
        process::run_as_user("brew", &["uninstall", package])
    }

    fn update(&self, package: &str) -> Result<InstalledPackage> {
        process::run_as_user("brew", &["upgrade", package])?;
        let version = installed_version(package);
        Ok(InstalledPackage::from_backend(
            package, "homebrew", package, version,
        ))
    }
}

/// Best-effort version lookup via `brew list --versions <pkg>`, which prints
/// e.g. `ripgrep 14.1.1`. Returns `None` if anything about that fails.
fn installed_version(package: &str) -> Option<String> {
    let out = process::output_as_user("brew", &["list", "--versions", package]).ok()?;
    out.split_whitespace().nth(1).map(str::to_string)
}
