//! Zen Browser gradient theming.
//!
//! Zen stores its per-space theme (gradient colours, opacity, texture) inside
//! `zen-sessions.jsonlz4` — a Mozilla-flavoured LZ4 file (`mozLz40\0` magic +
//! little-endian uncompressed size + a single raw LZ4 block). The space named
//! "Space" carries a `theme` object of type `gradient`.
//!
//! `apply_zen_gradient` patches that theme for the active colour profile:
//!   * `sakura` / `sakura-*`  — two pinks, no texture
//!   * `paper` / `Paper`      — light beige + grain texture
//!   * `glass`                — left untouched (standard Zen gradient)
//!   * others                 — two colours derived from the live palette
//!
//! Zen must be closed before the file is written: a running Zen holds the
//! session in memory and overwrites the file on exit. When Zen is running the
//! patch is deferred to a detached child process that waits for Zen to exit.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};

use crate::palette::{Palette, Variant};
use crate::tracked_apps::zen_profile_dir;

/// Magic header of a Mozilla LZ4 file.
const MOZ_LZ4_MAGIC: &[u8; 8] = b"mozLz40\x00";

// ─────────────────────────────────────────────────────────────────────────────
// mozLz4 read / write
// ─────────────────────────────────────────────────────────────────────────────

/// Decompress a `mozLz40` file into raw bytes.
fn moz_lz4_decompress(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 12 || &data[..8] != MOZ_LZ4_MAGIC {
        return Err(anyhow!("not a mozLz4 file (bad magic)"));
    }
    let usize = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
    let block = &data[12..];
    let decompressed =
        lz4::block::decompress(block, Some(usize as i32)).context("lz4 decompression failed")?;
    Ok(decompressed)
}

