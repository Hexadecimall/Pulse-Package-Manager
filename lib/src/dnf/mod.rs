//! DNF source (Fedora, RHEL, and derivatives) — native, no `dnf` process.
//!
//! Detection is by distro family. Resolution (reading `repomd.xml`/`primary.xml`
//! and fetching `.rpm` packages) is on the roadmap; nothing shells out.

use crate::backend::{Backend, Package};
use crate::db::InstalledPackage;
use crate::{native, platform};
use anyhow::{Result, bail};

pub struct Dnf;

impl Backend for Dnf {
    fn name(&self) -> &'static str {
        "dnf"
    }

    fn is_available(&self) -> bool {
        cfg!(target_os = "linux")
            && (platform::distro_is("fedora")
                || platform::distro_is("rhel")
                || platform::distro_is("centos"))
    }

    fn search(&self, _query: &str) -> Result<Vec<Package>> {
        bail!("the native dnf client isn't implemented yet");
    }

    fn install(&self, package: &str) -> Result<InstalledPackage> {
        native::install_package(&resolve(package)?)
    }

    fn remove(&self, package: &str) -> Result<()> {
        native::remove(package)
    }
}

fn resolve(name: &str) -> Result<native::PackageFile> {
    bail!("the native dnf client isn't implemented yet (installing '{name}' from Fedora/RHEL repos is on the roadmap)");
}
