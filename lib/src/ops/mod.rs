//! High-level operations that route work across whichever backends are
//! available on the current machine, and keep the install database in sync.
//! This is the surface the command-line front end calls into.

use crate::backend::{Package, Registry};
use crate::config::Config;
use crate::db::{Db, InstalledPackage};
use anyhow::{Context, Result};

/// Options controlling how [`install`] chooses a platform.
#[derive(Debug, Default)]
pub struct InstallOptions {
    /// Force the direct-binary installer regardless of the target's shape.
    pub direct: bool,
    /// Force a specific platform by name (e.g. `"homebrew"`, `"direct"`).
    pub platform: Option<String>,
    /// Override the installed binary's name (direct installs only).
    pub name: Option<String>,
}

/// Whether a target looks like a direct install (a URL or an `owner/repo`
/// slug) rather than a plain package name.
fn looks_direct(target: &str) -> bool {
    target.contains("://") || target.contains('/')
}

/// Install a package, recording it in the database on success.
///
/// Platform choice: an explicit `--platform`, else the configured
/// `default_platform`, else the one native to this machine. A URL or
/// `owner/repo` target (or `--direct`) always uses the direct installer.
pub fn install(target: &str, opts: &InstallOptions) -> Result<InstalledPackage> {
    let registry = Registry::all();

    let record = if opts.platform.as_deref() == Some("direct")
        || opts.direct
        || (opts.platform.is_none() && looks_direct(target))
    {
        registry.direct().install_spec(target, opts.name.as_deref())?
    } else if let Some(name) = &opts.platform {
        let platform = registry
            .get(name)
            .with_context(|| format!("unknown platform '{name}'"))?;
        platform.install(target)?
    } else if let Some(name) = Config::load().ok().and_then(|c| c.default_platform) {
        let platform = registry
            .get(&name)
            .with_context(|| format!("configured default-platform '{name}' is unknown"))?;
        platform.install(target)?
    } else {
        let platform = registry
            .primary()
            .context("no package source was detected on this system")?;
        platform.install(target)?
    };

    let mut db = Db::load()?;
    db.record(record.clone());
    db.save()?;
    Ok(record)
}

/// Remove a package. If Pulse installed it, the recorded backend does the
/// removal and the record is dropped; otherwise the primary system manager is
/// asked to remove it.
pub fn remove(name: &str) -> Result<()> {
    let mut db = Db::load()?;
    if let Some(record) = db.get(name).cloned() {
        let registry = Registry::all();
        let backend = registry
            .get(&record.source)
            .with_context(|| format!("backend '{}' is no longer available", record.source))?;
        backend.remove(name)?;
        db.forget(name);
        db.save()?;
        return Ok(());
    }

    let registry = Registry::all();
    let backend = registry
        .primary()
        .context("no supported package manager was detected on this system")?;
    backend.remove(name)
}

/// Update a package Pulse installed to the latest version, refreshing its
/// record. With no name, updates everything Pulse tracks.
pub fn update(name: &str) -> Result<InstalledPackage> {
    let mut db = Db::load()?;
    let record = db
        .get(name)
        .cloned()
        .with_context(|| format!("'{name}' isn't tracked by Pulse; install it first"))?;

    let registry = Registry::all();
    let spec = record.spec.as_deref().unwrap_or(name);

    let updated = if record.source == "direct" {
        registry.direct().install_spec(spec, Some(name))?
    } else {
        let backend = registry
            .get(&record.source)
            .with_context(|| format!("backend '{}' is no longer available", record.source))?;
        backend.update(spec)?
    };

    db.record(updated.clone());
    db.save()?;
    Ok(updated)
}

/// Update every package Pulse tracks. Returns the names that failed, with the
/// error, so the caller can report them without aborting the whole run.
pub fn update_all() -> Result<Vec<(String, String)>> {
    let names: Vec<String> = Db::load()?.iter().map(|p| p.name.clone()).collect();
    let mut failures = Vec::new();
    for name in names {
        if let Err(e) = update(&name) {
            failures.push((name, format!("{e:#}")));
        }
    }
    Ok(failures)
}

/// Search every available backend for a query, gathering all results. A
/// backend that can't fulfil the search is skipped rather than failing the
/// whole query.
pub fn search(query: &str) -> Result<Vec<Package>> {
    let registry = Registry::all();
    let mut results = Vec::new();
    for backend in registry.available() {
        if let Ok(mut found) = backend.search(query) {
            results.append(&mut found);
        }
    }
    Ok(results)
}

/// The record Pulse holds for a package, if any.
pub fn info(name: &str) -> Result<Option<InstalledPackage>> {
    Ok(Db::load()?.get(name).cloned())
}

/// Everything Pulse has on record as installed.
pub fn installed() -> Result<Db> {
    Db::load()
}
