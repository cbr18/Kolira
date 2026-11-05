//! Making a new theme (the "theme constructor", P2):
//!
//! * **`new NAME --from=BASE`** — start from a base: an existing theme
//!   (copied under the new name and applied) or a built-in neutral template
//!   (`neutral-light` / `neutral-dark`: Breeze everywhere, no blur, no
//!   translucency, opaque terminals) applied to the live session and saved as
//!   the new theme. The user then edits the live look and saves over it.
//! * **`look-and-feel NAME`** — a Look & Feel package for the current look,
//!   `theme-profile-<name>`: `metadata.json` + `contents/defaults` only (colour
//!   scheme, widget style, icons, cursor, Plasma style, window decoration) —
//!   never a layout, so applying it does not touch panels. It is selected
//!   (key only — the look is already live) and a save carries it as the
//!   theme's own asset.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::field::{self, Base};
use crate::paths::{data_home, validate_profile_name};

/// The neutral template's field values.
pub fn neutral(dark: bool) -> Vec<(String, Value)> {
    let pairs: Vec<(&str, Value)> = vec![
        ("look_and_feel", json!(if dark { "org.kde.breezedark.desktop" } else { "org.kde.breeze.desktop" })),
        ("color_scheme", json!(if dark { "BreezeDark" } else { "BreezeLight" })),
        ("widget_style", json!("Breeze")),
        ("plasma_theme", json!("default")),
        ("icon_theme", json!(if dark { "breeze-dark" } else { "breeze" })),
        ("cursor_theme", json!("breeze_cursors")),
        ("gtk_theme", json!("Breeze")),
        ("gtk_dark", json!(dark)),
        ("decoration_library", json!("org.kde.breeze")),
        ("decoration_theme", json!("Breeze")),
        ("blur_enabled", json!(false)),
        ("legacy_blur", json!(false)),
        ("translucency_enabled", json!(false)),
        ("kitty_opacity", json!(1.0)),
        ("alacritty_opacity", json!(1.0)),
        ("zen_transparent", json!(false)),
    ];
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

/// What `new` did.
pub struct NewTheme {
    pub name: String,
    pub base: String,
    pub notes: Vec<String>,
}

/// Start theme `name` from `base` (a theme name, `neutral-light` or
/// `neutral-dark`). Replaces the live look (applying makes a backup first).
pub fn new_theme(name: &str, base: &str, description: Option<String>) -> Result<NewTheme> {
    validate_profile_name(name)?;
    if crate::paths::profiles_dir().join(name).exists() {
        bail!("a theme named {name:?} already exists");
    }
    let mut notes = Vec::new();
    match base {
        "neutral-light" | "neutral-dark" => {
            let report = field::apply(&neutral(base == "neutral-dark"));
            notes.extend(report.errors.iter().map(|e| format!("neutral template: {e}")));
            let options = crate::theme::SaveOptions { description, include_wallpaper: true };
            crate::theme::save(name, &options)?;
        }
        theme => {
            validate_profile_name(theme)?;
            if !crate::paths::profiles_dir().join(theme).is_dir() {
                bail!("no theme named {theme:?}");
            }
            crate::profile::clone_profile(theme, name)?;
            if let Some(d) = description {
                crate::profile::set_profile_description(name, &d)?;
            }
            let report = crate::theme::apply(name, &crate::theme::ApplyOptions::default())?;
            notes.extend(report.errors);
        }
    }
    crate::profile::write_active(name)?;
    Ok(NewTheme { name: name.into(), base: base.into(), notes })
}

/// `theme-profile-<name>` for a theme name (lower-case, safe).
pub fn laf_id(name: &str) -> String {
    format!("theme-profile-{}", name.to_lowercase())
}

/// `contents/defaults` from the live look.
pub fn laf_defaults() -> String {
    let get = |id: &str| field::field(id).and_then(|f| field::read(f, &Base::Live)).map(|v| field::value_str(&v)).unwrap_or_default();
    let mut groups: Vec<(&str, Vec<(&str, String)>)> = vec![
        ("kdeglobals][General", vec![("ColorScheme", get("color_scheme"))]),
        ("kdeglobals][KDE", vec![("widgetStyle", get("widget_style"))]),
        ("kdeglobals][Icons", vec![("Theme", get("icon_theme"))]),
        ("kcminputrc][Mouse", vec![("cursorTheme", get("cursor_theme"))]),
        ("plasmarc][Theme", vec![("name", get("plasma_theme"))]),
        ("kwinrc][org.kde.kdecoration2", vec![("library", get("decoration_library")), ("theme", get("decoration_theme"))]),
    ];
    let mut out = String::new();
    for (group, keys) in groups.iter_mut() {
        keys.retain(|(_, v)| !v.is_empty());
        if keys.is_empty() {
            continue;
        }
        out.push_str(&format!("[{group}]\n"));
        for (k, v) in keys.iter() {
            out.push_str(&format!("{k}={v}\n"));
        }
        out.push('\n');
    }
    out
}

/// Write the Look & Feel package for the live look and select it (key only).
/// Returns its directory.
pub fn look_and_feel(name: &str) -> Result<PathBuf> {
    validate_profile_name(name)?;
    let id = laf_id(name);
    let dir = data_home().join("plasma/look-and-feel").join(&id);
    let contents = dir.join("contents");
    std::fs::create_dir_all(&contents)?;
    let metadata = json!({
        "KPackageStructure": "Plasma/LookAndFeel",
        "KPlugin": {
            "Id": id,
            "Name": name,
            "Description": format!("Look of the theme {name} (generated by theme-profile; no layout)"),
            "License": "MIT",
            "Authors": [{"Name": "theme-profile"}],
            "Version": "1.0"
        },
        "X-Plasma-APIVersion": "2"
    });
    std::fs::write(dir.join("metadata.json"), serde_json::to_string_pretty(&metadata)? + "\n")?;
    std::fs::write(contents.join("defaults"), laf_defaults()).context("cannot write defaults")?;
    // The look is live already: write only the key (no plasma-apply-lookandfeel).
    let report = field::apply_with(&[("look_and_feel".into(), Value::from(id.clone()))], &["look_and_feel"]);
    if let Some(e) = report.errors.first() {
        bail!("{e}");
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_template_has_no_glass() {
        for dark in [false, true] {
            let values = neutral(dark);
            let get = |k: &str| values.iter().find(|(id, _)| id == k).map(|(_, v)| v.clone());
            assert_eq!(get("blur_enabled"), Some(json!(false)));
            assert_eq!(get("translucency_enabled"), Some(json!(false)));
            assert_eq!(get("kitty_opacity"), Some(json!(1.0)));
            assert_eq!(get("widget_style"), Some(json!("Breeze")));
            assert_eq!(get("gtk_dark"), Some(json!(dark)));
            for (id, _) in &values {
                assert!(field::field(id).is_some(), "unknown field {id}");
            }
        }
    }

    #[test]
    fn laf_ids_are_safe() {
        assert_eq!(laf_id("Frutiger-Aero"), "theme-profile-frutiger-aero");
    }
}
