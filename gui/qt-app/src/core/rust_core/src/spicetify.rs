//! Spotify through Spicetify: `~/.config/spicetify/config-xpui.ini`
//! (`[Setting] current_theme`, `color_scheme`, …) selects a theme folder in
//! `Themes/<Name>/` (`color.ini` + `user.css`); `spicetify apply` patches
//! Spotify.
//!
//! * a theme carries `Themes/` and only the theme keys of `config-xpui.ini`;
//! * the palette generates `Themes/Kolira/color.ini` (matugen's mapping)
//!   and selects it;
//! * applying a theme runs `spicetify apply` when Spicetify is installed.

use std::fmt::Write as _;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::field::set_ini_text;
use crate::palette::{kde_roles, Palette, Rgb};
use crate::paths::config_home;

pub const DIR: &str = ".config/spicetify";
pub const CONFIG: &str = ".config/spicetify/config-xpui.ini";
pub const THEMES: &str = ".config/spicetify/Themes";
pub const THEME_NAME: &str = "Kolira";

pub fn config_paths() -> Vec<String> {
    if config_home().join("spicetify").exists() || which::which("spicetify").is_ok() {
        vec![CONFIG.to_string(), THEMES.to_string()]
    } else {
        Vec::new()
    }
}

fn hex(c: Rgb) -> String {
    c.hex().trim_start_matches('#').to_string()
}

/// `color.ini` for the palette (section `[Kolira]`).
pub fn color_ini(p: &Palette) -> String {
    let r = kde_roles(p);
    let mut out = format!(
        "; kolira: palette \"{}\" (generated)\n[{THEME_NAME}]\n",
        p.name.replace(['\n', '"'], "")
    );
    let mut key = |k: &str, c: Rgb| {
        let _ = writeln!(out, "{k:<18} = {}", hex(c));
    };
    key("main", r.window);
    key("main-secondary", r.window_alt);
    key("accent", r.accent);
    key("button", r.accent);
    key("button-secondary", r.button);
    key("button-active", r.accent.mix(r.text, 0.15));
    key("button-disabled", r.button_alt);
    key("misc", r.link);
    key("subtext", r.text_inactive);
    key("text", r.text);
    key("sidebar", r.window_alt);
    key("player", r.window_alt);
    key("card", r.view_alt);
    key("notification", r.view);
    key("notification-error", r.negative);
    key("shadow", r.window.mix(Rgb(0, 0, 0), 0.6));
    key("nav-active-text", r.selection_text);
    key("nav-active", r.accent);
    key("tab-active", r.view_alt);
    key("play-button", r.accent);
    key("playback-bar", r.accent);
    key("selected-row", r.text);
    key("highlight", r.hover.mix(r.window, 0.7));
    key("highlight-elevated", r.view_alt);
    out
}

/// Palette target `spicetify`: write the theme, select it, `spicetify apply`.
pub fn apply_palette(p: &Palette) -> Result<Option<PathBuf>> {
    let base = config_home().join("spicetify");
    let config = base.join("config-xpui.ini");
    if !config.is_file() {
        return Ok(None);
    }
    let dir = base.join("Themes").join(THEME_NAME);
    let mut plan = crate::change_plan::Plan::default();
    let color = dir.join("color.ini");
    crate::app_sync::write(&mut plan, &color, color_ini(p))?;
    if !dir.join("user.css").exists() {
        crate::app_sync::write(
            &mut plan,
            &dir.join("user.css"),
            "/* kolira: colours only (color.ini) */\n".into(),
        )?;
    }
    let mut text = plan.text(&config)?.context("Missing Spicetify config")?;
    for (k, v) in [
        ("current_theme", THEME_NAME),
        ("color_scheme", THEME_NAME),
        ("inject_css", "1"),
        ("replace_colors", "1"),
    ] {
        text = set_ini_text(&text, "Setting", k, v);
    }
    crate::app_sync::write(&mut plan, &config, text)?;
    if let Err(error) = plan.commit().and_then(|_| refresh()) {
        let rollback = crate::change_plan::Plan {
            before: plan.after.clone(),
            after: plan.before.clone(),
            ..Default::default()
        };
        rollback
            .commit()
            .context("Spicetify configuration rollback failed")?;
        return Err(error);
    }
    Ok(Some(color))
}

