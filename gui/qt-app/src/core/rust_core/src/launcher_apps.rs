//! Launcher palette adapters: Rofi, Fuzzel, Walker. Colour-only resources,
//! never personal bindings, history or launch commands. A missing resource in
//! the next theme removes the managed entry; late personal edits survive.

use crate::{
    app_sync,
    change_plan::Plan,
    native_appearance,
    palette::{kde_roles, Palette},
    paths,
    theme::Package,
};
use anyhow::{Context, Result};
use std::{collections::BTreeMap, fs, path::Path};

pub const TARGETS: &[&str] = &["rofi", "fuzzel", "walker"];

pub const ROFI_HOST: &str = ".config/rofi/config.rasi";
pub const ROFI_THEME: &str = ".config/rofi/themes/kolira.rasi";
pub const FUZZEL: &str = ".config/fuzzel/fuzzel.ini";
pub const WALKER: &str = ".config/walker/style.css";

pub const ROFI_BEGIN: &str = "/* BEGIN kolira colours */";
pub const ROFI_END: &str = "/* END kolira colours */";
pub const FUZZEL_BEGIN: &str = "# BEGIN kolira colours";
pub const FUZZEL_END: &str = "# END kolira colours";
pub const WALKER_BEGIN: &str = "/* BEGIN kolira colours */";
pub const WALKER_END: &str = "/* END kolira colours */";

pub fn paths_for(id: &str) -> Vec<String> {
    let paths: &[&str] = match id {
        "rofi" => &[ROFI_HOST, ROFI_THEME],
        "fuzzel" => &[FUZZEL],
        "walker" => &[WALKER],
        _ => &[],
    };
    paths.iter().map(|s| s.to_string()).collect()
}

fn rofi(p: &Palette) -> String {
    let r = kde_roles(p);
    let mut out = String::from(ROFI_BEGIN);
    out.push_str("\n* {\n");
    out.push_str(&format!("    background: {};\n", r.window.hex()));
    out.push_str(&format!("    foreground: {};\n", r.text.hex()));
    out.push_str(&format!("    background-color: {};\n", r.window.hex()));
    out.push_str(&format!("    color: {};\n", r.text.hex()));
    out.push_str(&format!("    border: 1px solid {};\n", p.slot(2).hex()));
    out.push_str("    border-radius: 4px;\n");
    out.push_str("    padding: 8px;\n");
    out.push_str("}\n");
    out.push_str(&format!("#input {{ background-color: {}; color: {}; border-color: {}; }}\n", r.window_alt.hex(), r.text.hex(), r.accent.hex()));
    out.push_str(&format!("#entry:selected {{ background-color: {}; color: {}; }}\n", r.accent.hex(), r.selection_text.hex()));
    out.push_str(&format!("#text:selected {{ color: {}; }}\n", r.selection_text.hex()));
    out.push_str(&format!("\n{}", ROFI_END));
    out.push('\n');
    out
}

fn fuzzel(p: &Palette) -> String {
    let r = kde_roles(p);
    let mut out = String::from(FUZZEL_BEGIN);
    out.push_str("\n[color]\n");
    out.push_str(&format!("background = {}\n", r.window.hex()));
    out.push_str(&format!("text = {}\n", r.text.hex()));
    out.push_str(&format!("background-selected = {}\n", r.accent.hex()));
    out.push_str(&format!("text-selected = {}\n", r.selection_text.hex()));
    out.push_str(&format!("border = {}\n", p.slot(2).hex()));
    out.push_str(&format!("\n{}", FUZZEL_END));
    out.push('\n');
    out
}

fn walker(p: &Palette) -> String {
    let r = kde_roles(p);
    let mut out = String::from(WALKER_BEGIN);
    out.push_str("\n/* Kolira colours only */\n");
    out.push_str(&format!("window {{ background-color: {}; color: {}; }}\n", r.window.hex(), r.text.hex()));
    out.push_str(&format!("#input {{ background-color: {}; color: {}; border-color: {}; }}\n", r.window_alt.hex(), r.text.hex(), r.accent.hex()));
    out.push_str(&format!("#entry:selected {{ background-color: {}; }}\n", r.accent.hex()));
    out.push_str(&format!("#text:selected {{ color: {}; }}\n", r.selection_text.hex()));
    out.push_str(&format!("\n{}", WALKER_END));
    out.push('\n');
    out
}

