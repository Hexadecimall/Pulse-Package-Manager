//! winget backend (Windows Package Manager). Driven through the `winget` CLI.

use crate::backend::{Backend, Package, command_exists};
use crate::db::InstalledPackage;
use crate::process;
use anyhow::Result;

pub struct Winget;

impl Backend for Winget {
    fn name(&self) -> &'static str {
        "winget"
    }

    fn is_available(&self) -> bool {
        command_exists("winget")
    }

    fn search(&self, query: &str) -> Result<Vec<Package>> {
        // winget's table output is column-formatted with a "Name Id Version"
        // header and a dashed separator; take the Name and Id columns.
        let out = process::output("winget", &["search", query])?;
        let mut lines = out.lines().skip_while(|l| !l.starts_with("---"));
        lines.next(); // the "---" separator itself
        Ok(lines
            .filter(|l| !l.trim().is_empty())
            .filter_map(|line| {
                let mut cols = line.split_whitespace();
                let name = cols.next()?.to_string();
                let id = cols.next().map(str::to_string);
                let version = cols.next().map(str::to_string);
                Some(Package {
                    name,
                    version,
                    description: id.map(|id| format!("id: {id}")),
                    source: "winget".to_string(),
                })
            })
            .collect())
    }

    fn install(&self, package: &str) -> Result<InstalledPackage> {
        process::run(
            "winget",
            &[
                "install",
                "--accept-package-agreements",
                "--accept-source-agreements",
                package,
            ],
        )?;
        Ok(InstalledPackage::from_backend(
            package, "winget", package, None,
        ))
    }

    fn remove(&self, package: &str) -> Result<()> {
        process::run("winget", &["uninstall", package])
    }

    fn update(&self, package: &str) -> Result<InstalledPackage> {
        process::run(
            "winget",
            &[
                "upgrade",
                "--accept-package-agreements",
                "--accept-source-agreements",
                package,
            ],
        )?;
        Ok(InstalledPackage::from_backend(
            package, "winget", package, None,
        ))
    }
}
