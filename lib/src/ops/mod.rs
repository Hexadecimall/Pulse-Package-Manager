//! High-level operations that route work across whichever backends are
//! available on the current machine, and keep the install database in sync.
//! This is the surface the command-line front end calls into.

use crate::backend::{Backend, Describe, Package, Registry};
use crate::config::Config;
use crate::db::{Db, InstalledPackage};
use crate::progress::Spinner;
use anyhow::{Context, Result, bail};
use colored::Colorize;
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
        install_direct(&registry, target, opts.name.as_deref())?
    } else if let Some(name) = &opts.platform {
        // Explicit platform: describe it if we can (for the summary), then install.
        let info = registry
            .get(name)
            .and_then(|p| p.describe(target).ok())
            .unwrap_or_default();
        install_described(&registry, target, name, &info)?
    } else {
        install_from_any(&registry, target)?
    };

    let mut db = Db::load()?;
    db.record(record.clone());
    db.save()?;
    Ok(record)
}

/// The direct installer, with a clean spinner + success line.
fn install_direct(registry: &Registry, target: &str, name: Option<&str>) -> Result<InstalledPackage> {
    let spinner = Spinner::start(format!("Downloading {target}…"));
    let result = registry.direct().install_spec(target, name);
    spinner.stop();
    let record = result?;
    println!();
    println!("{}", format!("Successfully installed {}!", record.name).bold().green());
    Ok(record)
}

/// Resolve against whatever platform actually has the package (native + default
/// first, then non-native), then install it — with the styled summary.
fn install_from_any(registry: &Registry, target: &str) -> Result<InstalledPackage> {
    let spinner = Spinner::start("Reading package lists…");
    let mut errors = Vec::new();
    let mut chosen = None;
    for name in resolution_order(registry) {
        let Some(platform) = registry.get(&name) else {
            continue;
        };
        match platform.describe(target) {
            Ok(info) => {
                chosen = Some((name, info));
                break;
            }
            Err(e) => errors.push(format!("{name}: {e:#}")),
        }
    }
    spinner.stop();

    let Some((name, info)) = chosen else {
        if errors.is_empty() {
            bail!("no package source was detected on this system");
        }
        bail!("no platform has '{target}':\n  {}", errors.join("\n  "));
    };
    install_described(registry, target, &name, &info)
}

/// Print the styled summary and install `target` from `platform_name`, using
/// the already-known [`Describe`] info.
fn install_described(
    registry: &Registry,
    target: &str,
    platform_name: &str,
    info: &Describe,
) -> Result<InstalledPackage> {
    let platform = registry
        .get(platform_name)
        .with_context(|| format!("unknown platform '{platform_name}'"))?;

    let version = info
        .version
        .as_deref()
        .map(|v| format!(" (v{v})"))
        .unwrap_or_default();
    println!("{} {}", "Found".bold().cyan(), target.bold());
    println!(
        "{} {}{} {}",
        "Installing".bold(),
        target.bold(),
        version,
        format!("[{platform_name}]").dimmed()
    );

    let spinner = Spinner::start(format!("Downloading {target}…"));
    let result = platform.install(target);
    spinner.stop();
    let mut record = result?;

    warn_if_alternative(platform.as_ref(), registry);

    if info.dependencies.is_empty() {
        println!("{} No dependencies", "•".green());
    } else {
        println!("{} Dependencies: {}", "•".green(), info.dependencies.join(", "));
    }
    match info.caveats.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
        None => println!("{} No caveats", "•".green()),
        Some(c) => println!("{} Caveats:\n{}", "•".yellow(), c),
    }

    if record.version.is_none() {
        record.version = info.version.clone();
    }
    println!();
    println!("{}", format!("Successfully installed {target}!").bold().green());
    Ok(record)
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
