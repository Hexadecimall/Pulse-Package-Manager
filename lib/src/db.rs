//! The record of everything Pulse has installed, persisted to
//! `~/.pulse/db.json`. This is what lets `list`, `update`, and `remove` know
//! what Pulse is responsible for — including binaries it fetched directly.

use crate::paths;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A single package Pulse has installed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The backend that installed it, e.g. `"homebrew"` or `"direct"`.
    pub source: String,
    /// The argument originally passed to `install` (a package name, a URL, or
    /// an `owner/repo` slug) — enough for `update` to redo the install.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<String>,
    /// For direct installs, the path to the binary under `~/.pulse/bin`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl InstalledPackage {
    /// A record for a package a system backend installed (no tracked path).
    pub fn from_backend(name: &str, source: &str, spec: &str, version: Option<String>) -> Self {
        InstalledPackage {
            name: name.to_string(),
            version,
            source: source.to_string(),
            spec: Some(spec.to_string()),
            path: None,
        }
    }
}

/// Pulse's on-disk install database.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Db {
    packages: BTreeMap<String, InstalledPackage>,
}

impl Db {
    /// Load the database, returning an empty one if it doesn't exist yet.
    pub fn load() -> Result<Self> {
        let path = paths::db_file()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let data =
            std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&data).context("parsing db.json")
    }

    /// Write the database back to disk, creating `~/.pulse` if needed.
    pub fn save(&self) -> Result<()> {
        paths::ensure()?;
        let path = paths::db_file()?;
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, data).with_context(|| format!("writing {}", path.display()))
    }

    pub fn record(&mut self, pkg: InstalledPackage) {
        self.packages.insert(pkg.name.clone(), pkg);
    }

    pub fn forget(&mut self, name: &str) -> Option<InstalledPackage> {
        self.packages.remove(name)
    }

    pub fn get(&self, name: &str) -> Option<&InstalledPackage> {
        self.packages.get(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = &InstalledPackage> {
        self.packages.values()
    }

    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
}
