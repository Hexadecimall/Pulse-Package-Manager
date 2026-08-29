//! Pacman backend (Arch Linux and derivatives). `pacman -Ss` searches without
//! privileges; install/remove need root.

use crate::backend::{Backend, Package, command_exists};
use crate::db::InstalledPackage;
use crate::process;
use anyhow::Result;

pub struct Pacman;

impl Backend for Pacman {
    fn name(&self) -> &'static str {
        "pacman"
    }

    fn is_available(&self) -> bool {
        command_exists("pacman")
    }

    fn search(&self, query: &str) -> Result<Vec<Package>> {
        // `pacman -Ss` alternates lines: "repo/name version ..." then an
        // indented description on the next line.
        let out = process::output("pacman", &["-Ss", query])?;
        let mut packages = Vec::new();
        for line in out.lines() {
            if line.starts_with(char::is_whitespace) {
                // Description line — attach it to the last package.
                if let Some(last) = packages.last_mut() {
                    let p: &mut Package = last;
                    p.description = Some(line.trim().to_string());
                }
                continue;
            }
            let mut parts = line.split_whitespace();
            let Some(repo_name) = parts.next() else {
                continue;
            };
            let name = repo_name.split_once('/').map(|(_, n)| n).unwrap_or(repo_name);
            packages.push(Package {
                name: name.to_string(),
                version: parts.next().map(str::to_string),
                description: None,
                source: "pacman".to_string(),
            });
        }
        Ok(packages)
    }

    fn install(&self, package: &str) -> Result<InstalledPackage> {
        process::run_privileged("pacman", &["-S", "--noconfirm", package])?;
        Ok(InstalledPackage::from_backend(
            package, "pacman", package, None,
        ))
    }

    fn remove(&self, package: &str) -> Result<()> {
        process::run_privileged("pacman", &["-R", "--noconfirm", package])
    }
}