/// Imported themes cannot enable or supply executable Spotify extensions.
pub fn is_script(rel: &str) -> bool {
    std::path::Path::new(rel).starts_with(THEMES)
        && std::path::Path::new(rel).extension().is_some_and(|v| {
            ["js", "mjs", "cjs", "wasm", "ts"]
                .iter()
                .any(|ext| v.to_string_lossy().eq_ignore_ascii_case(ext))
        })
}
/// Directory snapshots are copied recursively elsewhere; apply the same script
/// exclusion here so a raw legacy Themes/ directory cannot bypass it.
pub(crate) fn plan_theme_tree(
    plan: &mut crate::change_plan::Plan,
    source: &std::path::Path,
    dest: &std::path::Path,
    rel: &str,
) -> Result<()> {
    if is_script(rel) {
        return Ok(());
    }
    if source.is_dir() && !source.is_symlink() {
        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            let name = entry.file_name();
            plan_theme_tree(
                plan,
                &entry.path(),
                &dest.join(&name),
                &format!("{rel}/{}", name.to_string_lossy()),
            )?;
        }
        Ok(())
    } else {
        plan.copy(source, dest, "apply")
    }
}
pub fn preflight(source: &std::path::Path) -> Result<()> {
    if source.join(CONFIG).is_file() {
        let text = std::fs::read_to_string(source.join(CONFIG))?;
        if let Some(name) = crate::field::ini_get(&text, "Setting", "current_theme") {
            let valid = name.is_empty()
                || (!name.contains(['/', '\\'])
                    && !name.chars().any(char::is_control)
                    && matches!(
                        std::path::Path::new(&name).components().next(),
                        Some(std::path::Component::Normal(_))
                    )
                    && std::path::Path::new(&name).components().count() == 1);
            anyhow::ensure!(
                valid,
                "Spicetify current_theme must be a single theme folder name"
            );
        }
    }

    if !crate::field::no_apply()
        && source.join(CONFIG).is_file()
        && which::which("spicetify").is_ok()
    {
        anyhow::ensure!(crate::paths::live_path(CONFIG).is_file(),"Spicetify must already be configured before applying its theme; refusing to create an untracked Spotify baseline");
    }
    Ok(())
}

/// Re-patch Spotify after the Spicetify config/theme changed.
pub fn refresh() -> Result<()> {
    if std::env::var("KTM_THEME_NO_APPLY").as_deref() == Ok("1")
        || which::which("spicetify").is_err()
    {
        return Ok(());
    }
    let status = crate::paths::status_bounded(
        std::process::Command::new("spicetify")
            .args(["apply", "-n"])
            .stderr(std::process::Stdio::null()),
    )
    .context("Spicetify apply timed out or could not start")?;
    anyhow::ensure!(
        status.success(),
        "Spicetify apply failed ({status}); Spotify runtime changes could not be verified"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_ini_has_the_section_and_keys() {
        let p = crate::palette::parse_base16(
            "scheme: \"T\"\nbase00: \"282a36\"\nbase01: \"21222c\"\nbase02: \"44475a\"\nbase03: \"6272a4\"\nbase04: \"9ea8c7\"\nbase05: \"f8f8f2\"\nbase06: \"f8f8f2\"\nbase07: \"ffffff\"\nbase08: \"ff5555\"\nbase09: \"ffb86c\"\nbase0A: \"f1fa8c\"\nbase0B: \"50fa7b\"\nbase0C: \"8be9fd\"\nbase0D: \"bd93f9\"\nbase0E: \"ff79c6\"\nbase0F: \"993333\"\n",
            "test",
        )
        .unwrap();
        let ini = color_ini(&p);
        assert!(ini.contains("[Kolira]"));
        assert!(ini.contains("main               = 282a36"));
        assert!(!ini.contains('#'));
    }
}
