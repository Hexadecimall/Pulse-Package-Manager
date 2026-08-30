//! DNF backend (Fedora, RHEL, and derivatives). `dnf search` needs no
//! privileges; install/remove do.

use crate::backend::{Backend, Package, command_exists};
use crate::db::InstalledPackage;
use crate::process;
use anyhow::Result;

pub struct Dnf;

impl Backend for Dnf {
    fn name(&self) -> &'static str {
        "dnf"
    }

    fn is_available(&self) -> bool {
        command_exists("dnf")
    }

    fn search(&self, query: &str) -> Result<Vec<Package>> {
        // `dnf search` prints "name.arch : description", after a header line.
        let out = process::output("dnf", &["--quiet", "search", query])?;
        Ok(out
            .lines()
            .filter_map(|line| {
                let (name, desc) = line.split_once(" : ")?;
                // Drop the trailing ".arch" (e.g. ".x86_64") from the name.
                let name = name.trim();
                let name = name.rsplit_once('.').map(|(n, _)| n).unwrap_or(name);
                Some(Package {
                    name: name.to_string(),
                    version: None,
                    description: Some(desc.trim().to_string()),
                    source: "dnf".to_string(),
                })
            })
            .collect())
    }

    fn install(&self, package: &str) -> Result<InstalledPackage> {
        process::run_privileged("dnf", &["install", "-y", package])?;
        Ok(InstalledPackage::from_backend(package, "dnf", package, None))
    }

    fn remove(&self, package: &str) -> Result<()> {
        process::run_privileged("dnf", &["remove", "-y", package])
    }

    fn update(&self, package: &str) -> Result<InstalledPackage> {
        process::run_privileged("dnf", &["upgrade", "-y", package])?;
        Ok(InstalledPackage::from_backend(package, "dnf", package, None))
    }
}
