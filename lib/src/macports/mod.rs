//! MacPorts source (macOS) — native, no `port` process.
//!
//! MacPorts serves prebuilt binary archives (`.tbz2`) from
//! `packages.macports.org`, keyed by port name, version/revision, variants, and
//! the macOS version + arch. Resolving those (from the ports index) is on the
//! roadmap; detection just checks for macOS. Nothing shells out.

use crate::backend::{Backend, Package};
use crate::db::InstalledPackage;
use crate::native;
use anyhow::{Result, bail};

pub struct Macports;

impl Backend for Macports {
    fn name(&self) -> &'static str {
        "macports"
    }

    fn is_available(&self) -> bool {
        cfg!(target_os = "macos")
    }

    fn search(&self, _query: &str) -> Result<Vec<Package>> {
        bail!("the native MacPorts client isn't implemented yet");
    }

    fn install(&self, package: &str) -> Result<InstalledPackage> {
        native::install_package(&resolve(package)?)
    }

    fn remove(&self, package: &str) -> Result<()> {
        native::remove(package)
    }
}

fn resolve(name: &str) -> Result<native::PackageFile> {
    bail!("the native MacPorts client isn't implemented yet (installing '{name}' from packages.macports.org is on the roadmap)");
}
