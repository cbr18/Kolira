//! base16 palette gallery: bundled schemes, a local cache and the online
//! tinted-theming catalogue (fetched only on request).
//!
//! * bundled — `data/base16/*.yaml`, compiled in, always available offline;
//! * cached  — schemes downloaded before (`$XDG_CACHE_HOME/theme-profile/base16/`);
//! * online  — the catalogue index (GitHub tree API, cached for a week) and the
//!   raw scheme files of `tinted-theming/schemes` (branch `spec-0.11`).
//!
//! Network use can be turned off with `"network_enabled": false` in
//! `app-preferences.json` or `KTM_OFFLINE=1` (tests); then only bundled and
//! cached schemes are listed.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Context, Result};

use crate::palette::{parse_base16, Palette, Variant};

const BRANCH: &str = "spec-0.11";
const INDEX_URL: &str =
    "https://api.github.com/repos/tinted-theming/schemes/git/trees/spec-0.11?recursive=1";
const INDEX_TTL: Duration = Duration::from_secs(7 * 24 * 3600);
const MAX_SCHEME_BYTES: u64 = 64 * 1024;
const MAX_INDEX_BYTES: u64 = 8 * 1024 * 1024;

/// Schemes compiled into the binary (offline set).
const BUNDLED: &[(&str, &str)] = &[
    ("catppuccin-latte", include_str!("../data/base16/catppuccin-latte.yaml")),
    ("catppuccin-mocha", include_str!("../data/base16/catppuccin-mocha.yaml")),
    ("dracula", include_str!("../data/base16/dracula.yaml")),
    ("everforest", include_str!("../data/base16/everforest.yaml")),
    ("gruvbox-dark-medium", include_str!("../data/base16/gruvbox-dark-medium.yaml")),
    ("gruvbox-light-medium", include_str!("../data/base16/gruvbox-light-medium.yaml")),
    ("kanagawa", include_str!("../data/base16/kanagawa.yaml")),
    ("nord", include_str!("../data/base16/nord.yaml")),
    ("one-light", include_str!("../data/base16/one-light.yaml")),
    ("onedark", include_str!("../data/base16/onedark.yaml")),
    ("rose-pine", include_str!("../data/base16/rose-pine.yaml")),
    ("rose-pine-dawn", include_str!("../data/base16/rose-pine-dawn.yaml")),
    ("solarized-dark", include_str!("../data/base16/solarized-dark.yaml")),
    ("solarized-light", include_str!("../data/base16/solarized-light.yaml")),
    ("tokyo-night-dark", include_str!("../data/base16/tokyo-night-dark.yaml")),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Bundled,
    Cached,
    Online,
}

impl Origin {
    pub fn as_str(self) -> &'static str {
        match self {
            Origin::Bundled => "bundled",
            Origin::Cached => "cached",
            Origin::Online => "online",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub id: String,
    pub origin: Origin,
    /// Known without downloading (bundled/cached); None for online-only ids.
    pub name: Option<String>,
    pub variant: Option<Variant>,
}

/// Scheme ids are file stems in the catalogue: `[a-z0-9][a-z0-9._-]*`.
pub fn validate_id(id: &str) -> Result<()> {
    let ok = !id.is_empty()
        && id.len() <= 80
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
        && !id.contains("..");
    if ok {
        Ok(())
    } else {
        Err(anyhow!("invalid scheme id {id:?}"))
    }
}

pub fn cache_dir() -> PathBuf {
    crate::paths::cache_home().join("theme-profile").join("base16")
}

/// Network allowed? Off via `KTM_OFFLINE=1` or `network_enabled: false`.
pub fn network_enabled() -> bool {
    if std::env::var("KTM_OFFLINE").as_deref() == Ok("1") {
        return false;
    }
    std::fs::read_to_string(crate::paths::settings_file())
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("network_enabled").and_then(serde_json::Value::as_bool))
        .unwrap_or(true)
}

fn source_label(id: &str) -> String {
    format!("gallery:{id}")
}