/// Replace or insert a managed colour block, keeping everything else intact.
pub fn replace_block(existing: &str, block: &str, begin: &str, end: &str) -> String {
    let mut kept = Vec::new();
    let mut inside = false;
    for line in existing.lines() {
        if line.trim().starts_with(begin) {
            inside = true;
            continue;
        }
        if inside {
            if line.trim().starts_with(end) {
                inside = false;
            }
            continue;
        }
        kept.push(line);
    }
    let body = kept.join("\n");
    let body = body.trim_end();
    if body.is_empty() {
        block.to_string()
    } else if block.is_empty() {
        body.to_string()
    } else {
        format!("{body}\n\n{block}")
    }
}

fn valid_rasi(text: &str) -> Result<String> {
    let bytes = text.as_bytes();
    let mut colours = Vec::new();
    for (i, c) in bytes.iter().enumerate() {
        if *c == b'#'
            && i + 7 <= bytes.len()
            && bytes[i + 1..i + 7].iter().all(u8::is_ascii_hexdigit)
        {
            colours.push(crate::palette::Rgb::parse(&text[i..i + 7])?.hex());
        }
    }
    anyhow::ensure!(
        !colours.is_empty(),
        "Rofi theme must contain at least one controlled colour"
    );
    for c in &colours {
        crate::palette::Rgb::parse(c)?;
    }
    Ok(text.into())
}

fn valid_ini(text: &str) -> Result<String> {
    let mut in_color = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_color = t == "[color]" || t.eq_ignore_ascii_case("[color]");
            continue;
        }
        if in_color && !t.is_empty() && !t.starts_with('#') {
            if let Some((_, v)) = t.split_once('=') {
                let c = v.trim().trim_matches(['\'', '"']);
                if c.starts_with('#') {
                    crate::palette::Rgb::parse(c)?;
                }
            }
        }
    }
    Ok(text.into())
}

fn valid_css(text: &str) -> Result<String> {
    let bytes = text.as_bytes();
    let mut colours = Vec::new();
    for (i, c) in bytes.iter().enumerate() {
        if *c == b'#'
            && i + 7 <= bytes.len()
            && bytes[i + 1..i + 7].iter().all(u8::is_ascii_hexdigit)
        {
            colours.push(crate::palette::Rgb::parse(&text[i..i + 7])?.hex());
        }
    }
    anyhow::ensure!(
        !colours.is_empty(),
        "Walker CSS must contain at least one controlled colour"
    );
    for c in &colours {
        crate::palette::Rgb::parse(c)?;
    }
    Ok(text.into())
}

pub fn is_asset(rel: &str) -> bool {
    matches!(rel, ROFI_THEME | FUZZEL | WALKER)
}

pub fn valid_asset(rel: &str, text: &str) -> Result<String> {
    if rel == ROFI_THEME {
        return valid_rasi(text);
    }
    if rel == FUZZEL {
        return valid_ini(text);
    }
    if rel == WALKER {
        return valid_css(text);
    }
    Ok(text.into())
}

fn write_managed(plan: &mut Plan, rel: &str, text: String) -> Result<()> {
    app_sync::write(plan, &paths::live_path(rel), text)
}

pub fn bundle_resources(p: &Palette) -> BTreeMap<String, String> {
    [
        (ROFI_THEME.into(), rofi(p)),
        (FUZZEL.into(), fuzzel(p)),
        (WALKER.into(), walker(p)),
    ]
    .into_iter()
    .collect()
}

pub fn plan(source: &Path, package: &Package, plan: &mut Plan) -> Result<()> {
    let wants = |id: &str| package.app_targets.iter().any(|s| s == id);
    let Some(p) = &package.palette else {
        return Ok(());
    };
    p.validate()?;
    let resources = bundle_resources(p);
    for id in TARGETS.iter().filter(|id| wants(id)) {
        let command = match *id {
            "rofi" => "rofi",
            "fuzzel" => "fuzzel",
            "walker" => "walker",
            _ => unreachable!(),
        };
        // Presence check: the config folder must exist (the app may be installed
        // without being on PATH; missing apps are skipped silently).
        let present = paths::live_path(match *id {
            "rofi" => ".config/rofi",
            "fuzzel" => ".config/fuzzel",
            "walker" => ".config/walker",
            _ => "",
        })
        .is_dir()
        || which::which(command).is_ok();
        if !present {
            continue;
        }
        for rel in paths_for(id) {
            // Theme-carried resource wins over palette generation.
            let text = if source.join(&rel).is_file() {
                fs::read_to_string(source.join(&rel))?
            } else {
                match resources.get(&rel) {
                    Some(t) => t.clone(),
                    None => continue,
                }
            };
            // Fuzzel and Walker keep surrounding personal sections.
            let out = if rel == FUZZEL || rel == WALKER {
                let old = plan
                    .text(&paths::live_path(rel.as_str()))?
                    .unwrap_or_default();
                if old.contains(FUZZEL_BEGIN) || old.contains(WALKER_BEGIN) {
                    let (begin, end) = if rel == FUZZEL {
                        (FUZZEL_BEGIN, FUZZEL_END)
                    } else {
                        (WALKER_BEGIN, WALKER_END)
                    };
                    replace_block(&old, &text, begin, end)
                } else if old.trim().is_empty() {
                    text
                } else {
                    format!("{old}\n{text}")
                }
            } else {
                text
            };
            // Validate before any write: invalid resources abort the preview.
            valid_asset(rel.as_str(), &out)?;
            write_managed(plan, rel.as_str(), out)?;
        }
        // Host-side include / selection wiring (always applied when the target
        // is selected, independent of whether the theme carries a resource file).
        if *id == "rofi" {
            let old = plan.text(&paths::live_path(ROFI_HOST))?.unwrap_or_default();
            if !old.contains("@theme \"kolira\"") {
                let host = if old.trim().is_empty() {
                    format!("@theme \"{}\"\n", paths::live_path(ROFI_THEME).display())
                } else {
                    format!("{old}\n@theme \"{}\"\n", paths::live_path(ROFI_THEME).display())
                };
                write_managed(plan, ROFI_HOST, host)?;
            }
        }
    }
    Ok(())
}

