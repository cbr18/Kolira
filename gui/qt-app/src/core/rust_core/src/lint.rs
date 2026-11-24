//! Theme lint: problems `check` cannot see because every resource is present.
//!
//! * A bundled icon theme that inherits Breeze shadows inherited icons:
//!   KIconLoader shortens a missing name at each dash *inside the current
//!   theme* before it looks at the parents, so `folder.svg` replaces every
//!   `folder-*` icon Breeze has and they all look the same.
//! * Unreadable text in a KDE colour scheme (Window/View/Button/Tooltip/Header;
//!   Selection is left out — stock Breeze itself keeps its secondary
//!   selection colours close to the accent).
//! * Unreadable text in a Kvantum `[GeneralColors]` block: tooltip/window text
//!   on its base, and on-accent highlight text — the widget style paints real
//!   windows, menus and tooltips, so a light foreground on a light base (e.g.
//!   a recoloured light theme that kept a dark base's white tooltip text) is
//!   invisible to the KDE colour-scheme check.
//! * A terminal "bright black" that disappears into the background, or
//!   unreadable foreground, in kitty, alacritty, konsole, ghostty, wezterm and
//!   foot: fish autosuggestions, comments and `ls` dim text use bright black.
//! * A GTK `colors.css` foreground that disappears into its background (normal
//!   and selected pairs); GTK 3/4 apps read these, not the KDE scheme.
use crate::{
    config_parser::ini_group_key,
    palette::{contrast, Rgb},
    paths, theme,
};
use anyhow::Result;
use serde::Serialize;
use std::{
    collections::{BTreeSet, HashSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Warning {
    /// icons | colors | terminal | kvantum | gtk
    pub kind: &'static str,
    /// Package-relative file or directory.
    pub path: String,
    pub message: String,
}

/// Normal text on its own background (WCAG AA).
const MIN_TEXT: f64 = 4.5;
/// Secondary (inactive) text in a KDE colour set.
const MIN_INACTIVE: f64 = 2.0;
/// Terminal bright black on the background.
const MIN_DIM: f64 = 2.0;
/// Kvantum tooltip/on-accent text on its base: a softer bar than body text,
/// because these are short labels and the artwork behind them may differ.
const MIN_KVANTUM_TEXT: f64 = 3.0;
/// Icon names listed in a warning.
const EXAMPLES: usize = 4;

/// Lint the installed theme `name`.
pub fn lint(name: &str) -> Result<Vec<Warning>> {
    paths::validate_profile_name(name)?;
    let dir = paths::profile_dir(name)?;
    let package = theme::load(&dir)?;
    Ok(lint_package(&dir, &package, &icon_roots()))
}

/// Where inherited icon themes are looked up (user first, as KDE does).
pub fn icon_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        paths::data_home().join("icons"),
        paths::home_dir().join(".icons"),
    ];
    roots.extend(
        std::env::var("XDG_DATA_DIRS")
            .unwrap_or_else(|_| "/usr/local/share:/usr/share".into())
            .split(':')
            .filter(|s| !s.trim().is_empty())
            .map(|s| PathBuf::from(s).join("icons")),
    );
    roots
}

pub fn lint_package(dir: &Path, package: &theme::Package, icon_roots: &[PathBuf]) -> Vec<Warning> {
    let mut out = vec![];
    let mut schemes: BTreeSet<String> = BTreeSet::new();
    for asset in &package.assets {
        match asset.kind.as_str() {
            "icons" => out.extend(icon_shadowing(dir, &asset.path, icon_roots)),
            "color-scheme" => {
                schemes.insert(asset.path.clone());
            }
            _ => {}
        }
    }
    if dir.join(".config/kdeglobals").is_file() {
        schemes.insert(".config/kdeglobals".into());
    }
    for rel in schemes {
        out.extend(color_scheme(dir, &rel));
    }
    out.extend(kvantum(dir));
    out.extend(terminals(dir));
    out.extend(gtk(dir));
    out
}

