use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use pulse::mode::{self, Mode};
use pulse::ops::InstallOptions;
use pulse::update::{self, Channel};
use pulse::{Registry, ops, paths};
use std::str::FromStr;

#[derive(Parser)]
#[command(
    name = "pulse",
    version,
    about = "One command to install software on any platform"
)]
struct Cli {
    /// Operate system-wide (system paths; needs root or a setuid install)
    #[arg(long, global = true)]
    as_root: bool,

    /// Operate in your home (~/.local), never touching system paths or needing root
    #[arg(long, global = true, conflicts_with = "as_root")]
    as_user: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Install a package, via a system manager or directly
    Install {
        /// A package name, a URL, or a GitHub owner/repo
        target: String,
        /// Force the direct-binary installer
        #[arg(long)]
        direct: bool,
        /// Use a specific platform (e.g. homebrew, apt, macports, direct)
        #[arg(long)]
        platform: Option<String>,
        /// Name for the installed binary (direct installs only)
        #[arg(long)]
        name: Option<String>,
    },
    /// Uninstall a package
    Uninstall { package: String },
    /// Search across the available backends
    Search { query: String },
    /// List everything Pulse has installed
    List,
    /// Refresh the package list, or update Pulse itself with `update self`
    Update {
        #[command(subcommand)]
        what: Option<UpdateWhat>,
    },
    /// Show details about a package
    Info { package: String },
    /// Show which platforms (package sources) were detected on this machine
    Platforms,
    /// Check the environment and report problems
    Doctor,
    /// View or change Pulse's settings (mode, channel)
    Settings {
        /// Setting to read or change: mode | channel
        key: Option<String>,
        /// New value; omit to just read the setting
        value: Option<String>,
    },
    /// Developer tools for Pulse's own package registry
    Dev {
        #[command(subcommand)]
        command: DevCommand,
    },
    // /// Run an installed package through Wine (Windows executables)
    // #[command(name = "wine-run")]
    // WineRun {
    //     /// The installed package to run
    //     package: String,
    //     /// Explicit path to libwine (otherwise common locations are tried)
    //     #[arg(long)]
    //     wine_lib: Option<String>,
    // },
}

#[derive(Subcommand)]
enum UpdateWhat {
    /// Update Pulse itself (optionally: stable, beta, or dev)
    #[command(name = "self")]
    SelfUpdate { channel: Option<String> },
}

#[derive(Subcommand)]
enum DevCommand {
    /// Write a template package manifest to <name>.json
    New { name: String },
    /// Validate a package manifest file
    Check { path: String },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // A flag fixes the operating mode for this run; otherwise it's resolved
    // lazily from the recorded install mode (defaulting to user).
    if cli.as_root {
        mode::set(Mode::System);
    } else if cli.as_user {
        mode::set(Mode::User);
    }

    let Some(command) = cli.command else {
        Cli::command().print_help()?;
        println!();
        return Ok(());
    };

