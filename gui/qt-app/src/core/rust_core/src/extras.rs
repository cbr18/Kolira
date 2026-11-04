//! Smaller apps ricers theme: oh-my-posh (prompt), ghostty, cava, fish
//! colours, yazi, bat. The same rules as everywhere (appearance.rs): only the
//! look travels, the user's config keeps everything else.
//!
//! * **oh-my-posh** — the prompt theme file the shell's `oh-my-posh init …
//!   --config <file>` line points to travels as
//!   `~/.config/oh-my-posh/theme-profile.omp.json`; applying only swaps the
//!   `--config` path in existing init lines (fish/zsh/bash), never adds one;
//! * **ghostty** — like kitty: the look in `ghostty/theme-profile`, the user's
//!   `config` gains `config-file = ?theme-profile` (last);
//! * **cava** — `[color]`; the palette sets a gradient; SIGUSR1 reloads.

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;

use crate::field::set_ini_text;
use crate::palette::{ansi16, kde_roles, Palette};
use crate::paths::{config_home, home_dir};

// ─────────────────────────────────────────────────────────────────────────────
// oh-my-posh
// ─────────────────────────────────────────────────────────────────────────────

pub const OMP_THEME: &str = ".config/oh-my-posh/theme-profile.omp.json";
pub const SHELL_HOSTS: &[&str] = &[".config/fish/config.fish", ".zshrc", ".bashrc"];
const OMP_PATH: &str = "\"$HOME/.config/oh-my-posh/theme-profile.omp.json\"";

/// Byte span of the `--config` value in an `oh-my-posh init` line.
fn omp_config_span(line: &str) -> Option<(usize, usize)> {
    if !line.contains("oh-my-posh") || !line.contains(" init ") || line.trim_start().starts_with('#') {
        return None;
    }
    let at = line.find("--config")?;
    let mut start = at + "--config".len();
    let bytes = line.as_bytes();
    if bytes.get(start) == Some(&b'=') {
        start += 1;
    }
    while bytes.get(start) == Some(&b' ') {
        start += 1;
    }
    let end = match bytes.get(start) {
        Some(q @ (b'\'' | b'"')) => line[start + 1..].find(*q as char).map(|e| start + 1 + e + 1)?,
        Some(_) => line[start..].find([' ', '|', ')']).map(|e| start + e).unwrap_or(line.len()),
        None => return None,
    };
    Some((start, end))
}