/// Ids from a GitHub tree listing (`base16/<id>.yaml` blobs).
pub fn parse_index(json: &str) -> Result<Vec<String>> {
    let value: serde_json::Value = serde_json::from_str(json).context("invalid catalogue index")?;
    let tree = value
        .get("tree")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow!("catalogue index has no tree"))?;
    let mut ids: Vec<String> = tree
        .iter()
        .filter_map(|item| item.get("path")?.as_str())
        .filter_map(|path| path.strip_prefix("base16/")?.strip_suffix(".yaml"))
        .filter(|id| validate_id(id).is_ok())
        .map(str::to_string)
        .collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

fn http_get(url: &str, limit: u64) -> Result<String> {
    use std::io::Read;
    let response = ureq::get(url)
        .set("User-Agent", "theme-profile-kit")
        .timeout(Duration::from_secs(20))
        .call()
        .with_context(|| format!("GET {url}"))?;
    let mut body = String::new();
    response
        .into_reader()
        .take(limit + 1)
        .read_to_string(&mut body)
        .context("reading response")?;
    if body.len() as u64 > limit {
        return Err(anyhow!("response from {url} is larger than {limit} bytes"));
    }
    Ok(body)
}

fn fresh(path: &Path) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < INDEX_TTL)
}

/// Catalogue ids: the cached index when fresh, else fetched (if allowed);
/// a stale index is still used when the network fails.
fn online_ids(dir: &Path, refresh: bool, network: bool) -> Result<Vec<String>> {
    let index = dir.join("index.json");
    if !refresh && fresh(&index) {
        return parse_index(&std::fs::read_to_string(&index)?);
    }
    if !network {
        return match std::fs::read_to_string(&index) {
            Ok(text) => parse_index(&text),
            Err(_) => Ok(Vec::new()),
        };
    }
    match http_get(INDEX_URL, MAX_INDEX_BYTES).and_then(|body| parse_index(&body).map(|ids| (body, ids))) {
        Ok((body, ids)) => {
            std::fs::create_dir_all(dir)?;
            std::fs::write(&index, body)?;
            Ok(ids)
        }
        Err(e) => match std::fs::read_to_string(&index) {
            Ok(text) => parse_index(&text),
            Err(_) => Err(e),
        },
    }
}

fn cached_scheme(dir: &Path, id: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(format!("{id}.yaml"))).ok()
}

/// List gallery schemes whose id or name contains `query` (case-insensitive).
/// `online` adds catalogue ids (network on request; cached index otherwise).
pub fn list(query: &str, online: bool, refresh: bool) -> Result<Vec<Entry>> {
    list_in(&cache_dir(), query, online, refresh, network_enabled())
}

fn list_in(dir: &Path, query: &str, online: bool, refresh: bool, network: bool) -> Result<Vec<Entry>> {
    let q = query.to_lowercase();
    let matches = |id: &str, name: Option<&str>| {
        q.is_empty() || id.contains(&q) || name.is_some_and(|n| n.to_lowercase().contains(&q))
    };
    let describe = |id: &str, text: &str, origin: Origin| {
        let parsed = parse_base16(text, &source_label(id)).ok();
        Entry {
            id: id.to_string(),
            origin,
            name: parsed.as_ref().map(|p| p.name.clone()),
            variant: parsed.map(|p| p.variant),
        }
    };

    let mut entries: Vec<Entry> = BUNDLED.iter().map(|(id, text)| describe(id, text, Origin::Bundled)).collect();
    if let Ok(read) = std::fs::read_dir(dir) {
        for item in read.flatten() {
            let file = item.file_name().to_string_lossy().to_string();
            let Some(id) = file.strip_suffix(".yaml") else { continue };
            if validate_id(id).is_err() || entries.iter().any(|e| e.id == id) {
                continue;
            }
            if let Some(text) = cached_scheme(dir, id) {
                entries.push(describe(id, &text, Origin::Cached));
            }
        }
    }
    if online {
        for id in online_ids(dir, refresh, network)? {
            if !entries.iter().any(|e| e.id == id) {
                entries.push(Entry { id, origin: Origin::Online, name: None, variant: None });
            }
        }
    }
    entries.retain(|e| matches(&e.id, e.name.as_deref()));
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(entries)
}