/// Compress raw bytes into a `mozLz40` file.
fn moz_lz4_compress(raw: &[u8]) -> Result<Vec<u8>> {
    let compressed = lz4::block::compress(raw, None, false).context("lz4 compression failed")?;
    let mut out = Vec::with_capacity(12 + compressed.len());
    out.extend_from_slice(MOZ_LZ4_MAGIC);
    out.extend_from_slice(&(raw.len() as u32).to_le_bytes());
    out.extend_from_slice(&compressed);
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// Path helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Absolute path to the Zen sessions file for the detected profile.
pub fn zen_sessions_path() -> Option<PathBuf> {
    let dir = zen_profile_dir()?;
    let home = crate::paths::home_dir();
    Some(home.join(".zen").join(dir).join("zen-sessions.jsonlz4"))
}

/// True when a Zen process is currently running.
fn zen_is_running() -> bool {
    // The binary is `zen-browser` (wrapper) → `zen-bin` (actual process).
    Command::new("pgrep")
        .args(["-x", "zen-bin"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ─────────────────────────────────────────────────────────────────────────────
// Gradient definition
// ─────────────────────────────────────────────────────────────────────────────

/// A gradient colour stop for the Zen theme.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct GradientStop {
    /// RGB triple for the primary stop, or a hex string like `#ff80ab73` for
    /// custom stops (the trailing two hex digits are the alpha channel).
    colour: String,
    is_primary: bool,
}

/// Texture type + intensity for the Zen theme.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Texture {
    kind: String,
    intensity: f64,
}

impl Texture {
    fn none() -> Self {
        Texture {
            kind: "none".to_string(),
            intensity: 0.0,
        }
    }
    fn grain() -> Self {
        Texture {
            kind: "grain".to_string(),
            intensity: 0.125,
        }
    }
}

/// Full gradient spec for a profile.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct GradientSpec {
    stops: Vec<GradientStop>,
    opacity: f64,
    texture: Texture,
}

/// Return the gradient spec for a named profile, or `None` to skip (glass).
///
/// Matching is case-insensitive and tolerant of common suffixes, so both
/// `sakura`/`paper` and the deployed kolira names (`sakura-dream`, `Paper`)
/// resolve to the same spec.
fn spec_for_profile(profile: &str, palette: Option<&Palette>) -> Option<GradientSpec> {
    let name = profile.to_lowercase();
    let is = |needle: &str| {
        name == needle || name.starts_with(&format!("{needle}-")) || name.starts_with(needle)
    };

    match () {
        // Glass: default transparent — neutral dark translucent, no texture.
        _ if is("glass") => Some(GradientSpec {
            stops: vec![
                GradientStop {
                    colour: "30,30,30".into(),
                    is_primary: true,
                },
                GradientStop {
                    colour: "#1e1e1e73".into(),
                    is_primary: false,
                },
            ],
            opacity: 0.4,
            texture: Texture::none(),
        }),

        // Sakura: two pinks, no texture.
        _ if is("sakura") => Some(GradientSpec {
            stops: vec![
                GradientStop {
                    colour: "255,64,129".into(),
                    is_primary: true,
                }, // #FF4081
                GradientStop {
                    colour: "#FF80AB73".into(),
                    is_primary: false,
                }, // light pink ~45%
            ],
            opacity: 0.4,
            texture: Texture::none(),
        }),

        // Paper: light beige + grain (paper/canvas feel).
        _ if is("paper") => Some(GradientSpec {
            stops: vec![
                GradientStop {
                    colour: "245,235,220".into(),
                    is_primary: true,
                }, // #F5EBDC
                GradientStop {
                    colour: "#E8D5B773".into(),
                    is_primary: false,
                }, // warm beige ~45%
            ],
            opacity: 0.4,
            texture: Texture::grain(),
        }),

        // Any other profile: derive two colours from the live palette.
        _ => palette.map(spec_from_palette),
    }
}

/// Build a two-stop gradient from a palette: the window background (base00) as
/// the primary stop and the accent as a custom stop at ~45 % opacity. The
/// texture is grain on light palettes (paper feel) and none on dark ones.
fn spec_from_palette(p: &Palette) -> GradientSpec {
    let bg = p.slot(0x00);
    let accent = p.accent_rgb();
    let accent_hex = format!("#{:02x}{:02x}{:02x}73", accent.0, accent.1, accent.2);
    let texture = match p.variant {
        Variant::Light => Texture::grain(),
        Variant::Dark => Texture::none(),
    };
    GradientSpec {
        stops: vec![
            GradientStop {
                colour: format!("{},{},{}", bg.0, bg.1, bg.2),
                is_primary: true,
            },
            GradientStop {
                colour: accent_hex,
                is_primary: false,
            },
        ],
        opacity: 0.4,
        texture,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JSON manipulation
// ─────────────────────────────────────────────────────────────────────────────

/// Find the first space whose theme is a gradient and return a mutable
/// reference to it.
fn find_gradient_theme_mut(root: &mut Value) -> Option<&mut Value> {
    let spaces = root.get_mut("spaces")?.as_array_mut()?;
    for space in spaces {
        if let Some(theme) = space.get_mut("theme") {
            if theme.get("type").and_then(|t| t.as_str()) == Some("gradient") {
                return Some(theme);
            }
        }
    }
    None
}

/// Rewrite the gradient theme in the parsed JSON to match `spec`.
fn patch_gradient_json(root: &mut Value, spec: &GradientSpec) -> Result<()> {
    let theme = find_gradient_theme_mut(root)
        .ok_or_else(|| anyhow!("no gradient theme found in zen-sessions"))?;

    let stops: Vec<Value> = spec
        .stops
        .iter()
        .map(|s| {
            if s.is_primary {
                let rgb: Vec<u8> = s
                    .colour
                    .split(',')
                    .filter_map(|part| part.trim().parse::<u8>().ok())
                    .collect();
                json!({
                    "c": rgb,
                    "isCustom": false,
                    "algorithm": "floating",
                    "isPrimary": true,
                    "lightness": "91",
                    "position": {"x": 131, "y": 205},
                    "type": "explicit-lightness",
                })
            } else {
                json!({
                    "c": s.colour,
                    "isCustom": true,
                    "algorithm": "floating",
                    "isPrimary": false,
                    "lightness": "91",
                    "position": null,
                    "type": null,
                })
            }
        })
        .collect();

    theme["gradientColors"] = Value::Array(stops);
    theme["opacity"] = json!(spec.opacity);
    theme["textureType"] = json!(spec.texture.kind);
    theme["texture"] = json!(spec.texture.intensity);
    Ok(())
}

/// Read the sessions file, patch the gradient, and write it back atomically.
fn patch_sessions_file(path: &Path, spec: &GradientSpec) -> Result<()> {
    let compressed =
        std::fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    let raw = moz_lz4_decompress(&compressed)
        .with_context(|| format!("cannot decompress {}", path.display()))?;

    let mut root: Value = serde_json::from_slice(&raw)
        .with_context(|| format!("invalid JSON in {}", path.display()))?;

    patch_gradient_json(&mut root, spec)?;

    let new_raw = serde_json::to_vec(&root)?;
    let new_compressed = moz_lz4_compress(&new_raw)?;

    // Atomic write: temp file + rename, preserving permissions.
    let tmp = path.with_extension("jsonlz4.tmp");
    std::fs::write(&tmp, &new_compressed)?;
    if let Ok(meta) = std::fs::metadata(path) {
        let _ = std::fs::set_permissions(&tmp, meta.permissions());
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Apply the Zen gradient for `profile`, reading the palette from the live
/// location. Convenience wrapper around [`apply_zen_gradient_with_palette`].
pub fn apply_zen_gradient(profile: &str) -> Result<bool> {
    let palette = load_live_palette();
    apply_zen_gradient_with_palette(profile, palette.as_ref())
}

/// Apply the Zen gradient for `profile` using the given palette (read from the
/// profile directory by the caller). If Zen is running, a detached child
/// process waits for it to exit and then patches the file; the function returns
/// immediately. If Zen is not running the file is patched synchronously.
///
/// Returns `Ok(true)` when a patch was scheduled or applied, `Ok(false)` when
/// the sessions file is missing or `KTM_THEME_NO_APPLY=1`.
pub fn apply_zen_gradient_with_palette(profile: &str, palette: Option<&Palette>) -> Result<bool> {
    if no_apply() {
        return Ok(false);
    }

    let Some(spec) = spec_for_profile(profile, palette) else {
        return Ok(false);
    };

    let Some(path) = zen_sessions_path() else {
        return Ok(false);
    };
    if !path.is_file() {
        return Ok(false);
    }

    if zen_is_running() {
        spawn_deferred_patch(&path, &spec)?;
        Ok(true)
    } else {
        patch_sessions_file(&path, &spec)?;
        Ok(true)
    }
}

/// Load the live palette if it exists (best-effort).
fn load_live_palette() -> Option<Palette> {
    let path = crate::palette::live_palette_path();
    crate::palette::load(&path).ok()
}

fn no_apply() -> bool {
    std::env::var("KTM_THEME_NO_APPLY").as_deref() == Ok("1")
}

/// Spawn a detached child process that polls until Zen exits, then patches the
/// sessions file. The child is reparented to init so it survives the parent.
fn spawn_deferred_patch(path: &Path, spec: &GradientSpec) -> Result<()> {
    let spec_json = serde_json::to_string(spec).context("failed to serialise gradient spec")?;
    let path_str = path.to_string_lossy().to_string();

    let exe = std::env::current_exe().context("cannot determine current executable")?;

    Command::new(&exe)
        .args(["__zen-deferred-patch", &path_str, &spec_json])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("failed to spawn deferred Zen patch process")?;

    Ok(())
}

/// Wait for Zen to exit, then patch the sessions file. Called by the detached
/// child process spawned in [`spawn_deferred_patch`].
pub fn run_deferred_patch(path: &str, spec_json: &str) -> Result<()> {
    // Poll every second until Zen is gone (max 10 minutes).
    for _ in 0..600 {
        if !zen_is_running() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    // Small grace period so Zen can finish writing the file on exit.
    std::thread::sleep(std::time::Duration::from_millis(500));

    let spec: GradientSpec =
        serde_json::from_str(spec_json).context("invalid gradient spec JSON")?;
    patch_sessions_file(Path::new(path), &spec)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_sessions() -> Value {
        json!({
            "lastCollected": 1,
            "tabs": [],
            "spaces": [
                {
                    "name": "Space",
                    "theme": {
                        "type": "gradient",
                        "gradientColors": [
                            {"c": [65, 232, 124], "isCustom": false, "algorithm": "floating",
                             "isPrimary": true, "lightness": "91",
                             "position": {"x": 131, "y": 205}, "type": "explicit-lightness"},
                            {"c": "#00000073", "isCustom": true, "algorithm": "floating",
                             "isPrimary": false, "lightness": "91",
                             "position": null, "type": null}
                        ],
                        "opacity": 0.4,
                        "texture": 0,
                        "textureType": "none"
                    }
                }
            ]
        })
    }

    #[test]
    fn moz_lz4_roundtrip() {
        let raw = b"hello world hello world hello world";
        let compressed = moz_lz4_compress(raw).unwrap();
        assert_eq!(&compressed[..8], MOZ_LZ4_MAGIC);
        let decompressed = moz_lz4_decompress(&compressed).unwrap();
        assert_eq!(decompressed, raw);
    }

    #[test]
    fn patch_sakura_gradient() {
        let mut root = sample_sessions();
        let spec = spec_for_profile("sakura", None).unwrap();
        patch_gradient_json(&mut root, &spec).unwrap();

        let theme = &root["spaces"][0]["theme"];
        assert_eq!(theme["type"], "gradient");
        let stops = theme["gradientColors"].as_array().unwrap();
        assert_eq!(stops.len(), 2);
        assert_eq!(stops[0]["c"][0], 255);
        assert_eq!(stops[0]["c"][1], 64);
        assert_eq!(stops[0]["c"][2], 129);
        assert_eq!(stops[1]["c"], "#FF80AB73");
        assert_eq!(theme["textureType"], "none");
        assert_eq!(theme["texture"], 0.0);
    }

    #[test]
    fn patch_paper_gradient() {
        let mut root = sample_sessions();
        let spec = spec_for_profile("paper", None).unwrap();
        patch_gradient_json(&mut root, &spec).unwrap();

        let theme = &root["spaces"][0]["theme"];
        let stops = theme["gradientColors"].as_array().unwrap();
        assert_eq!(stops.len(), 2);
        assert_eq!(stops[0]["c"][0], 245);
        assert_eq!(theme["textureType"], "grain");
        assert_eq!(theme["texture"], 0.125);
    }

    #[test]
    fn glass_has_default_transparent() {
        let spec = spec_for_profile("glass", None).unwrap();
        assert_eq!(spec.stops[0].colour, "30,30,30");
        assert_eq!(spec.texture.kind, "none");
        assert!(spec_for_profile("Glass", None).is_some());
    }

    #[test]
    fn profile_name_variants_resolve() {
        assert!(spec_for_profile("sakura-dream", None).is_some());
        assert!(spec_for_profile("Paper", None).is_some());
        let s = spec_for_profile("sakura-dream", None).unwrap();
        assert_eq!(s.stops[0].colour, "255,64,129");
        let p = spec_for_profile("Paper", None).unwrap();
        assert_eq!(p.texture.kind, "grain");
    }

    #[test]
    fn palette_profile_derives_from_palette() {
        let p = crate::palette::parse_base16(
            "scheme: \"Test\"\nvariant: dark\n\
             base00: \"#1a1a2e\"\nbase01: \"#16213e\"\nbase02: \"#0f3460\"\n\
             base03: \"#533483\"\nbase04: \"#e94560\"\nbase05: \"#eee2dc\"\n\
             base06: \"#f5f5f5\"\nbase07: \"#ffffff\"\nbase08: \"#e94560\"\n\
             base09: \"#f39c12\"\nbase0A: \"#f1c40f\"\nbase0B: \"#2ecc71\"\n\
             base0C: \"#1abc9c\"\nbase0D: \"#3498db\"\nbase0E: \"#9b59b6\"\n\
             base0F: \"#e74c3c\"\naccent: \"#e94560\"\n",
            "test",
        )
        .unwrap();
        let spec = spec_for_profile("custom", Some(&p)).unwrap();
        assert_eq!(spec.stops.len(), 2);
        assert!(spec.stops[0].is_primary);
        assert_eq!(spec.stops[0].colour, "26,26,46");
        assert!(spec.stops[1].colour.starts_with("#e94560"));
        assert_eq!(spec.texture.kind, "none");
    }

    #[test]
    fn full_file_roundtrip() {
        let root = sample_sessions();
        let raw = serde_json::to_vec(&root).unwrap();
        let compressed = moz_lz4_compress(&raw).unwrap();
        let decompressed = moz_lz4_decompress(&compressed).unwrap();
        let back: Value = serde_json::from_slice(&decompressed).unwrap();
        assert_eq!(root, back);
    }
}
