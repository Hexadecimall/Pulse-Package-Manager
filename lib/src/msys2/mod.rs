//! MSYS2 pacman source (the pacman-based package manager that ships with MSYS2
//! on Windows) — native, no `pacman` process.
//!
//! Like Arch's pacman it serves `.pkg.tar.zst` packages, but from MSYS2's own
//! repositories (`repo.msys2.org`). Detection looks for an MSYS2 environment.
//! Resolution is on the roadmap; nothing shells out.

use crate::backend::{Backend, Package};
use crate::db::InstalledPackage;
use crate::native;
use anyhow::{Result, bail};

pub struct Msys2;

impl Backend for Msys2 {
    fn name(&self) -> &'static str {
        "msys2"
    }

    fn is_available(&self) -> bool {
        // An MSYS2 environment sets MSYSTEM (MINGW64/UCRT64/…); also accept a
        // default MSYS2 install directory.
        cfg!(target_os = "windows")
            && (std::env::var_os("MSYSTEM").is_some()
                || std::path::Path::new(r"C:\msys64").exists())
    }

    fn search(&self, _query: &str) -> Result<Vec<Package>> {
        bail!("the native MSYS2 pacman client isn't implemented yet");
    }

    fn install(&self, package: &str) -> Result<InstalledPackage> {
        native::install_package(&resolve(package)?)
    }

    fn remove(&self, package: &str) -> Result<()> {
        native::remove(package)
    }
}

fn resolve(name: &str) -> Result<native::PackageFile> {
    bail!("the native MSYS2 pacman client isn't implemented yet (installing '{name}' from repo.msys2.org is on the roadmap)");
}
