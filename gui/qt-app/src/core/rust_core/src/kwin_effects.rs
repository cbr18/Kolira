//! Third-party KWin effects (Better Blur DX, Rounded Corners, glass, …).
//!
//! KWin writes `<effect>Enabled` into `[Plugins]` of kwinrc only when it
//! differs from the effect's `EnabledByDefault`. A theme that does not mention
//! an effect is therefore ambiguous — and an effect enabled by default
//! (Rounded Corners) would switch itself on in every older theme, with its
//! default settings. So:
//!
//! * **save** records the effective state of every installed third-party
//!   effect explicitly (`pin_states`);
//! * **apply** loads/unloads each installed effect as the theme says; an
//!   effect the theme does not know (it was saved before the effect was
//!   installed) is switched off (`sync`).
//!
//! Built into KWin (blur, contrast, wobbly windows…) are not handled here:
//! they keep KWin's own defaulting. Installed effects are found as compiled
//! plugins (`…/qt6/plugins/kwin/effects/plugins/<id>.so` — KWin's own effects
//! are built into KWin, not there) and as the user's scripted effects
//! (`~/.local/share/kwin/effects/<id>/metadata.json`; the system dir holds
//! KWin's and Plasma's own).

use std::fs;
use std::path::{Path, PathBuf};

use crate::paths::data_home;

const PLUGIN_DIRS: &[&str] = &["/usr/lib/qt6/plugins/kwin/effects/plugins", "/usr/local/lib/qt6/plugins/kwin/effects/plugins"];

/// An installed third-party effect: its id and where it lives.
#[derive(Debug, Clone, PartialEq)]
pub struct Effect {
    pub id: String,
    pub path: PathBuf,
}

fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

/// Installed third-party effects (user scripted effects first).
pub fn installed() -> Vec<Effect> {
    installed_in(&PLUGIN_DIRS.iter().map(PathBuf::from).collect::<Vec<_>>(), &script_roots())
}

/// Scripted effects of the user only: `/usr/share/kwin/effects` holds the ones
/// KWin and Plasma ship themselves (fade, login, cube…).
fn script_roots() -> Vec<PathBuf> {
    vec![data_home().join("kwin/effects")]
}

fn installed_in(plugin_dirs: &[PathBuf], script_dirs: &[PathBuf]) -> Vec<Effect> {
    let mut out: Vec<Effect> = Vec::new();
    let mut push = |id: String, path: PathBuf| {
        if safe_id(&id) && !out.iter().any(|e| e.id == id) {
            out.push(Effect { id, path });
        }
    };
    for dir in script_dirs {
        let Ok(entries) = fs::read_dir(dir) else { continue };
        let mut dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.join("metadata.json").is_file()).collect();
        dirs.sort();
        for d in dirs {
            if let Some(id) = d.file_name().and_then(|n| n.to_str()) {
                push(id.to_string(), d.clone());
            }
        }
    }
    for dir in plugin_dirs {
        let Ok(entries) = fs::read_dir(dir) else { continue };
        let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "so")).collect();
        files.sort();
        for f in files {
            if let Some(id) = f.file_stem().and_then(|n| n.to_str()) {
                push(id.to_string(), f.clone());
            }
        }
    }
    out
}

