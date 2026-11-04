//! Network access shared by the gallery and the app theme stores.
//!
//! Every request goes through here, so `network_enabled` (setting
//! `"network_enabled": false` in app-preferences.json, or `KTM_OFFLINE=1`)
//! is honoured everywhere, and responses are size-capped.

use std::io::Read;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

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

fn ensure_allowed() -> Result<()> {
    if network_enabled() {
        Ok(())
    } else {
        Err(anyhow!("network is disabled (network_enabled=false or KTM_OFFLINE=1)"))
    }
}

/// GET returning at most `limit` bytes (error if larger).
pub fn http_get_bytes(url: &str, limit: u64) -> Result<Vec<u8>> {
    ensure_allowed()?;
    let response = ureq::get(url)
        .set("User-Agent", "theme-profile-kit")
        .timeout(Duration::from_secs(30))
        .call()
        .with_context(|| format!("GET {url}"))?;
    let mut body = Vec::new();
    response
        .into_reader()
        .take(limit + 1)
        .read_to_end(&mut body)
        .context("reading response")?;
    if body.len() as u64 > limit {
        return Err(anyhow!("response from {url} is larger than {limit} bytes"));
    }
    Ok(body)
}

pub fn http_get(url: &str, limit: u64) -> Result<String> {
    String::from_utf8(http_get_bytes(url, limit)?).context("response is not UTF-8")
}
