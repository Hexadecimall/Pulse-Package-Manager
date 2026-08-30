//! Pacman source (Arch Linux and derivatives) — native, no `pacman` process.
//!
//! Detection is by distro family. Resolution (reading the `.db` repo database
//! and fetching `.pkg.tar.zst` packages) is on the roadmap; nothing shells out.

use crate::backend::{Backend, Package};
use crate::db::InstalledPackage;
use crate::{native, platform};
use anyhow::{Result, bail};

pub struct Pacman;

impl Backend for Pacman {
    fn name(&self) -> &'static str {
        "pacman"
    }

    fn is_available(&self) -> bool {
        cfg!(target_os = "linux")
            && (platform::distro_is("arch") || platform::distro_is("archlinux"))
    }

    fn search(&self, _query: &str) -> Result<Vec<Package>> {
        bail!("the native pacman client isn't implemented yet");
    }

    fn install(&self, package: &str) -> Result<InstalledPackage> {
        native::install_package(&resolve(package)?)
    }

    fn remove(&self, package: &str) -> Result<()> {
        native::remove(package)
    }
}

fn resolve(name: &str) -> Result<native::PackageFile> {
    bail!("the native pacman client isn't implemented yet (installing '{name}' from Arch repos is on the roadmap)");
}