fn icon_shadowing(dir: &Path, rel: &str, roots: &[PathBuf]) -> Vec<Warning> {
    let root = dir.join(rel);
    let own = icon_names(&root);
    let mut inherited = HashSet::new();
    let mut seen = HashSet::new();
    let mut queue = inherits(&root);
    while let Some(parent) = queue.pop() {
        if !seen.insert(parent.clone()) {
            continue;
        }
        // Not installed here: nothing to compare with (offline lint).
        let Some(found) = roots
            .iter()
            .map(|r| r.join(&parent))
            .find(|p| p.join("index.theme").is_file())
        else {
            continue;
        };
        inherited.extend(icon_names(&found));
        queue.extend(inherits(&found));
    }
    let mut own_sorted: Vec<&String> = own.iter().collect();
    own_sorted.sort();
    own_sorted
        .into_iter()
        .filter_map(|name| {
            let prefix = format!("{name}-");
            let mut hidden: Vec<&String> = inherited
                .iter()
                .filter(|n| {
                    n.starts_with(&prefix) && !n.ends_with("-symbolic") && !own.contains(*n)
                })
                .collect();
            if hidden.is_empty() {
                return None;
            }
            hidden.sort();
            let examples: Vec<&str> = hidden.iter().take(EXAMPLES).map(|s| s.as_str()).collect();
            Some(Warning {
                kind: "icons",
                path: rel.to_string(),
                message: format!(
                    "`{name}` replaces {} inherited icon(s) without their own file here: {}{}",
                    hidden.len(),
                    examples.join(", "),
                    if hidden.len() > EXAMPLES { ", …" } else { "" }
                ),
            })
        })
        .collect()
}

fn inherits(theme_dir: &Path) -> Vec<String> {
    ini_group_key(&theme_dir.join("index.theme"), "Icon Theme", "Inherits")
        .map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// Icon names (file stems) anywhere below `root`; directory symlinks
/// (`16@2x` → `16`) are not followed, they only repeat names.
fn icon_names(root: &Path) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.is_dir() {
                stack.push(path);
                continue;
            }
            let file = entry.file_name().to_string_lossy().into_owned();
            if let Some((stem, ext)) = file.rsplit_once('.') {
                if matches!(ext, "svg" | "svgz" | "png" | "xpm") {
                    names.insert(stem.to_string());
                }
            }
        }
    }
    names
}

/// `r,g,b[,a]`, `#rrggbb`, `#rrggbbaa` or a bare `rrggbb`/`rrggbbaa` (foot,
/// alacritty hex without `#`).
fn parse_color(text: &str) -> Option<Rgb> {
    let text = text.trim().trim_matches('"').trim_matches('\'');
    if text.starts_with('#') {
        // KDE/Kvantum may append an alpha byte: keep the leading #rrggbb.
        return Rgb::parse(text.get(..7)?).ok();
    }
    if let Some(rgb) = bare_hex(text) {
        return Some(rgb);
    }
    let mut parts = text.split(',').map(|p| p.trim().parse::<u8>());
    Some(Rgb(
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    ))
}

/// `rrggbb` / `rrggbbaa` without a leading `#` (foot, some TOML dialects).
fn bare_hex(text: &str) -> Option<Rgb> {
    let is_hex = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit());
    if (text.len() == 6 || text.len() == 8) && is_hex(text) {
        Rgb::parse(&text[..6]).ok()
    } else {
        None
    }
}

fn color_scheme(dir: &Path, rel: &str) -> Vec<Warning> {
    let file = dir.join(rel);
    let mut out = vec![];
    for set in ["Window", "View", "Button", "Tooltip", "Header"] {
        let group = format!("Colors:{set}");
        let get = |key: &str| ini_group_key(&file, &group, key).and_then(|v| parse_color(&v));
        let Some(bg) = get("BackgroundNormal") else {
            continue;
        };
        for (key, min) in [
            ("ForegroundNormal", MIN_TEXT),
            ("ForegroundInactive", MIN_INACTIVE),
        ] {
            if let Some(fg) = get(key) {
                let ratio = contrast(fg, bg);
                if ratio < min {
                    out.push(Warning {
                        kind: "colors",
                        path: rel.to_string(),
                        message: format!(
                            "[{group}] {key} {} on BackgroundNormal {}: contrast {ratio:.2} < {min}",
                            fg.kde(),
                            bg.kde()
                        ),
                    });
                }
            }
        }
    }
    out
}

