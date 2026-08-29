use anyhow::Result;
use clap::{Parser, Subcommand};
use lib_pulse::pulse::InstallOptions;
use lib_pulse::{Registry, paths, pulse};

#[derive(Parser)]
#[command(
    name = "pulse",
    version,
    about = "One command to install software on any platform"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
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
    match cli.command {
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
            let pkg = pulse::install(&target, &opts)?;
            let version = pkg.version.as_deref().unwrap_or("");
            println!("Installed {} {} [{}]", pkg.name, version, pkg.source);
        }
        Command::Remove { package } => {
            pulse::remove(&package)?;
            println!("Removed {package}");
        }
        Command::Search { query } => {
            let results = pulse::search(&query)?;
            if results.is_empty() {
                println!("No results for \"{query}\".");
            }
            for p in results {
                let version = p.version.as_deref().unwrap_or("");
                println!("{}  {}  [{}]", p.name, version, p.source);
            }
        }
        Command::List => {
            let db = pulse::installed()?;
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
                let pkg = pulse::update(&p)?;
                let version = pkg.version.as_deref().unwrap_or("");
                println!("Updated {} {} [{}]", pkg.name, version, pkg.source);
            }
            None => {
                let failures = pulse::update_all()?;
                if failures.is_empty() {
                    println!("Everything is up to date.");
                } else {
                    for (name, err) in failures {
                        eprintln!("failed to update {name}: {err}");
                    }
                }
            }
        },
        Command::Info { package } => match pulse::info(&package)? {
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
        }
        Command::Doctor => {
            paths::ensure()?;
            println!("Pulse home: {}", paths::home()?.display());
            let registry = Registry::all();
            let available = registry.available();
            if available.is_empty() {
                println!(
                    "No system package managers detected. Direct-binary installs will still work."
                );
            } else {
                let names: Vec<&str> = available.iter().map(|b| b.name()).collect();
                println!("Detected backends: {}", names.join(", "));
            }
        }
    }
    Ok(())
}