    match command {
        Command::Install {
            target,
            direct,
            platform,
            name,
        } => {
            let opts = InstallOptions {
                direct,
                platform,
                name,
            };
            // ops::install prints its own styled progress + result.
            ops::install(&target, &opts)?;
        }
        Command::Uninstall { package } => {
            ops::remove(&package)?;
            println!("Uninstalled {package}");
        }
        Command::Search { query } => {
            let results = ops::search(&query)?;
            if results.is_empty() {
                println!("No results for \"{query}\".");
            }
            for p in results {
                let version = p.version.as_deref().unwrap_or("");
                println!("{}  {}  [{}]", p.name, version, p.source);
            }
        }
        Command::List => {
            let db = ops::installed()?;
            if db.is_empty() {
                println!("Pulse hasn't installed anything yet.");
            }
            for p in db.iter() {
                let version = p.version.as_deref().unwrap_or("");
                println!("{}  {}  [{}]", p.name, version, p.source);
            }
        }
        Command::Update { what } => match what {
            // `pulse update` — refresh the package list from each platform.
            None => {
                let refreshed = ops::refresh()?;
                if refreshed.is_empty() {
                    println!("No platforms with a remote list to refresh yet.");
                } else {
                    println!("Refreshed package lists: {}", refreshed.join(", "));
                }
            }
            // `pulse update self [channel]` — update Pulse itself.
            Some(UpdateWhat::SelfUpdate { channel }) => {
                let channel = channel.map(|c| Channel::from_str(&c)).transpose()?;
                let outcome = update::self_update(channel)?;
                if outcome.already_current {
                    println!(
                        "Pulse is already up to date on {} ({}).",
                        outcome.channel.as_str(),
                        outcome.tag
                    );
                } else {
                    println!(
                        "Updated Pulse to {} [{}] at {}",
                        outcome.tag,
                        outcome.channel.as_str(),
                        outcome.path.display()
                    );
                }
            }
        },
        Command::Info { package } => match ops::info(&package)? {
            Some(p) => {
                println!("{}", p.name);
                if let Some(v) = &p.version {
                    println!("  version: {v}");
                }
                println!("  source:  {}", p.source);
                if let Some(spec) = &p.spec {
                    println!("  spec:    {spec}");
                }
                if let Some(path) = &p.path {
                    println!("  path:    {path}");
                }
            }
            None => println!("Pulse has no record of '{package}'."),
        },
        Command::Platforms => {
            let registry = Registry::all();
            for b in registry.backends() {
                let mark = if b.is_available() {
                    "available"
                } else {
                    "not found"
                };
                println!("{:<10} {}", b.name(), mark);
            }
            println!("{:<10} available", "direct");
            println!("{:<10} available", "registry");
        }
        Command::Doctor => doctor()?,
        Command::Settings { key, value } => settings(key, value)?,
        Command::Dev { command } => dev(command)?,
        // Command::WineRun { package, wine_lib } => {
        //     let db = ops::installed()?;
        //     let record = db
        //         .get(&package)
        //         .with_context(|| format!("'{package}' isn't installed"))?;
        //     let path = record
        //         .path
        //         .as_deref()
        //         .with_context(|| format!("'{package}' has no recorded binary path"))?;
        //     let lib = wine_lib.or_else(|| std::env::var("PULSE_WINE_LIB").ok());
        //     println!("Running {path} through Wine...");
        //     pulse::wine::run(std::path::Path::new(path), lib.as_deref())?;
        // }
    }
    Ok(())
}

/// Real environment diagnosis: check the things that actually break installs
/// and report each with a fix.
fn doctor() -> Result<()> {
    use std::path::Path;
    paths::ensure().ok();

    println!("Mode:       {}", mode::current().as_str());
    println!("Pulse home: {}", paths::home()?.display());
    let bin = paths::bin_dir()?;
    println!("Install to: {}", bin.display());
    println!();

    let mut problems = 0u32;
    let mut check = |ok: bool, label: &str, fix: &str| {
        if ok {
            println!("  [ok]   {label}");
        } else {
            println!("  [warn] {label} — {fix}");
            problems += 1;
        }
    };

    // Pulse's state dir must be writable.
    let home_ok = paths::home().map(|h| paths::is_writable(&h)).unwrap_or(false);
    check(home_ok, "Pulse home is writable", "check permissions on ~/.pulse");

    // The bin dir must be on PATH or installs won't be found.
    let on_path = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .any(|p| p == bin);
    check(
        on_path,
        &format!("{} is on your PATH", bin.display()),
        &format!("add: export PATH=\"{}:$PATH\"", bin.display()),
    );

    // In system mode the setuid helper should be installed.
    #[cfg(unix)]
    if mode::current() == Mode::System {
        let helper = paths::helper_path();
        check(
            helper.exists(),
            "setuid helper is installed",
            "reinstall as root so the helper is deployed",
        );
    }

    // No install should point at a binary that's gone missing.
    let db = ops::installed()?;
    let broken: Vec<String> = db
        .iter()
        .filter(|p| {
            p.path
                .as_deref()
                .map(|path| !Path::new(path).exists())
                .unwrap_or(false)
        })
        .map(|p| p.name.clone())
        .collect();
    check(
        broken.is_empty(),
        "no broken installs",
        &format!("missing binaries: {} — reinstall or uninstall them", broken.join(", ")),
    );

    // At least detect a native platform (direct/registry always work anyway).
    let registry = Registry::all();
    let available: Vec<&str> = registry.available().iter().map(|b| b.name()).collect();
    check(
        !available.is_empty(),
        "a native platform was detected",
        "none found; direct and registry installs still work",
    );

    // Network reachability.
    check(
        pulse::networking::reachable("https://github.com"),
        "network reachable (github.com)",
        "check your internet connection",
    );

    println!();
    println!(
        "Detected platforms: {}",
        if available.is_empty() {
            "(none)".to_string()
        } else {
            available.join(", ")
        }
    );
    if problems == 0 {
        println!("No problems found.");
    } else {
        println!("{problems} problem(s) found.");
    }
    Ok(())
}

