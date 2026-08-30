//! winget source (Windows Package Manager) — native, no `winget` process.
//!
//! Resolution (reading the winget-pkgs manifest catalog and fetching the
//! installer) is on the roadmap; nothing shells out.

use crate::backend::{Backend, Package};
use crate::db::InstalledPackage;
use crate::native;
use anyhow::{Result, bail};

pub struct Winget;

impl Backend for Winget {
    fn name(&self) -> &'static str {
        "winget"
    }

    fn is_available(&self) -> bool {
        cfg!(target_os = "windows")
    }

    fn search(&self, _query: &str) -> Result<Vec<Package>> {
        bail!("the native winget client isn't implemented yet");
    }

    fn install(&self, package: &str) -> Result<InstalledPackage> {
        native::install_package(&resolve(package)?)
    }

    fn remove(&self, package: &str) -> Result<()> {
        native::remove(package)
    }
}

fn resolve(name: &str) -> Result<native::PackageFile> {
    bail!("the native winget client isn't implemented yet (installing '{name}' from the winget catalog is on the roadmap)");
}