/// The prompt theme the live shell config uses (first init line found).
pub fn omp_current() -> Option<PathBuf> {
    let home = home_dir();
    for rel in SHELL_HOSTS {
        let Ok(text) = fs::read_to_string(home.join(rel)) else { continue };
        for line in text.lines() {
            if let Some((s, e)) = omp_config_span(line) {
                let raw = line[s..e].trim_matches(['\'', '"']);
                let expanded = raw.replace("$HOME", &home.to_string_lossy()).replace("${HOME}", &home.to_string_lossy());
                let path = match expanded.strip_prefix("~/") {
                    Some(rest) => home.join(rest),
                    None => PathBuf::from(expanded),
                };
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }
    None
}

/// Shell config files that have an oh-my-posh init line.
pub fn omp_hosts() -> Vec<String> {
    let home = home_dir();
    SHELL_HOSTS
        .iter()
        .filter(|rel| fs::read_to_string(home.join(rel)).is_ok_and(|t| t.lines().any(|l| omp_config_span(l).is_some())))
        .map(|r| r.to_string())
        .collect()
}

/// The shell config with every oh-my-posh init line pointing at the theme file.
pub fn omp_host(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        match omp_config_span(line) {
            Some((s, e)) => out.push(format!("{}{OMP_PATH}{}", &line[..s], &line[e..])),
            None => out.push(line.to_string()),
        }
    }
    let mut t = out.join("\n");
    if text.ends_with('\n') {
        t.push('\n');
    }
    t
}

pub fn omp_config_paths() -> Vec<String> {
    let mut out = omp_hosts();
    if !out.is_empty() || home_dir().join(OMP_THEME).exists() {
        out.push(OMP_THEME.to_string());
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// ghostty
// ─────────────────────────────────────────────────────────────────────────────

pub const GHOSTTY_HOST: &str = ".config/ghostty/config";
pub const GHOSTTY_THEME: &str = ".config/ghostty/theme-profile";
pub const GHOSTTY_INCLUDE: &str = "config-file = ?theme-profile";
const GHOSTTY_LOOK: &[&str] = &[
    "theme", "background", "foreground", "palette", "cursor-color", "cursor-text", "cursor-style",
    "selection-background", "selection-foreground", "background-opacity", "background-blur",
    "background-image", "font-family", "font-family-bold", "font-family-italic",
    "font-family-bold-italic", "font-feature", "window-padding-x", "window-padding-y",
    "window-theme", "window-decoration", "unfocused-split-opacity", "minimum-contrast",
];

fn ghostty_key(line: &str) -> Option<&str> {
    let t = line.trim_start();
    if t.starts_with('#') {
        return None;
    }
    Some(t.split_once('=')?.0.trim())
}

/// (look lines, the rest without the include line).
pub fn ghostty_split(host: &str) -> (String, String) {
    let mut look = Vec::new();
    let mut rest = Vec::new();
    for line in host.lines() {
        if line.trim() == GHOSTTY_INCLUDE {
            continue;
        }
        if ghostty_key(line).is_some_and(|k| GHOSTTY_LOOK.contains(&k)) {
            look.push(line);
        } else {
            rest.push(line);
        }
    }
    let join = |v: Vec<&str>| {
        let s = v.join("\n");
        let s = s.trim_end();
        if s.is_empty() { String::new() } else { format!("{s}\n") }
    };
    (join(look), join(rest))
}

/// The user's ghostty config without its look, including the theme file last.
pub fn ghostty_host(host: &str) -> String {
    let (_, rest) = ghostty_split(host);
    let rest = rest.trim_end();
    if rest.is_empty() {
        format!("{GHOSTTY_INCLUDE}\n")
    } else {
        format!("{rest}\n\n{GHOSTTY_INCLUDE}\n")
    }
}

const GHOSTTY_BEGIN: &str = "# BEGIN theme-profile palette (generated)";
const GHOSTTY_END: &str = "# END theme-profile palette";

pub fn ghostty_colors(p: &Palette) -> String {
    let r = kde_roles(p);
    let mut out = format!("{GHOSTTY_BEGIN}\n");
    let _ = writeln!(out, "background = {}\nforeground = {}", r.window.hex(), r.text.hex());
    let _ = writeln!(out, "cursor-color = {}\nselection-background = {}\nselection-foreground = {}", r.text.hex(), r.accent.hex(), r.selection_text.hex());
    for (i, c) in ansi16(p).iter().enumerate() {
        let _ = writeln!(out, "palette = {i}={}", c.hex());
    }
    out.push_str(GHOSTTY_END);
    out.push('\n');
    out
}

fn replace_block(existing: &str, block: &str, begin: &str, end: &str) -> String {
    let mut kept = Vec::new();
    let mut inside = false;
    for line in existing.lines() {
        if line.starts_with(begin) {
            inside = true;
            continue;
        }
        if inside {
            if line.starts_with(end) {
                inside = false;
            }
            continue;
        }
        kept.push(line);
    }
    let body = kept.join("\n");
    let body = body.trim_end();
    if body.is_empty() { block.to_string() } else { format!("{body}\n\n{block}") }
}

pub fn ghostty_apply_palette(p: &Palette) -> Result<Option<PathBuf>> {
    let host = home_dir().join(GHOSTTY_HOST);
    if !host.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&host)?;
    let theme = home_dir().join(GHOSTTY_THEME);
    let mut current = fs::read_to_string(&theme).unwrap_or_default();
    if current.is_empty() {
        current = ghostty_split(&text).0; // one-time move of the look
    }
    fs::write(&theme, replace_block(&current, &ghostty_colors(p), GHOSTTY_BEGIN, GHOSTTY_END))?;
    fs::write(&host, ghostty_host(&text))?;
    Ok(Some(theme))
}

// ─────────────────────────────────────────────────────────────────────────────
// cava
// ─────────────────────────────────────────────────────────────────────────────

pub const CAVA: &str = ".config/cava/config";

/// Palette target `cava`: a bottom-to-top gradient through the palette.
pub fn cava_apply_palette(p: &Palette) -> Result<Option<PathBuf>> {
    let path = config_home().join("cava/config");
    if !path.is_file() {
        return Ok(None);
    }
    let r = kde_roles(p);
    let a = ansi16(p);
    let stops = [a[4], r.accent, a[5], a[1]];
    let mut text = fs::read_to_string(&path)?;
    text = set_ini_text(&text, "color", "gradient", "1");
    text = set_ini_text(&text, "color", "gradient_count", &stops.len().to_string());
    for (i, c) in stops.iter().enumerate() {
        text = set_ini_text(&text, "color", &format!("gradient_color_{}", i + 1), &format!("'{}'", c.hex()));
    }
    text = set_ini_text(&text, "color", "foreground", &format!("'{}'", r.accent.hex()));
    fs::write(&path, text)?;
    reload_cava();
    Ok(Some(path))
}

/// cava re-reads its config on SIGUSR1.
pub fn reload_cava() {
    if std::env::var("KTM_THEME_NO_APPLY").as_deref() == Ok("1") || which::which("pkill").is_err() {
        return;
    }
    let _ = std::process::Command::new("pkill")
        .args(["-USR1", "-x", "cava"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

// ─────────────────────────────────────────────────────────────────────────────
// fish colours, bat theme line
// ─────────────────────────────────────────────────────────────────────────────

pub const FISH_VARS: &str = ".config/fish/fish_variables";
pub const BAT_CONFIG: &str = ".config/bat/config";

/// Universal variables that are fish's colours.
pub fn fish_colour_line(line: &str) -> bool {
    let t = line.trim_start();
    let name = t.strip_prefix("SETUVAR --export ").or_else(|| t.strip_prefix("SETUVAR ")).unwrap_or("");
    name.starts_with("fish_color_") || name.starts_with("fish_pager_color_")
}

fn fish_var_name(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let rest = t.strip_prefix("SETUVAR --export ").or_else(|| t.strip_prefix("SETUVAR "))?;
    Some(rest.split(':').next().unwrap_or(rest))
}

/// Lines a policy keeps / merges by an identifying name.
pub fn lines_filter(text: &str, keep: fn(&str) -> bool) -> String {
    let mut s = text.lines().filter(|l| keep(l)).collect::<Vec<_>>().join("\n");
    s.push('\n');
    s
}

fn name_of(line: &str) -> Option<String> {
    if let Some(n) = fish_var_name(line) {
        return Some(n.to_string());
    }
    let t = line.trim_start();
    t.strip_prefix("--").map(|r| r.split(['=', ' ']).next().unwrap_or(r).to_string())
}

/// Matching lines replaced in place, new ones appended, the theme's absent
/// ones removed; everything else untouched.
pub fn lines_merge(theme: &str, live: &str, keep: fn(&str) -> bool) -> String {
    let theme_lines: Vec<&str> = theme.lines().filter(|l| keep(l)).collect();
    let mut used = vec![false; theme_lines.len()];
    let mut out: Vec<String> = Vec::new();
    for line in live.lines() {
        if !keep(line) {
            out.push(line.to_string());
            continue;
        }
        if let Some(i) = theme_lines.iter().position(|t| name_of(t) == name_of(line)) {
            if !used[i] {
                out.push(theme_lines[i].to_string());
                used[i] = true;
            }
        }
    }
    out.extend(theme_lines.iter().zip(&used).filter(|(_, u)| !**u).map(|(l, _)| l.to_string()));
    let mut text = out.join("\n");
    text.push('\n');
    text
}

pub fn bat_theme_line(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("--theme") || t.starts_with("--italic-text")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omp_swaps_only_the_config_path() {
        let fish = "oh-my-posh init fish --config '/home/u/.local/share/omp-manager/themes/aliens.omp.json' | source  # [omp-manager]\nabbr -a g git\n";
        let out = omp_host(fish);
        assert!(out.starts_with("oh-my-posh init fish --config \"$HOME/.config/oh-my-posh/theme-profile.omp.json\" | source  # [omp-manager]"), "{out}");
        assert!(out.contains("abbr -a g git"));
        assert_eq!(omp_host(&out), out);
        let zsh = "eval \"$(oh-my-posh init zsh --config=~/x.json)\"\n";
        assert!(omp_host(zsh).contains("--config=\"$HOME/.config/oh-my-posh/theme-profile.omp.json\")"));
        assert_eq!(omp_host("# oh-my-posh init fish --config x\n"), "# oh-my-posh init fish --config x\n");
    }

    #[test]
    fn ghostty_like_kitty() {
        let host = "font-size = 13\nbackground = #000000\nkeybind = ctrl+a=new_tab\nbackground-opacity = 0.8\n";
        let (look, rest) = ghostty_split(host);
        assert_eq!(look, "background = #000000\nbackground-opacity = 0.8\n");
        assert!(rest.contains("font-size = 13") && rest.contains("keybind"));
        let h = ghostty_host(host);
        assert!(h.trim_end().ends_with(GHOSTTY_INCLUDE) && !h.contains("background"));
        assert_eq!(ghostty_host(&h), h);
    }

    #[test]
    fn fish_and_bat_lines() {
        let live = "SETUVAR __fish_initialized:4300\nSETUVAR fish_color_command:blue\nSETUVAR fish_user_paths:/x\n";
        let theme = "SETUVAR fish_color_command:bd93f9\nSETUVAR fish_pager_color_prefix:ff79c6\n";
        let m = lines_merge(theme, live, fish_colour_line);
        assert!(m.contains("fish_color_command:bd93f9") && m.contains("fish_pager_color_prefix") && m.contains("fish_user_paths:/x"));
        assert!(!m.contains("command:blue"));
        let bat = lines_merge("--theme=\"Dracula\"\n", "--theme=\"TwoDark\"\n--style=numbers\n", bat_theme_line);
        assert_eq!(bat, "--theme=\"Dracula\"\n--style=numbers\n");
    }
}