/// `EnabledByDefault` of an installed effect: the scripted effect's
/// metadata.json, or the CBOR metadata embedded in a compiled plugin
/// (`EnabledByDefault` followed by 0xF5 = true / 0xF4 = false).
pub fn enabled_by_default(effect: &Effect) -> bool {
    if effect.path.is_dir() {
        return fs::read_to_string(effect.path.join("metadata.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| v.pointer("/KPlugin/EnabledByDefault").and_then(serde_json::Value::as_bool))
            .unwrap_or(false);
    }
    fs::read(&effect.path).map(|bytes| cbor_enabled_by_default(&bytes)).unwrap_or(false)
}

fn cbor_enabled_by_default(bytes: &[u8]) -> bool {
    let key = b"EnabledByDefault";
    bytes.windows(key.len()).position(|w| w == key).and_then(|i| bytes.get(i + key.len())) == Some(&0xF5)
}

fn no_apply() -> bool {
    std::env::var("KTM_THEME_NO_APPLY").as_deref() == Ok("1")
}

fn qdbus(args: &[&str]) -> Option<String> {
    if no_apply() {
        return None;
    }
    let (status, out) = crate::paths::output_bounded(std::process::Command::new("qdbus6").args(args).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()))?;
    status.success().then(|| String::from_utf8_lossy(&out).trim().to_string())
}

/// Is `id` loaded in the running KWin? None when KWin cannot be asked.
fn loaded(id: &str) -> Option<bool> {
    qdbus(&["org.kde.KWin", "/Effects", "org.kde.kwin.Effects.isEffectLoaded", id]).map(|s| s == "true")
}

/// Save: write the effective state of every installed effect the snapshot's
/// kwinrc does not mention (the running KWin's state, else the default).
pub fn pin_states(snapshot_kwinrc: &Path) -> std::io::Result<()> {
    pin_states_with(snapshot_kwinrc, &installed(), |e| loaded(&e.id).unwrap_or_else(|| enabled_by_default(e)))
}

fn pin_states_with(snapshot_kwinrc: &Path, effects: &[Effect], state: impl Fn(&Effect) -> bool) -> std::io::Result<()> {
    if !snapshot_kwinrc.is_file() {
        return Ok(());
    }
    let mut text = fs::read_to_string(snapshot_kwinrc)?;
    let before = text.clone();
    for e in effects {
        let key = format!("{}Enabled", e.id);
        if crate::field::ini_get(&text, "Plugins", &key).is_none() {
            text = crate::field::set_ini_text(&text, "Plugins", &key, if state(e) { "true" } else { "false" });
        }
    }
    if text != before {
        fs::write(snapshot_kwinrc, text)?;
    }
    Ok(())
}

/// Apply: what the theme's kwinrc says for each installed effect — `true`,
/// `false`, or nothing (the theme predates it → off).
pub fn wanted(theme_kwinrc: &str, effects: &[Effect]) -> Vec<(String, bool)> {
    effects
        .iter()
        .map(|e| {
            let on = crate::field::ini_get(theme_kwinrc, "Plugins", &format!("{}Enabled", e.id)).is_some_and(|v| v.trim() == "true");
            (e.id.clone(), on)
        })
        .collect()
}

/// Apply the theme's effect states: write them into the live kwinrc (so an
/// effect enabled by default stays off after the next login too), then
/// load/unload and reconfigure them in the running KWin.
pub fn sync(theme_kwinrc: &Path) {
    let theme = fs::read_to_string(theme_kwinrc).unwrap_or_default();
    let live = crate::paths::config_home().join("kwinrc");
    let effects = installed();
    let states = wanted(&theme, &effects);
    if let Ok(mut text) = fs::read_to_string(&live) {
        let before = text.clone();
        for (id, on) in &states {
            text = crate::field::set_ini_text(&text, "Plugins", &format!("{id}Enabled"), if *on { "true" } else { "false" });
        }
        if text != before {
            let _ = fs::write(&live, text);
        }
    }
    for (id, on) in &states {
        qdbus(&["org.kde.KWin", "/Effects", if *on { "loadEffect" } else { "unloadEffect" }, id]);
        if *on {
            qdbus(&["org.kde.KWin", "/Effects", "reconfigureEffect", id]);
        }
    }
}

/// The installed effect `id`, if any (for dependency detection).
pub fn find(id: &str) -> Option<Effect> {
    installed().into_iter().find(|e| e.id == id)
}

/// Effects a theme's kwinrc enables that are third-party (not built into
/// KWin): `<id>Enabled=true` in `[Plugins]` for ids not built in. Without the
/// effect installed here the id is still reported (it is a dependency).
pub fn enabled_third_party(theme_kwinrc: &str) -> Vec<String> {
    let mut in_plugins = false;
    let mut out = Vec::new();
    for line in theme_kwinrc.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_plugins = t == "[Plugins]";
            continue;
        }
        let Some((key, value)) = t.split_once('=').filter(|_| in_plugins) else { continue };
        let Some(id) = key.trim().strip_suffix("Enabled") else { continue };
        if value.trim() == "true" && safe_id(id) && !builtin(id) {
            out.push(id.to_string());
        }
    }
    out
}

