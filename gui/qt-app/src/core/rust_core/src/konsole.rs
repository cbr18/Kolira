//! Konsole: the look is a colour scheme (`*.colorscheme`, also holding
//! opacity and blur) plus the `[Appearance]` of the default profile (scheme
//! name, font family — the size stays the user's).
//!
//! * the default profile comes from `konsolerc [Desktop Entry] DefaultProfile`;
//!   without one a "Theme Profile" is created (only `[Appearance]` + a name,
//!   `Parent=FALLBACK/` so every behaviour setting stays Konsole's default)
//!   and made the default;
//! * colour schemes travel as theme assets (copied in when missing);
//! * the palette generates `Kolira<Name>.colorscheme` (same ANSI colours
//!   as kitty/Alacritty) and selects it in the default profile.

use std::fs;
use std::path::PathBuf;

use anyhow::Result;

use crate::field::{ini_get, set_ini_text};
use crate::palette::{ansi16, kde_roles, kde_scheme_id, Palette, Rgb};
use crate::paths::{config_home, data_home};

pub const THEME_PROFILE: &str = "Theme Profile.profile";
const KONSOLE_DIR: &str = ".local/share/konsole";

fn konsolerc() -> PathBuf {
    config_home().join("konsolerc")
}

fn konsole_dir() -> PathBuf {
    data_home().join("konsole")
}

/// `X.profile` named by konsolerc, if any.
pub fn default_profile() -> Option<String> {
    let text = fs::read_to_string(konsolerc()).ok()?;
    ini_get(&text, "Desktop Entry", "DefaultProfile")
        .filter(|p| p.ends_with(".profile") && !p.contains('/'))
}

/// Tracked files: the default profile (or the one a theme would create) and
/// its colour scheme when it is a user scheme.
pub fn config_paths() -> Vec<String> {
    if which::which("konsole").is_err() && !konsole_dir().exists() {
        return Vec::new();
    }
    let profile = default_profile().unwrap_or_else(|| THEME_PROFILE.to_string());
    let mut out = vec![format!("{KONSOLE_DIR}/{profile}")];
    if let Some(scheme) = fs::read_to_string(konsole_dir().join(&profile))
        .ok()
        .and_then(|t| ini_get(&t, "Appearance", "ColorScheme"))
        .filter(|s| !s.is_empty() && !s.contains('/'))
    {
        if konsole_dir()
            .join(format!("{scheme}.colorscheme"))
            .is_file()
        {
            out.push(format!("{KONSOLE_DIR}/{scheme}.colorscheme"));
        }
    }
    out
}

/// A theme without Konsole: the profile we created is removed and Konsole goes
/// back to its built-in default. A profile of the user's stays.
pub fn reset_generated() -> Result<()> {
    let mut plan = crate::change_plan::Plan::default();
    plan_reset_generated(&mut plan)?;
    plan.commit()
}
pub fn finish_profile(rel: &str) -> Result<()> {
    let mut plan = crate::change_plan::Plan::default();
    plan_finish_profile(rel, &mut plan)?;
    plan.commit()
}

pub(crate) fn plan_reset_generated(plan: &mut crate::change_plan::Plan) -> Result<()> {
    if plan
        .text(&konsolerc())?
        .and_then(|text| ini_get(&text, "Desktop Entry", "DefaultProfile"))
        .as_deref()
        != Some(THEME_PROFILE)
    {
        return Ok(());
    }
    plan.remove(&konsole_dir().join(THEME_PROFILE), "reset")?;
    let text = plan.text(&konsolerc())?.unwrap_or_default();
    let kept: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("DefaultProfile="))
        .collect();
    plan.write_text(&konsolerc(), kept.join("\n") + "\n", "reset")?;
    Ok(())
}