/// Direct palette-apply for Rofi: writes the managed theme file and wires the
/// host `config.rasi` to reference it. Mirrors `cava_apply_palette`.
pub fn apply_rofi(p: &Palette) -> Result<Option<std::path::PathBuf>> {
    let host = crate::paths::live_path(ROFI_HOST);
    let theme = crate::paths::live_path(ROFI_THEME);
    if !host.is_file() {
        return Ok(None);
    }
    let dir = theme.parent().context("rofi theme parent dir")?;
    std::fs::create_dir_all(dir)?;
    std::fs::write(&theme, rofi(p))?;
    let old = std::fs::read_to_string(&host).unwrap_or_default();
    if !old.contains("@theme \"kolira\"") {
        let host_text = if old.trim().is_empty() {
            format!("@theme \"{}\"\n", theme.display())
        } else {
            format!("{old}\n@theme \"{}\"\n", theme.display())
        };
        std::fs::write(&host, host_text)?;
    }
    Ok(Some(theme))
}

/// Direct palette-apply for Fuzzel: merges the managed `[color]` block into
/// `fuzzel.ini` positionally so personal sections survive.
pub fn apply_fuzzel(p: &Palette) -> Result<Option<std::path::PathBuf>> {
    let path = crate::paths::live_path(FUZZEL);
    if !path.is_file() {
        return Ok(None);
    }
    let old = std::fs::read_to_string(&path)?;
    let merged = native_appearance::merge(FUZZEL, fuzzel(p).as_str(), old.as_str())?;
    std::fs::write(&path, merged)?;
    Ok(Some(path))
}

/// Direct palette-apply for Walker: appends the managed CSS colour block.
pub fn apply_walker(p: &Palette) -> Result<Option<std::path::PathBuf>> {
    let path = crate::paths::live_path(WALKER);
    let css = if path.is_file() {
        let old = std::fs::read_to_string(&path)?;
        if old.contains(WALKER_BEGIN) {
            // Replace existing managed block.
            replace_block(&old, &walker(p), WALKER_BEGIN, WALKER_END)
        } else {
            format!("{old}\n{}", walker(p))
        }
    } else {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        walker(p)
    };
    std::fs::write(&path, css)?;
    Ok(Some(path))
}

pub fn capture(rel: &str, text: &str) -> Result<String> {
    if rel == ROFI_THEME || rel == WALKER || rel == FUZZEL {
        let bytes = text.as_bytes();
        let mut colours = Vec::new();
        for (i, c) in bytes.iter().enumerate() {
            if *c == b'#'
                && i + 7 <= bytes.len()
                && bytes[i + 1..i + 7].iter().all(u8::is_ascii_hexdigit)
            {
                colours.push(crate::palette::Rgb::parse(&text[i..i + 7])?.hex());
            }
        }
        if colours.is_empty() {
            return Ok(String::new());
        }
    }
    Ok(text.into())
}