/// Effects shipped inside KWin itself (and KWin scripts, which are not
/// effects): never dependencies.
fn builtin(id: &str) -> bool {
    !(find(id).is_some() || KNOWN_THIRD_PARTY.contains(&id))
}

/// Third-party effects recognised even where they are not installed (so a
/// receiver sees them as dependencies).
const KNOWN_THIRD_PARTY: &[&str] = &["better_blur_dx", "kwin4_effect_shapecorners", "glass", "forceblur", "kwin_effect_lightlyshaders"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cbor_default_flag() {
        let mut so = b"...KPlugin\xa3gEnabledByDefault\xf5gLicense...".to_vec();
        assert!(cbor_enabled_by_default(&so));
        so = b"EnabledByDefault\xf4".to_vec();
        assert!(!cbor_enabled_by_default(&so));
        assert!(!cbor_enabled_by_default(b"nothing"));
    }

    #[test]
    fn finds_compiled_and_scripted_effects() {
        let tmp = tempfile::tempdir().unwrap();
        let plugins = tmp.path().join("plugins");
        let scripts = tmp.path().join("scripts");
        fs::create_dir_all(&plugins).unwrap();
        fs::create_dir_all(scripts.join("kwin4_effect_geometry_change")).unwrap();
        fs::write(plugins.join("kwin4_effect_shapecorners.so"), b"x").unwrap();
        fs::write(plugins.join("readme.txt"), b"x").unwrap();
        fs::write(scripts.join("kwin4_effect_geometry_change/metadata.json"), r#"{"KPlugin":{"EnabledByDefault":true}}"#).unwrap();
        let found = installed_in(&[plugins], &[scripts]);
        let ids: Vec<&str> = found.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["kwin4_effect_geometry_change", "kwin4_effect_shapecorners"]);
        assert!(enabled_by_default(&found[0]), "scripted metadata.json");
    }

    #[test]
    fn save_pins_and_apply_switches_off_unknown() {
        let tmp = tempfile::tempdir().unwrap();
        let rc = tmp.path().join("kwinrc");
        fs::write(&rc, "[Plugins]\nbetter_blur_dxEnabled=true\n").unwrap();
        let effects = vec![
            Effect { id: "better_blur_dx".into(), path: PathBuf::new() },
            Effect { id: "kwin4_effect_shapecorners".into(), path: PathBuf::new() },
        ];
        pin_states_with(&rc, &effects, |e| e.id == "kwin4_effect_shapecorners").unwrap();
        let text = fs::read_to_string(&rc).unwrap();
        assert!(text.contains("better_blur_dxEnabled=true") && text.contains("kwin4_effect_shapecornersEnabled=true"), "{text}");
        // An older theme that never heard of Rounded Corners turns it off.
        let older = "[Plugins]\nbetter_blur_dxEnabled=true\n";
        assert_eq!(
            wanted(older, &effects),
            vec![("better_blur_dx".to_string(), true), ("kwin4_effect_shapecorners".to_string(), false)]
        );
    }

    #[test]
    fn third_party_enabled_effects() {
        let rc = "[Plugins]\nkwin4_effect_shapecornersEnabled=true\nbetter_blur_dxEnabled=false\nwobblywindowsEnabled=true\n\n[Other]\nglassEnabled=true\n";
        assert_eq!(enabled_third_party(rc), vec!["kwin4_effect_shapecorners".to_string()]);
    }
}
