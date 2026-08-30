//! APT source (Debian, Ubuntu, and derivatives) — native, no `apt` process.
//!
//! Detection is by distro family. Resolution (reading the Debian `Packages`
//! index and fetching `.deb` archives) is on the roadmap; nothing shells out.

use crate::backend::{Backend, Package};
use crate::db::InstalledPackage;
use crate::{native, platform};
use anyhow::{Result, bail};

pub struct Apt;

impl Backend for Apt {
    fn name(&self) -> &'static str {
        "apt"
    }

    fn is_available(&self) -> bool {
        cfg!(target_os = "linux") && (platform::distro_is("debian") || platform::distro_is("ubuntu"))
    }

    fn search(&self, _query: &str) -> Result<Vec<Package>> {
        bail!("the native apt client isn't implemented yet");
    }

    fn install(&self, package: &str) -> Result<InstalledPackage> {
        native::install_package(&resolve(package)?)
    }

    fn remove(&self, package: &str) -> Result<()> {
        native::remove(package)
    }
}

fn resolve(name: &str) -> Result<native::PackageFile> {
    bail!("the native apt client isn't implemented yet (installing '{name}' from Debian repos is on the roadmap)");
}
