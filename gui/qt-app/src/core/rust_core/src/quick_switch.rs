//! Quick Switch — preferences and the cheap theme feed for the overlay.
//!
//! The overlay is a separate process (`kolira-quick-switch`). It must open
//! fast, so it must not pay for the full theme summary: `card` reads
//! `theme.json` and the theme's own wallpaper and nothing else — no icon
//! probing, no directory walk, no Live reads. `profile.summaries` stays the
//! source for the focused card.
//!
//! Preferences live in `app-preferences.json` as a top-level
//! `"quick_switch"` object, the same way `automation` does. Keeping them out
//! of `settings_defaults()` matters: the main GUI holds one `AppSettings`
//! snapshot and sends it back on every Save, so a key stored there can be
//! clobbered by a stale copy from a second process.

use std::{fs, path::Path, time::UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};

/// Used when the user never chose a shortcut.
pub const DEFAULT_HOTKEY: &str = "Meta+K";
/// The desktop entry whose `[services][…]` group holds the shortcut.
pub const HOTKEY_APP: &str = "kolira-quick-switch.desktop";
/// `voxtype-toggle` currently owns `Meta+K`; it moves aside on the first
/// Quick Switch enable.
const VOXTYPE_APP: &str = "voxtype-toggle.desktop";
const VOXTYPE_FALLBACK_HOTKEY: &str = "Meta+Shift+K";

const SHAPES: &[&str] = &["rectangle", "rounded", "parallelogram"];
const SPEEDS: &[&str] = &["fast", "standard", "smooth"];
const REDUCED_MOTION: &[&str] = &["system", "on", "off"];

pub fn defaults() -> Value {
    json!({
        "enabled": false,
        "hotkey": DEFAULT_HOTKEY,
        "card_shape": "rounded",
        "card_scale": 1.0,
        "dim": 0.72,
        "glow": true,
        "animation_speed": "standard",
        "apply_animation": true,
        "reduced_motion": "system",
    })
}

fn read_settings() -> Map<String, Value> {
    fs::read_to_string(crate::paths::settings_file())
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

/// Stored Quick Switch object merged over the defaults (unknown keys kept).
fn stored() -> Value {
    let mut result = defaults();
    if let Some(Value::Object(values)) = read_settings().get("quick_switch").cloned() {
        for (key, value) in values {
            result[key] = value;
        }
    }
    result
}

pub fn get() -> Value {
    stored()
}

fn check_card_shape(value: &str) -> Result<()> {
    if SHAPES.contains(&value) {
        Ok(())
    } else {
        bail!(
            "Unknown card shape {value:?} (known: {})",
            SHAPES.join(", ")
        )
    }
}

fn check_speed(value: &str) -> Result<()> {
    if SPEEDS.contains(&value) {
        Ok(())
    } else {
        bail!(
            "Unknown animation speed {value:?} (known: {})",
            SPEEDS.join(", ")
        )
    }
}

fn check_reduced_motion(value: &str) -> Result<()> {
    if REDUCED_MOTION.contains(&value) {
        Ok(())
    } else {
        bail!(
            "Unknown reduced-motion mode {value:?} (known: {})",
            REDUCED_MOTION.join(", ")
        )
    }
}

/// KDE shortcut syntax (`Meta+K`, `Ctrl+Shift+T`, `none`).
pub fn valid_hotkey(key: &str) -> bool {
    let key = key.trim();
    if key.is_empty() || key.eq_ignore_ascii_case("none") || key == "-" {
        return true;
    }
    let mut parts = key.split('+').map(str::trim);
    let Some(last) = parts.next_back() else {
        return false;
    };
    if last.is_empty() {
        return false;
    }
    parts.all(|modifier| {
        matches!(
            modifier.to_ascii_lowercase().as_str(),
            "meta" | "alt" | "ctrl" | "control" | "shift"
        )
    })
}

/// Validate and merge `update` into the stored object.
fn validate(update: &Map<String, Value>) -> Result<()> {
    for (key, value) in update {
        match key.as_str() {
            "enabled" | "glow" | "apply_animation" => {
                if !value.is_boolean() {
                    bail!("Invalid quick_switch.{key}: expected bool");
                }
            }
            "hotkey" => {
                let Some(key_text) = value.as_str() else {
                    bail!("Invalid quick_switch.{key}: expected string");
                };
                if !valid_hotkey(key_text) {
                    bail!("Invalid shortcut {key_text:?}");
                }
            }
            "card_shape" => check_card_shape(str_value(value, key)?)?,
            "animation_speed" => check_speed(str_value(value, key)?)?,
            "reduced_motion" => check_reduced_motion(str_value(value, key)?)?,
            "card_scale" => {
                let number = value
                    .as_f64()
                    .filter(|n| (0.8..=1.2).contains(n))
                    .ok_or_else(|| {
                        anyhow::anyhow!("Invalid quick_switch.{key}: expected 0.8–1.2")
                    })?;
                let _ = number;
            }
            "dim" => {
                let number = value
                    .as_f64()
                    .filter(|n| (0.0..=1.0).contains(n))
                    .ok_or_else(|| {
                        anyhow::anyhow!("Invalid quick_switch.{key}: expected 0.0–1.0")
                    })?;
                let _ = number;
            }
            _ => {}
        }
    }
    Ok(())
}

fn str_value<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid quick_switch.{key}: expected string"))
}

