//! Browser profiles are personal: manage only colours and day/night preferences.
use crate::{field, palette::Palette};

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
pub const PREFS: &[&str] = &[
    "zen.theme.accent-color",
    "ui.systemUsesDarkTheme",
    "ui.accentcolor",
    "ui.accentcolortext",
    "zen.view.window.scheme",
    "browser.theme.toolbar-theme",
    "browser.theme.content-theme",
    "layout.css.prefers-color-scheme.content-override",
    "toolkit.legacyUserProfileCustomizations.stylesheets",
];
pub fn is_profile_file(rel: &str) -> bool {
    ROOTS
        .iter()
        .any(|root| rel.strip_prefix(root).is_some_and(|s| s.starts_with('/')))
}
pub fn is_zen(rel: &str) -> bool {
    ROOTS[..3]
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
    let roles = crate::palette::kde_roles(p);
    let mut out = field::userjs_set(
        text,
        "ui.accentcolor",
        &serde_json::json!(p.accent_rgb().hex()),
    );
    out = field::userjs_set(
        &out,
        "ui.accentcolortext",
        &serde_json::json!(roles.selection_text.hex()),
    );
    if zen {
        out = field::userjs_set(
            &out,
            "zen.theme.accent-color",
            &serde_json::json!(p.accent_rgb().hex()),
        );
    }
    out
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
        out = field::userjs_set(&out, "zen.view.window.scheme", &serde_json::json!(mode));
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
    fn white_icons_on_white_and_layout_injections_are_removed() {
        let css=safe_colors(":root { --toolbar-bgcolor: #ffffff; --toolbar-color: #ffffff; } * { width: 1px; fill: white !important; }").unwrap();
        assert!(!css.contains("--toolbar") && !css.contains("fill:") && !css.contains("width"));
        assert!(safe_colors("* { display:none }").is_err());
    }
}
