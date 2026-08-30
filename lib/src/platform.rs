//! Identifying the current platform, and matching release assets to it. Shared
//! by the direct-binary installer (arbitrary third-party releases) and the
//! self-updater (Pulse's own releases, which use canonical names).

use serde_json::Value;

/// Linux distro identifiers from `/etc/os-release` (`ID` plus `ID_LIKE`
/// tokens), lowercased — e.g. `["ubuntu", "debian"]`. Empty off Linux.
pub fn distro_ids() -> Vec<String> {
    #[cfg(target_os = "linux")]
    {
        let mut ids = Vec::new();
        if let Ok(text) = std::fs::read_to_string("/etc/os-release") {
            for line in text.lines() {
                if let Some(v) = line.strip_prefix("ID=") {
                    ids.push(unquote(v).to_lowercase());
                } else if let Some(v) = line.strip_prefix("ID_LIKE=") {
                    ids.extend(unquote(v).split_whitespace().map(|t| t.to_lowercase()));
                }
            }
        }
        ids
    }
    #[cfg(not(target_os = "linux"))]
    {
        Vec::new()
    }
}

/// Whether the running Linux distro belongs to a family (matches `ID` or an
/// `ID_LIKE` token), e.g. `distro_is("debian")`.
pub fn distro_is(family: &str) -> bool {
    distro_ids().iter().any(|id| id == family)
}

#[cfg(target_os = "linux")]
fn unquote(s: &str) -> &str {
    s.trim().trim_matches('"').trim_matches('\'')
}

/// Canonical OS label used in Pulse's own release asset names.
pub fn canonical_os() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        _ => "linux",
    }
}

/// Canonical architecture label used in Pulse's own release asset names.
pub fn canonical_arch() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "arm64",
        _ => "x64",
    }
}

/// The base name of Pulse's own release asset for this platform, e.g.
/// `pulse-macos-arm64`. The archive extension is appended by the caller.
pub fn asset_basename() -> String {
    format!("pulse-{}-{}", canonical_os(), canonical_arch())
}

/// OS name fragments that may appear in a third-party release asset name.
pub fn os_tokens() -> Vec<&'static str> {
    match std::env::consts::OS {
        "macos" => vec!["macos", "darwin", "apple", "osx", "mac"],
        "linux" => vec!["linux"],
        "windows" => vec!["windows", "win"],
        other => vec![other],
    }
}

/// Architecture fragments that may appear in a third-party release asset name.
pub fn arch_tokens() -> Vec<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => vec!["x86_64", "amd64", "x64", "x86-64"],
        "aarch64" => vec!["aarch64", "arm64"],
        other => vec![other],
    }
}

/// Choose the release asset that best fits the current platform: it must match
/// the OS and (ideally) the architecture, and not be a checksum or signature.
pub fn select_asset(assets: &[Value]) -> Option<Value> {
    let names: Vec<(String, &Value)> = assets
        .iter()
        .filter_map(|a| Some((a["name"].as_str()?.to_lowercase(), a)))
        .collect();

    let os = os_tokens();
    let arch = arch_tokens();
    let is_aux = |n: &str| {
        [
            ".sha256", ".sha512", ".sha1", ".md5", ".asc", ".sig", ".pem", ".txt", ".sbom", ".json",
        ]
        .iter()
        .any(|ext| n.ends_with(ext))
    };
    let os_match = |n: &str| os.iter().any(|o| n.contains(o));
    let arch_match = |n: &str| arch.iter().any(|a| n.contains(a));

    let mut candidates: Vec<&(String, &Value)> = names
        .iter()
        .filter(|(n, _)| !is_aux(n) && os_match(n) && arch_match(n))
        .collect();
    if candidates.is_empty() {
        candidates = names
            .iter()
            .filter(|(n, _)| !is_aux(n) && os_match(n))
            .collect();
    }
    if candidates.is_empty() && names.len() == 1 {
        candidates = names.iter().collect();
    }

    candidates.sort_by_key(|(n, _)| {
        if n.ends_with(".tar.gz") || n.ends_with(".tgz") || n.ends_with(".zip") {
            0
        } else {
            1
        }
    });
    candidates.first().map(|(_, a)| (*a).clone())
}