/// Store `update` (other preferences and unknown keys are kept).
pub fn set(update: &Value) -> Result<Value> {
    let update = update
        .as_object()
        .context("quick_switch update must be an object")?;
    validate(update)?;

    let _lock = crate::gui_api::lock_file(&crate::paths::theme_root().join("preferences.lock"))?;
    let mut settings = read_settings();
    let mut current = settings
        .get("quick_switch")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (key, value) in update {
        current.insert(key.clone(), value.clone());
    }
    let merged = Value::Object(current);
    // Defaults fill what the user never touched, so the answer is complete.
    let mut result = defaults();
    if let Value::Object(values) = &merged {
        for (key, value) in values {
            result[key] = value.clone();
        }
    }
    settings.insert("quick_switch".into(), merged);
    crate::gui_api::write_bytes(
        &crate::paths::settings_file(),
        &(serde_json::to_string_pretty(&Value::Object(settings))? + "\n").into_bytes(),
        Some(0o600),
    )?;
    Ok(result)
}

/// Where the cached, downscaled wallpaper for a card lives.
fn thumb_path(name: &str, source: &Path) -> Option<std::path::PathBuf> {
    let metadata = fs::metadata(source).ok()?;
    let stamp = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let dir = crate::paths::cache_home()
        .join("kolira")
        .join("quick-switch")
        .join("thumbs");
    Some(dir.join(format!("{}-{}-{}.jpg", name, metadata.len(), stamp)))
}

