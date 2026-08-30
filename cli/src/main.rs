use anyhow::Result;
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
        /// Use a specific backend (e.g. homebrew, apt, direct)
        #[arg(long)]
        backend: Option<String>,
        /// Name for the installed binary (direct installs only)
        #[arg(long)]
        name: Option<String>,
    },
    /// Uninstall a package
    Remove { package: String },
    /// Search across the available backends
    Search { query: String },
    /// List everything Pulse has installed
    List,
    /// Update one package, or all of them
    Update { package: Option<String> },
    /// Show details about a package
    Info { package: String },
    /// Show which package managers were detected on this machine
    Backends,
    /// Check the environment and report problems
    Doctor,
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
            backend,
            name,
        } => {
            let opts = InstallOptions {
                direct,
                backend,
                name,
            };
            let pkg = ops::install(&target, &opts)?;
            let version = pkg.version.as_deref().unwrap_or("");
            println!("Installed {} {} [{}]", pkg.name, version, pkg.source);
        }
        Command::Remove { package } => {
            ops::remove(&package)?;
            println!("Removed {package}");
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
        Command::Backends => {
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
                println!("Detected sources: {}", names.join(", "));
            }
        }
    }
    Ok(())
}
