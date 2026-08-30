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
    /// Update Pulse itself. Optionally pick a channel: stable, beta, or dev
    #[arg(long, value_name = "CHANNEL", num_args = 0..=1, default_missing_value = "")]
    update: Option<String>,

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
    /// Update one package, or all of them
    Update { package: Option<String> },
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

    // `--update [channel]` is a top-level action, separate from the package
    // `update` subcommand (which updates installed packages).
    if let Some(channel_arg) = cli.update {
        let channel = if channel_arg.is_empty() {
            None
        } else {
            Some(Channel::from_str(&channel_arg)?)
        };
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
        return Ok(());
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
            let pkg = ops::install(&target, &opts)?;
            let version = pkg.version.as_deref().unwrap_or("");
            println!("Installed {} {} [{}]", pkg.name, version, pkg.source);
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
        Command::Update { package } => match package {
            Some(p) => {
                let pkg = ops::update(&p)?;
                let version = pkg.version.as_deref().unwrap_or("");
                println!("Updated {} {} [{}]", pkg.name, version, pkg.source);
            }
            None => {
                let failures = ops::update_all()?;
                if failures.is_empty() {
                    println!("Everything is up to date.");
                } else {
                    for (name, err) in failures {
                        eprintln!("failed to update {name}: {err}");
                    }
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
        Command::Doctor => {
            paths::ensure()?;
            println!("Mode:       {}", mode::current().as_str());
            println!("Pulse home: {}", paths::home()?.display());
            println!("Install to: {}", paths::bin_dir()?.display());
            let registry = Registry::all();
            let available = registry.available();
            if available.is_empty() {
                println!("No native OS source detected. Direct and registry installs still work.");
            } else {
                let names: Vec<&str> = available.iter().map(|b| b.name()).collect();
                println!("Detected platforms: {}", names.join(", "));
            }
        }
        Command::Settings { key, value } => settings(key, value)?,
        Command::Dev { command } => dev(command)?,
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