/// View or change Pulse's persisted settings.
fn settings(key: Option<String>, value: Option<String>) -> Result<()> {
    use pulse::config::Config;
    let mut cfg = Config::load()?;

    let Some(key) = key else {
        // No key: show everything.
        println!("mode:             {}", cfg.install_mode.as_deref().unwrap_or("(default: user)"));
        println!("channel:          {}", cfg.channel.as_deref().unwrap_or("(default: stable)"));
        println!("default-platform: {}", cfg.default_platform.as_deref().unwrap_or("(default: native)"));
        return Ok(());
    };

    match (key.as_str(), value) {
        // Read a single setting.
        ("mode", None) => println!("{}", cfg.install_mode.as_deref().unwrap_or("(default: user)")),
        ("channel", None) => println!("{}", cfg.channel.as_deref().unwrap_or("(default: stable)")),
        ("default-platform", None) => {
            println!("{}", cfg.default_platform.as_deref().unwrap_or("(default: native)"))
        }
        // Change a setting.
        ("mode", Some(v)) => {
            if v != "user" && v != "system" {
                anyhow::bail!("mode must be 'user' or 'system'");
            }
            cfg.install_mode = Some(v.clone());
            cfg.save()?;
            println!("mode set to {v}");
        }
        ("channel", Some(v)) => {
            if !["stable", "beta", "dev"].contains(&v.as_str()) {
                anyhow::bail!("channel must be stable, beta, or dev");
            }
            cfg.channel = Some(v.clone());
            cfg.save()?;
            println!("channel set to {v}");
        }
        ("default-platform", Some(v)) => {
            if pulse::Registry::all().get(&v).is_none() {
                anyhow::bail!(
                    "unknown platform '{v}' (see `pulse platforms`; or direct / registry)"
                );
            }
            cfg.default_platform = Some(v.clone());
            cfg.save()?;
            println!("default-platform set to {v}");
        }
        (other, _) => {
            anyhow::bail!("unknown setting '{other}' (known: mode, channel, default-platform)")
        }
    }
    Ok(())
}

/// Developer tools for Pulse's own package registry.
fn dev(command: DevCommand) -> Result<()> {
    use pulse_registry::{Artifact, Manifest};
    use std::collections::BTreeMap;

    match command {
        DevCommand::New { name } => {
            let mut artifacts = BTreeMap::new();
            artifacts.insert(
                "macos-arm64".to_string(),
                Artifact {
                    url: format!("https://example.com/{name}-macos-arm64.tar.gz"),
                    sha256: None,
                    bin: None,
                },
            );
            let manifest = Manifest {
                name: name.clone(),
                version: "0.1.0".to_string(),
                description: Some(format!("The {name} package.")),
                artifacts,
                dependencies: Vec::new(),
            };
            let path = format!("{name}.json");
            let json = serde_json::to_string_pretty(&manifest)?;
            std::fs::write(&path, json)?;
            println!("Wrote template manifest to {path}");
        }
        DevCommand::Check { path } => {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {path}"))?;
            let manifest: Manifest = serde_json::from_str(&text)
                .with_context(|| format!("parsing {path}"))?;
            if manifest.artifacts.is_empty() {
                anyhow::bail!("{path}: manifest has no artifacts");
            }
            println!(
                "{path}: OK — {} {} ({} platform artifact(s))",
                manifest.name,
                manifest.version,
                manifest.artifacts.len()
            );
        }
    }
    Ok(())
}