/// (package-relative file, background, foreground, bright black)
type TerminalPalette = (String, Option<Rgb>, Option<Rgb>, Option<Rgb>);

fn terminal_palettes(dir: &Path) -> Vec<TerminalPalette> {
    let mut out = vec![];
    let rel_of = |p: &Path| {
        p.strip_prefix(dir)
            .unwrap_or(p)
            .to_string_lossy()
            .into_owned()
    };
    let listed = |sub: &str, ext: &str| -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = fs::read_dir(dir.join(sub))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == ext))
            .collect();
        files.sort();
        files
    };
    for path in listed(".config/kitty", "conf") {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let value = |key: &str| {
            text.lines().find_map(|l| {
                let mut it = l.split_whitespace();
                (it.next() == Some(key))
                    .then(|| it.next().and_then(parse_color))
                    .flatten()
            })
        };
        if value("background").is_some() || value("color8").is_some() {
            out.push((
                rel_of(&path),
                value("background"),
                value("foreground"),
                value("color8"),
            ));
        }
    }
    for path in listed(".config/alacritty", "toml") {
        let Ok(doc) = crate::config_parser::parse_toml(&path) else {
            continue;
        };
        let get = |group: &str, key: &str| {
            doc.get("colors")?
                .get(group)?
                .get(key)?
                .as_str()
                .and_then(parse_color)
        };
        if get("primary", "background").is_some() || get("bright", "black").is_some() {
            out.push((
                rel_of(&path),
                get("primary", "background"),
                get("primary", "foreground"),
                get("bright", "black"),
            ));
        }
    }
    for path in listed(".local/share/konsole", "colorscheme") {
        let get = |g: &str| ini_group_key(&path, g, "Color").and_then(|v| parse_color(&v));
        out.push((
            rel_of(&path),
            get("Background"),
            get("Foreground"),
            get("Color0Intense"),
        ));
    }
    // ghostty: `key = value`, bright black = `palette = 8=`.
    for path in listed(".config/ghostty", "conf").into_iter().chain(
        fs::read_dir(dir.join(".config/ghostty"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n == "kolira")
            }),
    ) {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let value = |key: &str| {
            text.lines().find_map(|l| {
                let (k, v) = l.split_once('=')?;
                (k.trim() == key).then(|| parse_color(v.trim()))?
            })
        };
        let bright_black = text.lines().find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == "palette" && v.trim_start().starts_with("8="))
                .then(|| parse_color(v.trim().strip_prefix("8=")?))?
        });
        if value("background").is_some() || bright_black.is_some() {
            out.push((
                rel_of(&path),
                value("background"),
                value("foreground"),
                bright_black,
            ));
        }
    }
    // wezterm: a `colors = { ... }` inline table.
    for path in listed(".config/wezterm", "toml") {
        let Ok(doc) = crate::config_parser::parse_toml(&path) else {
            continue;
        };
        let get = |key: &str| doc.get("colors")?.get(key)?.as_str().and_then(parse_color);
        let bright_black = doc
            .get("colors")
            .and_then(|c| c.get("brights"))
            .and_then(|b| b.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .and_then(parse_color);
        if get("background").is_some() || bright_black.is_some() {
            out.push((
                rel_of(&path),
                get("background"),
                get("foreground"),
                bright_black,
            ));
        }
    }
    // foot: `[colors-dark]` / `[colors-light]`, bare hex.
    for path in listed(".config/foot", "ini") {
        for section in ["colors-dark", "colors-light"] {
            let get = |key: &str| ini_group_key(&path, section, key).and_then(|v| parse_color(&v));
            let bright_black = get("bright0");
            let bg = get("background");
            let fg = get("foreground");
            if bg.is_some() || bright_black.is_some() {
                out.push((rel_of(&path), bg, fg, bright_black));
                break; // one warning set per file is enough (both sections match)
            }
        }
    }
    out
}

