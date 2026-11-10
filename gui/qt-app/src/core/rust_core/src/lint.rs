//! Theme lint: problems `check` cannot see because every resource is present.
//!
//! * A bundled icon theme that inherits Breeze shadows inherited icons:
//!   KIconLoader shortens a missing name at each dash *inside the current
//!   theme* before it looks at the parents, so `folder.svg` replaces every
//!   `folder-*` icon Breeze has and they all look the same.
//! * Unreadable text in a KDE colour scheme (Window/View/Button/Tooltip/Header;
//!   Selection is left out — stock Breeze itself keeps its secondary
//!   selection colours close to the accent).
//! * A terminal "bright black" that disappears into the background: fish
//!   autosuggestions, comments and `ls` dim text use it.
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
    /// icons | colors | terminal
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
    out.extend(terminals(dir));
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

/// `r,g,b[,a]` or `#rrggbb`.
fn parse_color(text: &str) -> Option<Rgb> {
    let text = text.trim().trim_matches('"').trim_matches('\'');
    if text.starts_with('#') {
        return Rgb::parse(text.get(..7)?).ok();
    }
    let mut parts = text.split(',').map(|p| p.trim().parse::<u8>());
    Some(Rgb(
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    ))
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
}
