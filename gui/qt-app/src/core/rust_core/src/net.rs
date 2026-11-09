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
        .and_then(|v| {
            v.get("network_enabled")
                .and_then(serde_json::Value::as_bool)
        })
        .unwrap_or(true)
}

fn ensure_allowed() -> Result<()> {
    if network_enabled() {
        Ok(())
    } else {
        Err(anyhow!(
            "network is disabled (network_enabled=false or KTM_OFFLINE=1)"
        ))
    }
}

/// Connection and *idle* timeouts, not a deadline for the whole transfer: a
/// large store archive (an icon theme of tens of MB) on a slow link keeps
/// downloading as long as bytes keep coming.
fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(30))
        .timeout_write(Duration::from_secs(30))
        .build()
}

/// GET returning at most `limit` bytes (error if larger).
pub fn http_get_bytes(url: &str, limit: u64) -> Result<Vec<u8>> {
    ensure_allowed()?;
    crate::dependency_jobs::event("downloading", "Connecting to the source");
    // Stores rate-limit bursts of downloads (KDE Store's file server answers
    // 429 after a few dozen): wait (Retry-After, at most 30 s; else 5 → 15 s)
    // and retry twice — about a minute at worst, then a clear error.
    let mut attempt = 0;
    let response = loop {
        match agent().get(url).set("User-Agent", "kolira").call() {
            Ok(r) => break r,
            Err(ureq::Error::Status(429, r)) if attempt < 2 => {
                let wait = r
                    .header("Retry-After")
                    .and_then(|v| v.trim().parse::<u64>().ok())
                    .unwrap_or([5, 15][attempt])
                    .min(30);
                attempt += 1;
                std::thread::sleep(Duration::from_secs(wait));
            }
            Err(ureq::Error::Status(429, _)) => {
                return Err(anyhow!(
                    "the server is rate-limiting downloads (429) — try again in a few minutes"
                ));
            }
            Err(e) => return Err(anyhow::Error::new(e).context(format!("GET {url}"))),
        }
    };
    let mut body = Vec::new();
    let mut reader = response.into_reader().take(limit + 1);
    let mut chunk = [0u8; 65536];
    let mut reported = std::time::Instant::now();
    loop {
        let count = reader.read(&mut chunk).context("reading response")?;
        if count == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..count]);
        if reported.elapsed() >= Duration::from_millis(500) {
            crate::dependency_jobs::event(
                "downloading",
                &format!("{} KiB received", body.len() / 1024),
            );
            reported = std::time::Instant::now();
        }
    }
    if body.len() as u64 > limit {
        return Err(anyhow!("response from {url} is larger than {limit} bytes"));
    }
    Ok(body)
}

pub fn http_get(url: &str, limit: u64) -> Result<String> {
    String::from_utf8(http_get_bytes(url, limit)?).context("response is not UTF-8")
}