fn terminals(dir: &Path) -> Vec<Warning> {
    let mut out = vec![];
    for (rel, bg, fg, dim) in terminal_palettes(dir) {
        let Some(bg) = bg else { continue };
        for (what, color, min, why) in [
            ("foreground", fg, MIN_TEXT, "text"),
            (
                "bright black",
                dim,
                MIN_DIM,
                "fish autosuggestions, comments, dim text",
            ),
        ] {
            if let Some(c) = color {
                let ratio = contrast(c, bg);
                if ratio < min {
                    out.push(Warning {
                        kind: "terminal",
                        path: rel.clone(),
                        message: format!(
                            "{what} {} on background {}: contrast {ratio:.2} < {min} ({why})",
                            c.hex(),
                            bg.hex()
                        ),
                    });
                }
            }
        }
    }
    out
}

/// Kvantum `[GeneralColors]`: a light foreground on a light base (tooltip text
/// above all) is invisible in real windows, menus and tooltips. The KDE
/// colour-scheme check never sees it because the widget style paints these.
fn kvantum(dir: &Path) -> Vec<Warning> {
    let mut out = vec![];
    let mut files: Vec<PathBuf> = fs::read_dir(dir.join(".config/Kvantum"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .flat_map(|d| {
            let name = d
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let kv = d.join(format!("{name}.kvconfig"));
            if kv.is_file() {
                vec![kv]
            } else {
                // A bare kvantum.kvconfig (theme selected by name) has no colours.
                vec![]
            }
        })
        .collect();
    files.sort();
    for path in files {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let get = |key: &str| general_colors(&text, key);
        let rel = path
            .strip_prefix(dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        // tooltip text on its base (or the window base if the theme sets none).
        let tooltip_bg = get("tooltip.base.color").or_else(|| get("window.color"));
        let mut pairs: Vec<(&str, Rgb, Rgb)> = Vec::new();
        if let (Some(fg), Some(bg)) = (get("tooltip.text.color"), tooltip_bg) {
            pairs.push(("tooltip.text.color", fg, bg));
        }
        if let (Some(fg), Some(bg)) = (get("text.color"), get("window.color")) {
            pairs.push(("text.color", fg, bg));
        }
        if let (Some(fg), Some(bg)) = (get("highlight.text.color"), get("highlight.color")) {
            pairs.push(("highlight.text.color", fg, bg));
        }
        for (key, fg, bg) in pairs {
            let ratio = contrast(fg, bg);
            if ratio < MIN_KVANTUM_TEXT {
                out.push(Warning {
                    kind: "kvantum",
                    path: rel.clone(),
                    message: format!(
                        "[GeneralColors] {key} {} on {}: contrast {ratio:.2} < {MIN_KVANTUM_TEXT}",
                        fg.hex(),
                        bg.hex()
                    ),
                });
            }
        }
    }
    out
}

/// A `[GeneralColors]` value (`#rrggbb[aa]`), read from a kvconfig string.
fn general_colors(text: &str, key: &str) -> Option<Rgb> {
    let mut in_colors = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_colors = trimmed == "[GeneralColors]";
            continue;
        }
        if !in_colors {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            if k.trim() == key {
                return parse_color(v.trim());
            }
        }
    }
    None
}

/// GTK `colors.css`: `@define-color` foreground/background pairs. GTK 3/4 apps
/// read these, not the KDE scheme.
fn gtk(dir: &Path) -> Vec<Warning> {
    let mut out = vec![];
    let mut files: Vec<PathBuf> = [".config/gtk-3.0/colors.css", ".config/gtk-4.0/colors.css"]
        .iter()
        .map(|rel| dir.join(rel))
        .filter(|p| p.is_file())
        .collect();
    files.sort();
    for path in files {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let color = |name: &str| {
            text.lines().find_map(|l| {
                let rest = l.trim().strip_prefix("@define-color ")?;
                let (n, v) = rest.split_once(' ')?;
                (n == name).then(|| parse_color(v.trim().trim_end_matches(';')))
            })?
        };
        let rel = path
            .strip_prefix(dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        for (fg_name, bg_name) in [
            ("theme_fg_color_breeze", "theme_bg_color_breeze"),
            (
                "theme_selected_fg_color_breeze",
                "theme_selected_bg_color_breeze",
            ),
        ] {
            if let (Some(fg), Some(bg)) = (color(fg_name), color(bg_name)) {
                let ratio = contrast(fg, bg);
                if ratio < MIN_TEXT {
                    out.push(Warning {
                        kind: "gtk",
                        path: rel.clone(),
                        message: format!(
                            "{fg_name} {} on {bg_name} {}: contrast {ratio:.2} < {MIN_TEXT}",
                            fg.hex(),
                            bg.hex()
                        ),
                    });
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn package(assets: &[(&str, &str)]) -> theme::Package {
        theme::Package {
            assets: assets
                .iter()
                .map(|(kind, path)| theme::Asset {
                    kind: (*kind).into(),
                    name: "x".into(),
                    path: (*path).into(),
                    origin: "generated".into(),
                    redistributable: true,
                    license: String::new(),
                    source: String::new(),
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn a_prefix_icon_shadows_inherited_ones() {
        let tmp = tempfile::tempdir().unwrap();
        let roots = vec![tmp.path().join("system")];
        let breeze = roots[0].join("breeze");
        write(
            &breeze.join("index.theme"),
            "[Icon Theme]\nName=Breeze\nInherits=hicolor\n",
        );
        for n in [
            "folder",
            "folder-open",
            "folder-music",
            "folder-symbolic",
            "preferences-desktop-mouse",
        ] {
            write(&breeze.join(format!("places/64/{n}.svg")), "<svg/>");
        }
        let hicolor = roots[0].join("hicolor");
        write(&hicolor.join("index.theme"), "[Icon Theme]\nName=Hicolor\n");
        write(&hicolor.join("apps/48/preferences-desktop-sound.png"), "");

        let pkg = tmp.path().join("pkg");
        let icons = pkg.join(".local/share/icons/mine");
        write(
            &icons.join("index.theme"),
            "[Icon Theme]\nName=Mine\nInherits=breeze,hicolor\n",
        );
        for n in [
            "folder",
            "folder-open",
            "preferences-desktop",
            "web-browser",
        ] {
            write(&icons.join(format!("scalable/{n}.svg")), "<svg/>");
        }
        let warnings = lint_package(
            &pkg,
            &package(&[("icons", ".local/share/icons/mine")]),
            &roots,
        );
        let messages: Vec<&str> = warnings.iter().map(|w| w.message.as_str()).collect();
        assert_eq!(warnings.len(), 2, "{messages:?}");
        // folder-open has its own file, folder-symbolic is a monochrome variant.
        assert!(
            messages[0].starts_with("`folder` replaces 1 "),
            "{}",
            messages[0]
        );
        assert!(messages[0].contains("folder-music"));
        // Inherited transitively (breeze → hicolor) and directly.
        assert!(
            messages[1].starts_with("`preferences-desktop` replaces 2 "),
            "{}",
            messages[1]
        );
        assert!(warnings.iter().all(|w| w.kind == "icons"));
    }

    #[test]
    fn missing_parent_theme_is_not_a_warning() {
        let tmp = tempfile::tempdir().unwrap();
        let icons = tmp.path().join("pkg/.local/share/icons/mine");
        write(
            &icons.join("index.theme"),
            "[Icon Theme]\nInherits=breeze\n",
        );
        write(&icons.join("scalable/folder.svg"), "<svg/>");
        let pkg = package(&[("icons", ".local/share/icons/mine")]);
        assert!(lint_package(&tmp.path().join("pkg"), &pkg, &[tmp.path().join("none")]).is_empty());
    }

    #[test]
    fn unreadable_scheme_text_is_reported_but_selection_is_not() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("pkg");
        write(
            &pkg.join(".local/share/color-schemes/X.colors"),
            "[Colors:Window]\nBackgroundNormal=56,56,56\nForegroundNormal=204,204,204\nForegroundInactive=60,60,60\n\n\
             [Colors:View]\nBackgroundNormal=37,37,37\nForegroundNormal=70,70,70\n\n\
             [Colors:Selection]\nBackgroundNormal=112,178,55\nForegroundInactive=112,178,55\n",
        );
        let warnings = lint_package(
            &pkg,
            &package(&[("color-scheme", ".local/share/color-schemes/X.colors")]),
            &[],
        );
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0]
            .message
            .starts_with("[Colors:Window] ForegroundInactive"));
        assert!(warnings[1]
            .message
            .starts_with("[Colors:View] ForegroundNormal"));
    }

    #[test]
    fn terminal_bright_black_on_background_is_reported() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("pkg");
        write(
            &pkg.join(".config/kitty/kolira.conf"),
            "foreground #CCCCCC\nbackground #252525\ncolor8 #404040\n",
        );
        write(
            &pkg.join(".config/alacritty/kolira.toml"),
            "[colors.primary]\nbackground = \"#252525\"\nforeground = \"#cccccc\"\n[colors.bright]\nblack = \"#808080\"\n",
        );
        write(
            &pkg.join(".local/share/konsole/X.colorscheme"),
            "[Background]\nColor=37,37,37\n\n[Foreground]\nColor=204,204,204\n\n[Color0Intense]\nColor=64,64,64\n",
        );
        let warnings = lint_package(&pkg, &package(&[]), &[]);
        let paths: Vec<&str> = warnings.iter().map(|w| w.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                ".config/kitty/kolira.conf",
                ".local/share/konsole/X.colorscheme"
            ],
            "{warnings:?}"
        );
        assert!(warnings
            .iter()
            .all(|w| w.kind == "terminal" && w.message.starts_with("bright black")));
    }

    #[test]
    fn ghostty_wezterm_foot_bright_black_is_reported() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("pkg");
        write(
            &pkg.join(".config/ghostty/kolira"),
            "background = #252525\nforeground = #cccccc\npalette = 0=#111111\npalette = 8=#3a3a3a\n",
        );
        write(
            &pkg.join(".config/wezterm/kolira.toml"),
            "colors = { background = \"#252525\", foreground = \"#cccccc\", ansi = [\"#111111\"], brights = [\"#3a3a3a\"] }\n",
        );
        write(
            &pkg.join(".config/foot/foot.ini"),
            "[colors-dark]\nbackground=252525\nforeground=cccccc\nbright0=3a3a3a\nregular0=111111\n",
        );
        let warnings = lint_package(&pkg, &package(&[]), &[]);
        let mut paths: Vec<&str> = warnings.iter().map(|w| w.path.as_str()).collect();
        paths.sort_unstable();
        assert_eq!(
            paths,
            [
                ".config/foot/foot.ini",
                ".config/ghostty/kolira",
                ".config/wezterm/kolira.toml",
            ],
            "{warnings:?}"
        );
        assert!(warnings.iter().all(|w| w.kind == "terminal"));
    }

    #[test]
    fn kvantum_light_tooltip_text_on_light_base_is_reported() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("pkg");
        // A light theme whose tooltip text stayed light (the Sandstone bug).
        write(
            &pkg.join(".config/Kvantum/X/X.kvconfig"),
            "[GeneralColors]\nwindow.color=#ebdbb2\nbase.color=#ebdbb2\ntext.color=#3c3836\nwindow.text.color=#3c3836\nbutton.text.color=#3c3836\ntooltip.text.color=#ebdbb2\n",
        );
        let warnings = lint_package(&pkg, &package(&[]), &[]);
        let msgs: Vec<&str> = warnings.iter().map(|w| w.message.as_str()).collect();
        assert_eq!(warnings.len(), 1, "{msgs:?}");
        assert!(warnings[0].kind == "kvantum");
        assert!(warnings[0]
            .message
            .starts_with("[GeneralColors] tooltip.text.color"));
    }

    #[test]
    fn kvantum_readable_tooltip_is_not_reported() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("pkg");
        write(
            &pkg.join(".config/Kvantum/X/X.kvconfig"),
            "[GeneralColors]\nwindow.color=#ebdbb2\ntooltip.text.color=#3c3836\ntext.color=#3c3836\n",
        );
        let warnings = lint_package(&pkg, &package(&[]), &[]);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn gtk_low_contrast_pair_is_reported() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("pkg");
        write(
            &pkg.join(".config/gtk-3.0/colors.css"),
            "@define-color theme_fg_color_breeze #cccccc;\n@define-color theme_bg_color_breeze #f0f0f0;\n@define-color theme_selected_fg_color_breeze #ffffff;\n@define-color theme_selected_bg_color_breeze #586d47;\n",
        );
        let warnings = lint_package(&pkg, &package(&[]), &[]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].kind == "gtk");
        assert!(warnings[0].message.starts_with("theme_fg_color_breeze"));
    }
}
