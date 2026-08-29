//! Fetching things over the network — the foundation of installing binaries
//! directly, with no system package manager involved.

use anyhow::{Context, Result};
use std::path::Path;

/// A blocking HTTP client with a User-Agent set — GitHub's API rejects
/// requests without one.
fn client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent(concat!("pulse/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("building HTTP client")
}

/// Download a URL to a file on disk.
pub fn download(url: &str, dest: &Path) -> Result<()> {
    let bytes = client()?
        .get(url)
        .send()
        .with_context(|| format!("requesting {url}"))?
        .error_for_status()
        .with_context(|| format!("fetching {url}"))?
        .bytes()
        .context("reading response body")?;
    std::fs::write(dest, &bytes).with_context(|| format!("writing {}", dest.display()))?;
    Ok(())
}

/// Fetch a URL and parse the response as JSON.
pub fn get_json(url: &str) -> Result<serde_json::Value> {
    client()?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .with_context(|| format!("requesting {url}"))?
        .error_for_status()
        .with_context(|| format!("fetching {url}"))?
        .json()
        .with_context(|| format!("parsing JSON from {url}"))
}
