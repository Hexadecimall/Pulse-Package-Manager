//! Homebrew source — native, no `brew` process involved.
//!
//! Packages are resolved through Homebrew's public JSON API
//! (`formulae.brew.sh`), which gives the bottle (a `.tar.gz`) for the current
//! platform and its SHA-256. Bottles live on GitHub's container registry, which
//! needs an anonymous bearer token on the request. The shared native installer
//! then downloads, verifies, unpacks, and places the executables.
//!
//! This grabs the package's binaries. Full Cellar relocation and dependency
//! resolution are not done yet, so formulae with runtime dependencies on other
//! bottles may not work until dependency handling lands.

use crate::backend::{Backend, Describe, Package};
use crate::db::InstalledPackage;
use crate::native::{self, PackageFile, PkgFormat};
use crate::networking;
use anyhow::{Context, Result, bail};

/// The anonymous bearer token Homebrew uses to pull bottles from GHCR.
const GHCR_ANON_TOKEN: &str = "Bearer QQ==";

pub struct Homebrew;

impl Backend for Homebrew {
    fn name(&self) -> &'static str {
        "homebrew"
    }

    /// Homebrew's bottles are macOS and Linux; treat it as available there.
    fn is_available(&self) -> bool {
        cfg!(any(target_os = "macos", target_os = "linux"))
    }

    fn search(&self, _query: &str) -> Result<Vec<Package>> {
        bail!("homebrew search isn't implemented yet");
    }

    fn describe(&self, package: &str) -> Result<Describe> {
        let url = format!("https://formulae.brew.sh/api/formula/{package}.json");
        let data =
            networking::get_json(&url).with_context(|| format!("looking up formula '{package}'"))?;
        let version = data["versions"]["stable"].as_str().map(str::to_string);
        let dependencies = data["dependencies"]
            .as_array()
            .map(|d| {
                d.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let caveats = data["caveats"].as_str().map(str::to_string);
        Ok(Describe {
            version,
            dependencies,
            caveats,
        })
    }

    fn install(&self, package: &str) -> Result<InstalledPackage> {
        let pkg = resolve(package)?;
        native::install_package(&pkg)
    }

    fn remove(&self, package: &str) -> Result<()> {
        native::remove(package)
    }
}

/// Resolve a formula name to its bottle for this platform.
fn resolve(name: &str) -> Result<PackageFile> {
    let url = format!("https://formulae.brew.sh/api/formula/{name}.json");
    let data =
        networking::get_json(&url).with_context(|| format!("looking up formula '{name}'"))?;

    let version = data["versions"]["stable"].as_str().map(str::to_string);

    let files = data["bottle"]["stable"]["files"]
        .as_object()
        .with_context(|| format!("'{name}' has no bottles"))?;

    // Pick the bottle matching this platform. Homebrew keys are `<arch>_<os>`
    // (`arm64_sonoma` for macOS arm64, bare codenames like `sonoma` for macOS
    // x86_64, `arm64_linux`/`x86_64_linux` for Linux), plus `all`.
    let want_linux = cfg!(target_os = "linux");
    let want_arm = cfg!(target_arch = "aarch64");
    let platform_ok = |k: &str| {
        let is_linux = k.contains("linux");
        if is_linux != want_linux {
            return false;
        }
        let is_arm = k.contains("arm64");
        if want_linux {
            is_arm == want_arm
        } else if want_arm {
            is_arm
        } else {
            !is_arm
        }
    };
    let bottle = files
        .iter()
        .find(|(k, _)| platform_ok(k))
        .or_else(|| files.iter().find(|(k, _)| k.as_str() == "all"))
        .map(|(_, v)| v)
        .with_context(|| format!("no bottle matching this platform for '{name}'"))?;

    let bottle_url = bottle["url"]
        .as_str()
        .context("bottle entry has no url")?
        .to_string();
    let sha256 = bottle["sha256"].as_str().map(str::to_string);

    let dependencies = data["dependencies"]
        .as_array()
        .map(|deps| {
            deps.iter()
                .filter_map(|d| d.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    Ok(PackageFile {
        name: name.to_string(),
        version,
        url: bottle_url,
        sha256,
        format: PkgFormat::TarGz,
        source: "homebrew".to_string(),
        headers: vec![("Authorization".to_string(), GHCR_ANON_TOKEN.to_string())],
        dependencies,
    })
}