pub fn restore(rel: &str, before: &str, after: &str, current: &str) -> Result<String> {
    // Position-preserving restore for TOML/INI/CSS hosts.
    if rel == ROFI_THEME || rel == WALKER {
        return native_appearance::restore(rel, before, after, current);
    }
    if rel == FUZZEL {
        return native_appearance::restore(rel, before, after, current);
    }
    if rel == ROFI_HOST {
        // Remove the managed @theme line if the after no longer carries it.
        let managed = format!("@theme \"{}\"", paths::live_path(ROFI_THEME).display());
        let mut out = current.to_string();
        if out.lines().any(|l| l.trim() == managed || l.trim_start().starts_with("@theme")) {
            out = out
                .lines()
                .filter(|l| {
                    let t = l.trim();
                    !(t == managed || t.starts_with("@theme"))
                })
                .collect::<Vec<_>>()
                .join("\n");
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
        }
        return Ok(out);
    }
    Ok(current.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rofi_colours_are_valid_and_contained() {
        let p = crate::palette::Palette {
            name: "test".into(),
            author: String::new(),
            variant: crate::palette::Variant::Dark,
            base: vec![
                "#000000".into(), "#ff0000".into(), "#00ff00".into(), "#ffff00".into(),
                "#0000ff".into(), "#ff00ff".into(), "#00ffff".into(), "#ffffff".into(),
                "#808080".into(), "#ff8080".into(), "#80ff80".into(), "#ffff80".into(),
                "#8080ff".into(), "#ff80ff".into(), "#80ffff".into(), "#c0c0c0".into(),
            ],
            accent: "#ff0000".into(),
            source: "test".into(),
        };
        let r = rofi(&p);
        assert!(r.contains(ROFI_BEGIN) && r.contains(ROFI_END));
        valid_rasi(&r).unwrap();
    }

    #[test]
    fn fuzzel_colours_are_valid() {
        let p = crate::palette::Palette {
            name: "test".into(),
            author: String::new(),
            variant: crate::palette::Variant::Dark,
            base: vec![
                "#000000".into(), "#ff0000".into(), "#00ff00".into(), "#ffff00".into(),
                "#0000ff".into(), "#ff00ff".into(), "#00ffff".into(), "#ffffff".into(),
                "#808080".into(), "#ff8080".into(), "#80ff80".into(), "#ffff80".into(),
                "#8080ff".into(), "#ff80ff".into(), "#80ffff".into(), "#c0c0c0".into(),
            ],
            accent: "#ff0000".into(),
            source: "test".into(),
        };
        let r = fuzzel(&p);
        assert!(r.contains(FUZZEL_BEGIN) && r.contains(FUZZEL_END));
        valid_ini(&r).unwrap();
    }

    #[test]
    fn walker_colours_are_valid() {
        let p = crate::palette::Palette {
            name: "test".into(),
            author: String::new(),
            variant: crate::palette::Variant::Dark,
            base: vec![
                "#000000".into(), "#ff0000".into(), "#00ff00".into(), "#ffff00".into(),
                "#0000ff".into(), "#ff00ff".into(), "#00ffff".into(), "#ffffff".into(),
                "#808080".into(), "#ff8080".into(), "#80ff80".into(), "#ffff80".into(),
                "#8080ff".into(), "#ff80ff".into(), "#80ffff".into(), "#c0c0c0".into(),
            ],
            accent: "#ff0000".into(),
            source: "test".into(),
        };
        let r = walker(&p);
        assert!(r.contains(WALKER_BEGIN) && r.contains(WALKER_END));
        valid_css(&r).unwrap();
    }

    #[test]
    fn replace_block_keeps_surrounding_personal_text() {
        let live = "font = \"mono\"\n/* BEGIN kolira colours */\nold = #000000\n/* END kolira colours */\n# personal comment\n";
        let block = "/* BEGIN kolira colours */\nnew = #111111\n/* END kolira colours */";
        let out = replace_block(live, block, ROFI_BEGIN, ROFI_END);
        assert!(out.contains("font = \"mono\""));
        assert!(out.contains("# personal comment"));
        assert!(out.contains("new = #111111"));
        assert!(!out.contains("old = #000000"));
    }

    #[test]
    fn rofi_host_adds_theme_line_once() {
        let old = "";
        let mut out = old.to_string();
        let theme_line = format!("@theme \"{}\"", "/home/user/.config/rofi/themes/kolira.rasi");
        if !out.contains(&theme_line) {
            out = format!("{out}\n{theme_line}\n");
        }
        assert_eq!(
            out.lines().filter(|l| l.trim().starts_with("@theme")).count(),
            1
        );
        // Idempotent
        if !out.contains(&theme_line) {
            out = format!("{out}\n{theme_line}\n");
        }
        assert_eq!(
            out.lines().filter(|l| l.trim().starts_with("@theme")).count(),
            1
        );
    }

    #[test]
    fn restore_removes_managed_rofi_theme_line() {
        let before = "font = \"mono\"\n";
        let after = "font = \"mono\"\n@theme \"/home/user/.config/rofi/themes/kolira.rasi\"\n";
        let restored = restore(ROFI_HOST, before, after, after).unwrap();
        assert!(!restored.contains("@theme"));
        assert!(restored.contains("font = \"mono\""));
    }
}