/// After a profile was written from a theme: give a newly created profile a
/// name and make it Konsole's default when none is set.
pub(crate) fn plan_finish_profile(rel: &str, plan: &mut crate::change_plan::Plan) -> Result<()> {
    let path = crate::paths::live_path(rel);
    let Some(mut text) = plan.text(&path)? else {
        return Ok(());
    };
    let file = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if ini_get(&text, "General", "Name").is_none() {
        let name = file.trim_end_matches(".profile").to_string();
        text = set_ini_text(&text, "General", "Name", &name);
        text = set_ini_text(&text, "General", "Parent", "FALLBACK/");
        plan.write_text(&path, text, "konsole")?;
    }
    if plan
        .text(&konsolerc())?
        .and_then(|text| ini_get(&text, "Desktop Entry", "DefaultProfile"))
        .is_none()
    {
        let rc = plan.text(&konsolerc())?.unwrap_or_default();
        plan.write_text(
            &konsolerc(),
            set_ini_text(&rc, "Desktop Entry", "DefaultProfile", &file),
            "konsole",
        )?;
    }
    Ok(())
}

fn rgb(c: Rgb) -> String {
    c.kde()
}

/// A Konsole colour scheme from the palette (ANSI colours shared with kitty).
pub fn colorscheme(p: &Palette) -> String {
    let r = kde_roles(p);
    let ansi = ansi16(p);
    let mut out = String::new();
    let mut section = |name: &str, color: Rgb, bold: bool| {
        out.push_str(&format!(
            "[{name}]\nBold={bold}\nColor={}\nTransparency=false\n\n",
            rgb(color)
        ));
    };
    section("Background", r.window, false);
    section("BackgroundIntense", r.window_alt, false);
    section("BackgroundFaint", r.window, false);
    for i in 0..8 {
        section(&format!("Color{i}"), ansi[i], false);
        section(&format!("Color{i}Intense"), ansi[i + 8], true);
        section(
            &format!("Color{i}Faint"),
            ansi[i].mix(r.window, 0.35),
            false,
        );
    }
    section("Foreground", r.text, false);
    section("ForegroundIntense", r.text, true);
    section("ForegroundFaint", r.text_inactive, false);
    out.push_str(&format!("[General]\nBlur=false\nColorRandomization=false\nDescription=Theme Profile {}\nOpacity=1\n", p.name));
    out
}

/// Palette target `konsole`: write the scheme and select it in the default
/// profile (created when there is none). Returns the scheme path.
pub fn apply_palette(p: &Palette) -> Result<PathBuf> {
    let dir = konsole_dir();
    fs::create_dir_all(&dir)?;
    let id = kde_scheme_id(p);
    let scheme = dir.join(format!("{id}.colorscheme"));
    // Keep the user's opacity/blur when re-generating.
    let mut text = colorscheme(p);
    if let Ok(old) = fs::read_to_string(&scheme) {
        for key in ["Opacity", "Blur"] {
            if let Some(v) = ini_get(&old, "General", key) {
                text = set_ini_text(&text, "General", key, &v);
            }
        }
    }
    fs::write(&scheme, text)?;
    let profile_name = default_profile().unwrap_or_else(|| THEME_PROFILE.to_string());
    let profile = dir.join(&profile_name);
    let current = fs::read_to_string(&profile).unwrap_or_default();
    fs::write(
        &profile,
        set_ini_text(&current, "Appearance", "ColorScheme", &id),
    )?;
    finish_profile(&format!("{KONSOLE_DIR}/{profile_name}"))?;
    Ok(scheme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorscheme_has_every_konsole_section() {
        let p = crate::palette::parse_base16(
            "scheme: \"Test\"\nbase00: \"282a36\"\nbase01: \"21222c\"\nbase02: \"44475a\"\nbase03: \"6272a4\"\nbase04: \"9ea8c7\"\nbase05: \"f8f8f2\"\nbase06: \"f8f8f2\"\nbase07: \"ffffff\"\nbase08: \"ff5555\"\nbase09: \"ffb86c\"\nbase0A: \"f1fa8c\"\nbase0B: \"50fa7b\"\nbase0C: \"8be9fd\"\nbase0D: \"bd93f9\"\nbase0E: \"ff79c6\"\nbase0F: \"993333\"\n",
            "test",
        )
        .unwrap();
        let s = colorscheme(&p);
        for section in [
            "[Background]",
            "[Foreground]",
            "[Color0]",
            "[Color7Intense]",
            "[Color3Faint]",
            "[General]",
        ] {
            assert!(s.contains(section), "{section}");
        }
        assert!(s.contains("Color=40,42,54"), "background from base00");
    }
}