/// Downscaled wallpaper for one card, cached on disk.
///
/// Cards only paint ~640×520, so decoding a 4K wallpaper on every open is
/// what made the overlay feel slow. The first call renders a small JPEG with
/// ImageMagick (the same external tool the wallpaper picker uses); later
/// calls and later processes just read it.
pub fn thumbnail(name: &str) -> Result<Value> {
    let dir = crate::paths::profile_dir(name)?;
    let package = crate::theme::load(&dir)?;
    let relative = package
        .wallpaper
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("theme {name:?} has no wallpaper"))?;
    let preview = crate::bundle_view::preview_image(&dir, &dir.join(relative))
        .ok_or_else(|| anyhow::anyhow!("wallpaper of {name:?} is not readable"))?;
    let source = std::path::PathBuf::from(preview["path"].as_str().unwrap_or_default());
    let cached = thumb_path(name, &source)
        .ok_or_else(|| anyhow::anyhow!("cannot read the wallpaper metadata"))?;
    if cached.is_file() {
        return Ok(json!({"path": cached, "revision": preview["revision"]}));
    }
    if let Some(parent) = cached.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = cached.with_extension("tmp.jpg");
    let output = std::process::Command::new("magick")
        .arg(&source)
        .args(["-auto-orient", "-resize", "720x", "-quality", "82"])
        .arg(&temporary)
        .output()
        .context("cannot run magick for the card thumbnail")?;
    if !output.status.success() {
        let _ = fs::remove_file(&temporary);
        bail!(
            "magick failed for {name:?}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    fs::rename(&temporary, &cached).context("cannot store the card thumbnail")?;
    Ok(json!({"path": cached, "revision": preview["revision"]}))
}

/// One carousel card: palette, wallpaper and nothing that needs a probe.
fn card(name: &str, dir: &Path, active: &str) -> Result<Value> {
    let package = if crate::theme::is_package(dir) {
        Some(crate::theme::load(dir)?)
    } else {
        None
    };
    let base = crate::field::Base::Snapshot(dir.to_path_buf());
    let read = |id: &str| -> Value {
        let Some(definition) = crate::field::field(id) else {
            return Value::Null;
        };
        let raw = match &package {
            Some(package) => package.values.get(id).cloned(),
            None => crate::field::read_raw(definition, &base),
        };
        raw.and_then(|value| crate::field::coerce(definition, value))
            .unwrap_or(Value::Null)
    };
    let display = |id: &str| {
        let value = read(id);
        if value.is_null() {
            String::new()
        } else if crate::field::field(id).is_some_and(|f| f.kind == "color") {
            crate::field::color_hex(&value)
        } else {
            crate::field::value_str(&value)
        }
    };
    let accent = {
        let accent = display("accent_color");
        if accent.is_empty() {
            display("selection_bg")
        } else {
            accent
        }
    };
    // What the card actually paints: with an accent taken from the wallpaper
    // the selection colour is the effective one (same rule as the gallery).
    let preview_accent = if read("accent_from_wallpaper") == Value::Bool(true) || accent.is_empty()
    {
        display("selection_bg")
    } else {
        accent.clone()
    };
    let window_bg = display("window_bg");
    let view_bg = display("view_bg");
    let text_color = display("text_color");
    let window_opacity = (read("translucency_enabled") == Value::Bool(true))
        .then(|| {
            read("translucency_inactive")
                .as_f64()
                .map(|n| n.clamp(0.0, 100.0) / 100.0)
        })
        .flatten();
    let radius = (display("decoration_library") == "org.kde.klassy")
        .then(|| read("klassy_radius").as_f64().map(|n| n.clamp(0.0, 48.0)))
        .flatten();
    let font = |id: &str| {
        let spec = display(id);
        let mut values = spec.split(',');
        let family = values.next().filter(|item| !item.is_empty());
        let points = values
            .next()
            .and_then(|item| item.parse::<f64>().ok())
            .filter(|size| (4.0..=72.0).contains(size));
        json!({"family":family,"points":points})
    };
    let swatches: Vec<String> = [
        window_bg.clone(),
        view_bg.clone(),
        preview_accent.clone(),
        text_color.clone(),
    ]
    .into_iter()
    .filter(|colour| !colour.is_empty())
    .collect();
    let palette = package
        .as_ref()
        .and_then(|package| package.palette.as_ref());
    let preview = package
        .as_ref()
        .and_then(|package| package.wallpaper.as_deref())
        .and_then(|relative| crate::bundle_view::preview_image(dir, &dir.join(relative)));
    // Already rendered and cached: free. Missing → the controller asks for it
    // once, in the background.
    let thumb_path = preview
        .as_ref()
        .and_then(|preview| preview["path"].as_str())
        .and_then(|path| thumb_path(name, Path::new(path)))
        .filter(|path| path.is_file());
    Ok(json!({
        "name": name,
        "active": name == active,
        "accent": preview_accent.clone(),
        "swatches": swatches,
        "previewColors": {
            "window": window_bg,
            "view": view_bg,
            "accent": preview_accent,
            "text": text_color,
        },
        "previewStyle": {
            "windowOpacity": window_opacity,
            "terminalOpacity": read("kitty_opacity").as_f64().map(|n| n.clamp(0.0, 1.0)),
            "radius": radius,
            "font": font("ui_font"),
            "titleFont": font("title_font"),
        },
        "iconTheme": display("icon_theme"),
        "variant": palette
            .map(|palette| match palette.variant {
                crate::palette::Variant::Dark => "dark",
                crate::palette::Variant::Light => "light",
            })
            .unwrap_or_default(),
        "paletteName": palette.map(|palette| palette.name.clone()).unwrap_or_default(),
        "wallpaperName": preview
            .as_ref()
            .and_then(|preview| preview["path"].as_str())
            .and_then(|path| {
                Path::new(path)
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
            }),
        "thumbPath": thumb_path.map(|path| path.to_string_lossy().to_string()),
        "preview": preview,
        "hasPreview": preview.is_some(),
    }))
}

/// The whole feed: every theme directory, cheapest-first data.
pub fn cards() -> Result<Value> {
    let active = crate::profile::get_active_profile().unwrap_or_default();
    let root = crate::paths::profiles_dir();
    let mut themes = Vec::new();
    if root.is_dir() {
        for entry in fs::read_dir(&root)?.flatten() {
            if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if crate::paths::validate_profile_name(&name).is_err() {
                continue;
            }
            match card(&name, &entry.path(), &active) {
                Ok(card) => themes.push(card),
                // A broken theme must not empty the carousel.
                Err(error) => themes.push(json!({"name": name, "error": error.to_string()})),
            }
        }
    }
    themes.sort_by(|a, b| {
        a["name"]
            .as_str()
            .unwrap_or_default()
            .cmp(b["name"].as_str().unwrap_or_default())
    });
    Ok(json!({"themes": themes, "active": active}))
}

/// The shortcut KDE shows in `[services][…]` groups.
fn launch_key(key: &str) -> String {
    let key = key.trim();
    if key.is_empty() || key.eq_ignore_ascii_case("none") || key == "-" {
        "none".to_string()
    } else {
        key.to_string()
    }
}

fn shortcut_file() -> std::path::PathBuf {
    crate::paths::config_home().join("kglobalshortcutsrc")
}

fn write_shortcut(app: &str, key: &str) -> Result<()> {
    let output = std::process::Command::new("kwriteconfig6")
        .arg("--file")
        .arg("kglobalshortcutsrc")
        .arg("--group")
        .arg("services")
        .arg("--group")
        .arg(app)
        .arg("--key")
        .arg("_launch")
        .arg(key)
        .output()
        .context("cannot run kwriteconfig6")?;
    if !output.status.success() {
        bail!(
            "kwriteconfig6 failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn current_shortcut(app: &str) -> String {
    let output = std::process::Command::new("kreadconfig6")
        .arg("--file")
        .arg("kglobalshortcutsrc")
        .arg("--group")
        .arg("services")
        .arg("--group")
        .arg(app)
        .arg("--key")
        .arg("_launch")
        .output()
        .ok();
    output
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default()
}

fn back_up_shortcut_file() -> Result<()> {
    let source = shortcut_file();
    if !source.is_file() {
        return Ok(());
    }
    let stamp = format!(
        "{}",
        fs::metadata(&source)?
            .modified()?
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default()
    );
    let dir = crate::paths::backups_dir().join("kglobalshortcutsrc");
    fs::create_dir_all(&dir)?;
    fs::copy(&source, dir.join(format!("kglobalshortcutsrc.{stamp}")))?;
    Ok(())
}

/// Point KDE's global shortcut at `key` (empty/`none` unbinds).
pub fn set_hotkey(key: &str) -> Result<Value> {
    if !valid_hotkey(key) {
        bail!("Invalid shortcut {key:?}");
    }
    back_up_shortcut_file()?;
    write_shortcut(HOTKEY_APP, &launch_key(key))?;
    // `Meta+K` used to belong to voxtype-toggle; move it aside once, only
    // when it really holds that key.
    if key.trim().eq_ignore_ascii_case(DEFAULT_HOTKEY)
        && current_shortcut(VOXTYPE_APP) == DEFAULT_HOTKEY
    {
        write_shortcut(VOXTYPE_APP, VOXTYPE_FALLBACK_HOTKEY)?;
    }
    Ok(json!({"hotkey": key.trim().to_string(), "shortcutFile": shortcut_file()}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkeys_parse() {
        for key in ["Meta+K", "Ctrl+Shift+T", "Alt+F4", "none", ""] {
            assert!(valid_hotkey(key), "{key:?} must be valid");
        }
        for key in ["Meta+", "Meta+K+", "+K", "Meta+Hyper+K"] {
            assert!(!valid_hotkey(key), "{key:?} must be rejected");
        }
        // A bare key is a legal KDE shortcut (`Print`, `Volume Up`).
        assert!(valid_hotkey("K"));
        assert!(valid_hotkey("Meta+K"));
    }

    #[test]
    fn shapes_and_speeds_are_checked() {
        assert!(SHAPES.contains(&"parallelogram"));
        assert!(check_card_shape("hexagon").is_err());
        assert!(check_speed("slow").is_err());
        assert!(check_reduced_motion("off").is_ok());
    }
}
