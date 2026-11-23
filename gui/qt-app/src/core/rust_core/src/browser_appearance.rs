//! Browser profiles are personal: manage only colours and day/night preferences.
use crate::{
    field,
    palette::{contrast, Palette, Rgb},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const ROOTS: &[&str] = &[
    ".zen",
    ".config/zen",
    ".var/app/app.zen_browser.zen/.zen",
    ".mozilla/firefox",
    ".config/mozilla/firefox",
    ".var/app/org.mozilla.firefox/.mozilla/firefox",
];
pub const COLORS: &str = "kolira-colors.css";
pub const IMPORT: &str = "@import url(\"kolira-colors.css\");";
pub(crate) const ZEN_ACCENT: &str = "zen.theme.accent-color";
pub(crate) const ZEN_MODE: &str = "zen.view.window.scheme";
const ACCENT_PREFS: &[&str] = &[ZEN_ACCENT, "ui.accentcolor", "ui.accentcolortext"];
const MODE_PREFS: &[&str] = &[
    ZEN_MODE,
    "ui.systemUsesDarkTheme",
    "browser.theme.toolbar-theme",
    "browser.theme.content-theme",
    "layout.css.prefers-color-scheme.content-override",
];
pub const PREFS: &[&str] = &[
    ZEN_ACCENT,
    "ui.systemUsesDarkTheme",
    "ui.accentcolor",
    "ui.accentcolortext",
    ZEN_MODE,
    "browser.theme.toolbar-theme",
    "browser.theme.content-theme",
    "layout.css.prefers-color-scheme.content-override",
    "toolkit.legacyUserProfileCustomizations.stylesheets",
];

/// A field edited through the saved profile owns its saved value or its
/// deliberate omission even when Base16 generation is enabled.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ZenOverrides {
    #[serde(default)]
    pub accent: bool,
    #[serde(default)]
    pub mode: bool,
}

impl ZenOverrides {
    pub(crate) fn is_empty(&self) -> bool {
        !self.accent && !self.mode
    }
}

pub(crate) fn zen_roots() -> &'static [&'static str] {
    &ROOTS[..3]
}
/// Remove obsolete INI-style assignments for owned prefs: these are invalid JS.
/// Unknown lines and personal preferences remain untouched.
pub(crate) fn clean_legacy_assignments(text: &str) -> String {
    text.split_inclusive('\n')
        .filter(|line| {
            let Some((key, value)) = line.trim().split_once('=') else {
                return true;
            };
            !(PREFS.contains(&key.trim())
                && serde_json::from_str::<serde_json::Value>(value.trim().trim_end_matches(';'))
                    .is_ok())
        })
        .collect()
}

pub fn is_profile_file(rel: &str) -> bool {
    ROOTS
        .iter()
        .any(|root| rel.strip_prefix(root).is_some_and(|s| s.starts_with('/')))
}
pub fn is_zen(rel: &str) -> bool {
    zen_roots()
        .iter()
        .any(|root| rel.starts_with(&format!("{root}/")))
}
pub fn host(live: &str) -> String {
    if live.lines().any(|line| line.trim() == IMPORT) {
        live.into()
    } else {
        format!("{IMPORT}\n{live}")
    }
}
/// Native LookAndFeel accent preferences; never enable or patch userChrome.
pub fn accent(text: &str, p: &Palette, zen: bool) -> String {
    accent_pair(
        text,
        p.accent_rgb(),
        crate::palette::kde_roles(p).selection_text,
        zen,
    )
}

fn accent_pair(text: &str, accent: Rgb, on_accent: Rgb, zen: bool) -> String {
    let clean = clean_legacy_assignments(text);
    let color = json!(accent.hex());
    let out = field::userjs_set(&clean, "ui.accentcolor", &color);
    let out = field::userjs_set(&out, "ui.accentcolortext", &json!(on_accent.hex()));
    if zen {
        field::userjs_set(&out, ZEN_ACCENT, &color)
    } else {
        out
    }
}

pub(crate) fn set_zen_accent(text: &str, accent: Option<Rgb>) -> String {
    if let Some(accent) = accent {
        let black = Rgb(0, 0, 0);
        let white = Rgb(255, 255, 255);
        let on_accent = if contrast(black, accent) >= contrast(white, accent) {
            black
        } else {
            white
        };
        accent_pair(text, accent, on_accent, true)
    } else {
        remove_prefs(text, ACCENT_PREFS)
    }
}

pub(crate) fn set_zen_mode(text: &str, dark: Option<bool>) -> String {
    match dark {
        Some(dark) => mode(text, dark, true),
        None => remove_prefs(text, MODE_PREFS),
    }
}

fn remove_prefs(text: &str, keys: &[&str]) -> String {
    keys.iter()
        .fold(clean_legacy_assignments(text), |text, key| {
            field::userjs_remove(&text, key)
        })
}

