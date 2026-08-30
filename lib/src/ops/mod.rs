//! High-level operations that route work across whichever backends are
//! available on the current machine, and keep the install database in sync.
//! This is the surface the command-line front end calls into.

use crate::backend::{Backend, Package, Registry};
use crate::config::Config;
use crate::db::{Db, InstalledPackage};
use anyhow::{Context, Result, bail};
use std::collections::HashSet;

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
/// A URL or `owner/repo` target (or `--direct`) always uses the direct
/// installer. An explicit `--platform` forces one source. Otherwise Pulse
/// resolves the package against whatever platform actually *has* it: the
/// configured default and the native sources first, then non-native ones — so a
/// package only MacPorts carries comes from MacPorts, not Homebrew. Installing
/// from a non-native source warns that the download may not run on this kernel,
/// then proceeds anyway.
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
        let record = platform.install(target)?;
        warn_if_alternative(platform.as_ref(), &registry);
        record
    } else {
        install_from_any(&registry, target)?
    };

    let mut db = Db::load()?;
    db.record(record.clone());
    db.save()?;
    Ok(record)
}

/// Try each platform in preference order until one has the package. Native
/// sources (and the configured default) come first; non-native ones are tried
/// last. On success, warns if the source is an alternative or non-native.
fn install_from_any(registry: &Registry, target: &str) -> Result<InstalledPackage> {
    let mut errors = Vec::new();
    for name in resolution_order(registry) {
        let Some(platform) = registry.get(&name) else {
            continue;
        };
        match platform.install(target) {
            Ok(record) => {
                warn_if_alternative(platform.as_ref(), registry);
                return Ok(record);
            }
            Err(e) => errors.push(format!("{name}: {e:#}")),
        }
    }
    if errors.is_empty() {
        bail!("no package source was detected on this system");
    }
    bail!("no platform had '{target}':\n  {}", errors.join("\n  "));
}

/// Platform names to try, in order: the configured default, then the native
/// (available) sources, then the non-native ones — deduplicated.
fn resolution_order(registry: &Registry) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |name: String| {
        if seen.insert(name.clone()) {
            names.push(name);
        }
    };
    if let Some(default) = Config::load().ok().and_then(|c| c.default_platform) {
        push(default);
    }
    for b in registry.available() {
        push(b.name().to_string());
    }
    for b in registry.backends() {
        if !b.is_available() {
            push(b.name().to_string());
        }
    }
    names
}

/// Warn when a package was installed from a non-native source (may not run on
/// this kernel) or an alternative to the default one.
fn warn_if_alternative(platform: &dyn Backend, registry: &Registry) {
    if !platform.is_available() {
        eprintln!(
            "warning: '{}' isn't native to this system — the download may not run on your kernel. Installing anyway.",
            platform.name()
        );
    } else if registry.primary().map(|p| p.name()) != Some(platform.name()) {
        eprintln!(
            "note: installed from alternative source '{}'.",
            platform.name()
        );
    }
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

/// Refresh the package list of every available platform. Returns the names of
/// the platforms that actually refreshed something.
pub fn refresh() -> Result<Vec<String>> {
    let registry = Registry::all();
    let mut refreshed = Vec::new();
    for platform in registry.available() {
        if platform.refresh().unwrap_or(false) {
            refreshed.push(platform.name().to_string());
        }
    }
    Ok(refreshed)
}

/// Everything Pulse has on record as installed.
pub fn installed() -> Result<Db> {
    Db::load()
}
