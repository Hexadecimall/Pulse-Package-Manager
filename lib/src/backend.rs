//! The [`Backend`] abstraction and the [`Registry`] that selects between
//! backends for the current machine.

use crate::apt::Apt;
use crate::db::InstalledPackage;
use crate::direct::Direct;
use crate::dnf::Dnf;
use crate::homebrew::Homebrew;
use crate::macports::Macports;
use crate::msys2::Msys2;
use crate::pacman::Pacman;
use crate::winget::Winget;
use anyhow::Result;

/// A package as surfaced by a backend's search.
#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    /// Identifier of the backend this result came from, e.g. `"homebrew"`.
    pub source: String,
}

/// What a platform knows about a package before installing it — used to show a
/// clean summary and to decide which platform actually *has* the package.
#[derive(Debug, Default, Clone)]
pub struct Describe {
    pub version: Option<String>,
    pub dependencies: Vec<String>,
    pub caveats: Option<String>,
}

/// A source Pulse can install software from — a system package manager, or the
/// direct-binary installer. Every backend implements the same handful of
/// operations, so the front end never has to special-case one.
pub trait Backend {
    /// Stable identifier, e.g. `"homebrew"`, `"apt"`, `"direct"`.
    fn name(&self) -> &'static str;

    /// Whether this backend can actually run on the current machine.
    fn is_available(&self) -> bool;

    fn search(&self, query: &str) -> Result<Vec<Package>>;

    /// Look a package up without installing it: confirms this platform has it
    /// and returns its version / dependencies / caveats. Errors if the platform
    /// doesn't carry the package. Defaults to "can't describe yet".
    fn describe(&self, package: &str) -> Result<Describe> {
        anyhow::bail!("{} can't look up '{package}' yet", self.name());
    }

    /// Install a package and return the record to persist for it.
    fn install(&self, package: &str) -> Result<InstalledPackage>;

    fn remove(&self, package: &str) -> Result<()>;

    /// Update a package to the latest version. Defaults to reinstalling, which
    /// is correct for backends whose install already fetches the latest;
    /// managers with a dedicated upgrade command override this.
    fn update(&self, package: &str) -> Result<InstalledPackage> {
        self.install(package)
    }

    /// Refresh this platform's cached package list. Returns whether anything
    /// was refreshed. Defaults to a no-op (`false`) for platforms that resolve
    /// on demand and keep no local index yet.
    fn refresh(&self) -> Result<bool> {
        Ok(false)
    }
}

/// Look up an executable on `PATH` without spawning a process — the native
/// equivalent of `which`, used for backend detection. On Windows an `.exe`
/// suffix is also considered.
pub fn command_exists(cmd: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        let candidate = dir.join(cmd);
        candidate.is_file() || candidate.with_extension("exe").is_file()
    })
}

/// The set of backends Pulse knows about, in preference order.
pub struct Registry {
    backends: Vec<Box<dyn Backend>>,
}

impl Registry {
    /// Every backend Pulse ships, whether or not it's usable here.
    pub fn all() -> Self {
        Registry {
            backends: vec![
                Box::new(Homebrew),
                Box::new(Apt),
                Box::new(Dnf),
                Box::new(Pacman),
                Box::new(Winget),
                Box::new(Msys2),
                Box::new(Macports),
            ],
        }
    }

    /// All known backends, regardless of availability.
    pub fn backends(&self) -> Vec<&dyn Backend> {
        self.backends.iter().map(Box::as_ref).collect()
    }

    /// Only the backends usable on this machine right now.
    pub fn available(&self) -> Vec<&dyn Backend> {
        self.backends
            .iter()
            .map(Box::as_ref)
            .filter(|b| b.is_available())
            .collect()
    }

    /// The preferred system manager for this machine — the first available
    /// one. The direct-binary installer is intentionally excluded; reach it
    /// with [`Registry::direct`].
    pub fn primary(&self) -> Option<&dyn Backend> {
        self.available().into_iter().next()
    }

    /// The direct-binary installer, which is always available.
    pub fn direct(&self) -> Direct {
        Direct
    }

    /// Look up a specific source by its identifier, including the always-on
    /// `"direct"` and `"registry"` sources.
    pub fn get(&self, name: &str) -> Option<Box<dyn Backend>> {
        match name {
            "direct" => Some(Box::new(Direct)),
            "registry" => Some(Box::new(crate::registry::RegistrySource)),
            "homebrew" => Some(Box::new(Homebrew)),
            "apt" => Some(Box::new(Apt)),
            "dnf" => Some(Box::new(Dnf)),
            "pacman" => Some(Box::new(Pacman)),
            "winget" => Some(Box::new(Winget)),
            "msys2" => Some(Box::new(Msys2)),
            "macports" => Some(Box::new(Macports)),
            _ => None,
        }
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::all()
    }
}