pub(crate) fn overlay_prefs(theme: &str, live: &str, keys: &[&str]) -> String {
    keys.iter()
        .filter_map(|key| field::userjs_get(theme, key).map(|value| (key, value)))
        .fold(clean_legacy_assignments(live), |text, (key, value)| {
            field::userjs_set(&text, key, &value)
        })
}

pub(crate) fn zen_accent(text: &str) -> Option<Rgb> {
    field::userjs_get(text, ZEN_ACCENT)
        .or_else(|| field::userjs_get(text, "ui.accentcolor"))
        .and_then(|value| Rgb::parse(value.as_str()?).ok())
}

pub(crate) fn zen_mode(text: &str) -> Option<bool> {
    match field::userjs_get(text, ZEN_MODE).and_then(|value: Value| value.as_u64()) {
        Some(0) => Some(true),
        Some(1) => Some(false),
        _ => None,
    }
}

pub fn mode(text: &str, dark: bool, zen: bool) -> String {
    let mode = if dark { 0 } else { 1 };
    let mut out = field::userjs_set(
        text,
        "ui.systemUsesDarkTheme",
        &serde_json::json!(if dark { 1 } else { 0 }),
    );
    for key in [
        "browser.theme.toolbar-theme",
        "browser.theme.content-theme",
        "layout.css.prefers-color-scheme.content-override",
    ] {
        out = field::userjs_set(&out, key, &serde_json::json!(mode));
    }
    if zen {
        out = field::userjs_set(&out, ZEN_MODE, &serde_json::json!(mode));
    }
    out
}

pub fn is_colors(rel: &str) -> bool {
    is_profile_file(rel) && rel.ends_with("/chrome/kolira-colors.css")
}
/// Retire legacy managed chrome CSS without emitting any widget/colour rule.
pub fn safe_colors(text: &str) -> anyhow::Result<String> {
    const RETIRED: &str = "/* Kolira: legacy chrome colours retired; native accent and day/night live in user.js. */\n";
    if text == RETIRED {
        return Ok(RETIRED.into());
    }
    let read = |key: &str| -> anyhow::Result<crate::palette::Rgb> {
        let marker = format!("{key}:");
        let value = text
            .split_once(&marker)
            .and_then(|(_, s)| s.split(';').next())
            .map(str::trim)
            .ok_or_else(|| anyhow::anyhow!("Browser theme needs {key}"))?;
        crate::palette::Rgb::parse(value.trim_end_matches("!important").trim())
    };
    let bg = read("--toolbar-bgcolor")?;
    let mut fg = read("--toolbar-color")?;
    if crate::palette::contrast(bg, fg) < 4.5 {
        let light = crate::palette::Rgb(255, 255, 255);
        let dark = crate::palette::Rgb(0, 0, 0);
        fg = if crate::palette::contrast(bg, light) > crate::palette::contrast(bg, dark) {
            light
        } else {
            dark
        };
    }
    let _ = fg;
    Ok(RETIRED.into())
}
/// Enabling our colours must not silently activate an unrelated disabled UI skin.
pub fn check_stylesheet(
    profile: &std::path::Path,
    host: &str,
    plan: &mut crate::change_plan::Plan,
) -> anyhow::Result<()> {
    if host.trim().is_empty() || host.contains(IMPORT) {
        return Ok(());
    }
    let user = plan.text(&profile.join("user.js"))?.unwrap_or_default();
    let prefs = plan.text(&profile.join("prefs.js"))?.unwrap_or_default();
    let key = "toolkit.legacyUserProfileCustomizations.stylesheets";
    let enabled = field::userjs_get(&user, key).or_else(|| field::userjs_get(&prefs, key));
    anyhow::ensure!(
        enabled == Some(serde_json::json!(true)),
        "Existing browser CSS is disabled; enable it explicitly before adding a colour stylesheet"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_assignments_do_not_poison_browser_prefs() {
        let text = "ui.accentcolor=\"#ff78ac\"\nzen.view.window.scheme=1\n// personal\nuser_pref(\"privacy.example\", true);\nunknown.setting=1\n";
        let clean = clean_legacy_assignments(text);
        assert!(!clean.contains("ff78ac") && !clean.contains("scheme=1"));
        assert!(
            clean.contains("// personal")
                && clean.contains("privacy.example")
                && clean.contains("unknown.setting=1")
        );
        assert_eq!(clean_legacy_assignments(&clean), clean);
    }
    #[test]
    fn white_icons_on_white_and_layout_injections_are_removed() {
        let css=safe_colors(":root { --toolbar-bgcolor: #ffffff; --toolbar-color: #ffffff; } * { width: 1px; fill: white !important; }").unwrap();
        assert!(!css.contains("--toolbar") && !css.contains("fill:") && !css.contains("width"));
        assert!(safe_colors("* { display:none }").is_err());
    }
}
