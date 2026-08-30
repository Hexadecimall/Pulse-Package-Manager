//! Pulse's own package registry: the manifest format and a client that reads a
//! Pulse index over HTTP.
//!
//! This is the "Pulse hosts its own thing" source. It is **not** a default
//! source yet — until the registry has real content, Pulse defaults to whatever
//! is native to the host OS. The crate is kept standalone (its own HTTP client,
//! no dependency on the core library) so the core can depend on it without a
//! cycle.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The default Pulse registry index. Nothing depends on it being populated yet.
pub const DEFAULT_INDEX: &str =
    "https://raw.githubusercontent.com/Hexadecimall/pulse-registry/main";

/// A single downloadable artifact for one platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    /// Direct download URL.
    pub url: String,
    /// Optional SHA-256 of the download, hex-encoded, for verification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// If the artifact is an archive, the path of the binary inside it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<String>,
}

/// A Pulse package manifest: what a registry entry describes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Per-platform artifacts, keyed by `"<os>-<arch>"` (e.g. `"macos-arm64"`).
    #[serde(default)]
    pub artifacts: BTreeMap<String, Artifact>,
    /// Names of other Pulse packages this one requires.
    #[serde(default)]
    pub dependencies: Vec<String>,
}

impl Manifest {
    /// The artifact for a given `"<os>-<arch>"` platform key, if published.
    pub fn artifact_for(&self, platform: &str) -> Option<&Artifact> {
        self.artifacts.get(platform)
    }
}

/// A client for a Pulse registry index.
pub struct Client {
    base_url: String,
}

impl Client {
    pub fn new(base_url: impl Into<String>) -> Self {
        Client {
            base_url: base_url.into(),
        }
    }

    /// The default registry (`DEFAULT_INDEX`).
    pub fn default_index() -> Self {
        Client::new(DEFAULT_INDEX)
    }

    fn http() -> Result<reqwest::blocking::Client> {
        reqwest::blocking::Client::builder()
            .user_agent(concat!("pulse-registry/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("building HTTP client")
    }

    /// Fetch a package manifest by name: `GET <base>/packages/<name>.json`.
    pub fn fetch(&self, name: &str) -> Result<Manifest> {
        let url = format!("{}/packages/{name}.json", self.base_url.trim_end_matches('/'));
        Self::http()?
            .get(&url)
            .send()
            .with_context(|| format!("requesting {url}"))?
            .error_for_status()
            .with_context(|| format!("no registry entry for '{name}'"))?
            .json()
            .with_context(|| format!("parsing manifest for '{name}'"))
    }
}