/// Palette for a gallery id: bundled → cached → downloaded (then cached).
pub fn fetch(id: &str) -> Result<Palette> {
    fetch_in(&cache_dir(), id, network_enabled())
}

fn fetch_in(dir: &Path, id: &str, network: bool) -> Result<Palette> {
    validate_id(id)?;
    if let Some((_, text)) = BUNDLED.iter().find(|(bid, _)| *bid == id) {
        return parse_base16(text, &source_label(id));
    }
    if let Some(text) = cached_scheme(dir, id) {
        return parse_base16(&text, &source_label(id));
    }
    if !network {
        return Err(anyhow!("scheme {id:?} is not available offline (network disabled)"));
    }
    let url = format!("https://raw.githubusercontent.com/tinted-theming/schemes/{BRANCH}/base16/{id}.yaml");
    let text = http_get(&url, MAX_SCHEME_BYTES)?;
    let palette = parse_base16(&text, &source_label(id))?; // validate before caching
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(format!("{id}.yaml")), &text)?;
    Ok(palette)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_schemes_all_parse() {
        for (id, text) in BUNDLED {
            let p = parse_base16(text, id).unwrap_or_else(|e| panic!("{id}: {e:#}"));
            assert!(!p.name.is_empty());
        }
        assert!(BUNDLED.windows(2).all(|w| w[0].0 < w[1].0), "keep BUNDLED sorted");
    }

    #[test]
    fn ids_are_validated() {
        for ok in ["everforest", "gruvbox-dark-medium", "0x96f", "a.b_c"] {
            assert!(validate_id(ok).is_ok(), "{ok}");
        }
        for bad in ["", "../etc", "a/b", "Upper", "-x", "a..b", "x y"] {
            assert!(validate_id(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn index_parsing_keeps_base16_yaml_only() {
        let json = r#"{"tree":[{"path":"base16/nord.yaml"},{"path":"base24/nord.yaml"},
            {"path":"base16/README.md"},{"path":"base16/../evil.yaml"},{"path":"base16/zen.yaml"}]}"#;
        assert_eq!(parse_index(json).unwrap(), vec!["nord", "zen"]);
        assert!(parse_index("{}").is_err());
    }

    #[test]
    fn offline_listing_and_fetch_use_bundled_and_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        // A cached scheme and a stale-or-not index are used without network.
        let mut cached = BUNDLED[0].1.replace("Catppuccin Latte", "My Cached");
        cached.push('\n');
        std::fs::write(dir.join("my-cached.yaml"), &cached).unwrap();
        std::fs::write(dir.join("index.json"), r#"{"tree":[{"path":"base16/zzz-online.yaml"}]}"#).unwrap();

        let all = list_in(dir, "", true, false, false).unwrap();
        assert!(all.iter().any(|e| e.id == "everforest" && e.origin == Origin::Bundled));
        assert!(all.iter().any(|e| e.id == "my-cached" && e.origin == Origin::Cached && e.name.as_deref() == Some("My Cached")));
        assert!(all.iter().any(|e| e.id == "zzz-online" && e.origin == Origin::Online));

        let q = list_in(dir, "ROSÉ", false, false, false).unwrap();
        assert!(q.iter().all(|e| e.id.starts_with("rose-pine")) && q.len() == 2, "{q:?}");

        assert_eq!(fetch_in(dir, "everforest", false).unwrap().source, "gallery:everforest");
        assert_eq!(fetch_in(dir, "my-cached", false).unwrap().name, "My Cached");
        assert!(fetch_in(dir, "zzz-online", false).is_err(), "offline: not downloadable");
        assert!(fetch_in(dir, "../x", true).is_err(), "id validated before any network use");
    }
}
