//! The Pulse-registry source: installs packages described by a Pulse manifest
//! (see the `pulse-registry` crate). It resolves the manifest, picks the
//! artifact for this platform, and reuses the direct installer to fetch and
//! place the binary — all native, no shelling.
//!
//! Not part of the default source set yet: until the registry has content,
//! Pulse defaults to the host OS's native source.

use crate::backend::{Backend, Package};
use crate::db::InstalledPackage;
use crate::direct::Direct;
use crate::platform;
use anyhow::{Context, Result, bail};

pub struct RegistrySource;

impl Backend for RegistrySource {
    fn name(&self) -> &'static str {
        "registry"
    }

    /// Always usable — it's just HTTP against the index.
    fn is_available(&self) -> bool {
        true
    }

    fn search(&self, _query: &str) -> Result<Vec<Package>> {
        bail!("registry search isn't implemented yet");
    }

    fn install(&self, package: &str) -> Result<InstalledPackage> {
        let client = pulse_registry::Client::default_index();
        let manifest = client
            .fetch(package)
            .with_context(|| format!("looking up '{package}' in the Pulse registry"))?;

        let plat = format!("{}-{}", platform::canonical_os(), platform::canonical_arch());
        let artifact = manifest
            .artifact_for(&plat)
            .with_context(|| format!("'{package}' has no artifact for {plat}"))?;

        // The direct installer already does download/extract/place; reuse it,
        // then relabel the record as coming from the registry.
        let mut record = Direct.install_spec(&artifact.url, Some(&manifest.name))?;
        record.source = "registry".to_string();
        record.version = Some(manifest.version.clone());
        record.spec = Some(package.to_string());
        Ok(record)
    }

    fn remove(&self, name: &str) -> Result<()> {
        // Registry installs land in ~/.pulse/bin, same as direct ones.
        Direct.remove(name)
    }
}
