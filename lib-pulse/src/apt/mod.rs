//! APT backend (Debian, Ubuntu, and derivatives). Uses `apt-cache` for
//! searches (no privileges needed) and `apt-get` for changes (needs root).

use crate::backend::{Backend, Package, command_exists};
use crate::db::InstalledPackage;
use crate::process;
use anyhow::Result;

pub struct Apt;

impl Backend for Apt {
    fn name(&self) -> &'static str {
        "apt"
    }

    fn is_available(&self) -> bool {
        command_exists("apt-get")
    }

    fn search(&self, query: &str) -> Result<Vec<Package>> {
        // `apt-cache search` prints "name - description" per line.
        let out = process::output("apt-cache", &["search", query])?;
        Ok(out
            .lines()
            .filter_map(|line| {
                let (name, desc) = line.split_once(" - ")?;
                Some(Package {
                    name: name.trim().to_string(),
                    version: None,
                    description: Some(desc.trim().to_string()),
                    source: "apt".to_string(),
                })
            })
            .collect())
    }

    fn install(&self, package: &str) -> Result<InstalledPackage> {
        process::run_privileged("apt-get", &["install", "-y", package])?;
        Ok(InstalledPackage::from_backend(package, "apt", package, None))
    }

    fn remove(&self, package: &str) -> Result<()> {
        process::run_privileged("apt-get", &["remove", "-y", package])
    }

    fn update(&self, package: &str) -> Result<InstalledPackage> {
        process::run_privileged("apt-get", &["install", "--only-upgrade", "-y", package])?;
        Ok(InstalledPackage::from_backend(package, "apt", package, None))
    }
}
