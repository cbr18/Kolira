//! What a theme needs that does not travel inside it, and where to get it.
//!
//! At save time every referenced asset that is not bundled in the theme
//! (icons, cursors, GTK, Kvantum, Plasma style, Global Theme, colour scheme,
//! window decoration, panel widgets, editor themes) becomes a [`Dependency`]
//! with its source, detected automatically:
//!
//! 1. KDE Store (`api.kde-look.org`) — from KDE's own KNewStuff registries
//!    (`~/.local/share/knewstuff3/*.knsregistry`, written by "Get New…"):
//!    exact id;
//! 2. a system package — `pacman -Qqo` on the asset path (repo or AUR);
//! 3. VS Code Marketplace / Open VSX / Zed extensions — installed extensions;
//! 4. otherwise a KDE Store search by name within the right category: a
//!    *candidate* (`exact: false`) the user confirms before installing.
//!
//! On another machine [`missing`] lists what is not installed, and
//! [`install`] fetches it from the official source: Plasma packages through
//! `kpackagetool6`, icons/cursors/GTK/Kvantum/Aurorae/colour schemes by
//! unpacking the store archive into the user's directories, editor themes
//! through the stores module. Explicit package jobs invoke the system package
//! manager in a visible terminal; password input stays there. The legacy
//! single-resource `install` API returns the command instead.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::paths::{config_home, data_home, home_dir};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Dependency {
    /// icons | cursors | gtk | kvantum | plasma-theme | look-and-feel |
    /// color-scheme | aurorae | decoration | plasmoid | vscode | cursor | zed
    pub kind: String,
    /// The name the theme uses (icon theme dir, Kvantum theme, plugin id…).
    pub name: String,
    /// kde-store | package | aur | vscode-marketplace | open-vsx | zed | none
    pub source: String,
    /// Store id, package name or extension id.
    #[serde(default)]
    pub id: String,
    /// Store title (may differ from `name`).
    #[serde(default)]
    pub title: String,
    /// Page to show the user.
    #[serde(default)]
    pub url: String,
    /// `false`: found by a name search — confirm before installing.
    #[serde(default = "yes")]
    pub exact: bool,
}

fn yes() -> bool {
    true
}

const OCS: &str = "https://api.kde-look.org/ocs/v1/content/data";

fn store_page(id: &str) -> String {
    format!("https://store.kde.org/p/{id}")
}

fn store_search(name: &str) -> String {
    format!(
        "https://store.kde.org/search?search={}",
        name.replace(' ', "+")
    )
}

fn safe_name(name: &str) -> bool {
    !name.is_empty() && !name.contains('/') && !name.starts_with('.')
}

// ─────────────────────────────────────────────────────────────────────────────
// Where assets live
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn roots(kind: &str) -> Vec<PathBuf> {
    let home = home_dir();
    let data = data_home();
    match kind {
        "icons" | "cursors" => vec![
            data.join("icons"),
            home.join(".icons"),
            "/usr/share/icons".into(),
        ],
        "gtk" => vec![
            data.join("themes"),
            home.join(".themes"),
            "/usr/share/themes".into(),
        ],
        "kvantum" => vec![config_home().join("Kvantum"), "/usr/share/Kvantum".into()],
        "plasma-theme" => vec![
            data.join("plasma/desktoptheme"),
            "/usr/share/plasma/desktoptheme".into(),
        ],
        "look-and-feel" => vec![
            data.join("plasma/look-and-feel"),
            "/usr/share/plasma/look-and-feel".into(),
        ],
        "color-scheme" => vec![
            data.join("color-schemes"),
            "/usr/share/color-schemes".into(),
        ],
        "aurorae" => vec![
            data.join("aurorae/themes"),
            "/usr/share/aurorae/themes".into(),
        ],
        "plasmoid" => vec![
            data.join("plasma/plasmoids"),
            "/usr/share/plasma/plasmoids".into(),
        ],
        _ => Vec::new(),
    }
}

/// The installed asset `name` of `kind`, if any (the one that wins: user
/// directories before system ones).
pub fn locate(kind: &str, name: &str) -> Option<PathBuf> {
    locate_all(kind, name).into_iter().next()
}

/// Every installed copy of asset `name` (a user copy may shadow a packaged
/// one — Klassy generates its icon theme into ~/.local/share/icons).
pub(crate) fn locate_all(kind: &str, name: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !safe_name(name) {
        return out;
    }
    if kind == "kwin-effect" {
        return crate::kwin_effects::find(name)
            .map(|e| e.path)
            .into_iter()
            .collect();
    }
    for root in roots(kind) {
        let hit = match kind {
            "color-scheme" => Some(root.join(format!("{name}.colors"))).filter(|p| p.is_file()),
            "kvantum" => {
                let direct = root.join(name);
                if direct.join(format!("{name}.kvconfig")).is_file() {
                    Some(direct)
                } else {
                    fs::read_dir(&root).ok().and_then(|it| {
                        it.flatten()
                            .map(|e| e.path())
                            .find(|d| d.join(format!("{name}.kvconfig")).is_file())
                    })
                }
            }
            "cursors" => Some(root.join(name)).filter(|p| p.join("cursors").exists()),
            "icons" => Some(root.join(name)).filter(|p| p.join("index.theme").is_file()),
            _ => Some(root.join(name)).filter(|p| p.exists()),
        };
        out.extend(hit);
    }
    out
}

fn decoration_plugin(library: &str) -> Option<PathBuf> {
    let short = library.rsplit('.').next()?.to_lowercase();
    for dir in [
        "/usr/lib/qt6/plugins/org.kde.kdecoration3",
        "/usr/lib/qt6/plugins/org.kde.kdecoration2",
    ] {
        if let Ok(it) = fs::read_dir(dir) {
            if let Some(p) = it.flatten().map(|e| e.path()).find(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().to_lowercase().contains(&short))
            }) {
                return Some(p);
            }
        }
    }
    None
}

/// Qt widget style plugin for `style` (kvantum → libkvantum.so), unless the
/// style ships with Qt/KDE itself (Breeze, Fusion, Windows, Oxygen).
fn widget_style_plugin(style: &str) -> Option<PathBuf> {
    let short = style.to_lowercase();
    if matches!(
        short.as_str(),
        "" | "breeze" | "fusion" | "windows" | "oxygen"
    ) {
        return None;
    }
    ["/usr/lib/qt6/plugins/styles", "/usr/lib/qt/plugins/styles"]
        .iter()
        .find_map(|dir| {
            fs::read_dir(dir)
                .ok()?
                .flatten()
                .map(|e| e.path())
                .find(|p| {
                    p.file_stem().is_some_and(|n| {
                        n.to_string_lossy()
                            .to_lowercase()
                            .trim_start_matches("lib")
                            .starts_with(&short)
                    })
                })
        })
}

/// Is dependency `d` satisfied on this machine?
pub fn installed(d: &Dependency) -> bool {
    match d.kind.as_str() {
        "command" => which::which(&d.name).is_ok(),
        "decoration" => decoration_plugin(&d.name).is_some(),
        "spicetify" => which::which("spicetify").is_ok(),
        "widget-style" => widget_style_plugin(&d.name).is_some(),
        "oh-my-posh" => {
            which::which("oh-my-posh").is_ok() || home_dir().join(".local/bin/oh-my-posh").is_file()
        }
        "discord-client" => crate::discord::CLIENTS
            .iter()
            .any(|(_, rel)| home_dir().join(rel).exists()),
        "firefox-theme" => crate::tracked_apps::firefox_profile_dir().is_some_and(|dir| {
            home_dir()
                .join(".mozilla/firefox")
                .join(dir)
                .join("extensions")
                .join(format!("{}.xpi", d.name))
                .exists()
        }),
        "vscode" | "cursor" | "zed" => {
            let field = match d.kind.as_str() {
                "vscode" => "vscode_theme",
                "cursor" => "cursor_ide_theme",
                _ => "zed_dark",
            };
            crate::theme::available(field, &Value::from(d.name.clone()))
        }
        kind => locate(kind, &d.name).is_some(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Sources
// ─────────────────────────────────────────────────────────────────────────────

/// (knsrc stem, store id, title) of the KNewStuff entry that installed `path`.
fn knewstuff_entry(path: &Path) -> Option<(String, String, String)> {
    let dir = data_home().join("knewstuff3");
    let path = path.to_string_lossy().to_string();
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let file = entry.path();
        if file.extension().is_none_or(|e| e != "knsregistry") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        for stuff in text.split("<stuff").skip(1) {
            let tag = |t: &str| -> Option<String> {
                let open = format!("<{t}>");
                let s = stuff.find(&open)? + open.len();
                let e = stuff[s..].find(&format!("</{t}>"))? + s;
                Some(stuff[s..e].trim().to_string())
            };
            let provides = stuff.match_indices("<installedfile>").any(|(i, _)| {
                let rest = &stuff[i + "<installedfile>".len()..];
                let installed = rest
                    .split("</installedfile>")
                    .next()
                    .unwrap_or("")
                    .trim_end_matches("/*")
                    .trim_end_matches('/');
                !installed.is_empty()
                    && (path == installed
                        || path.starts_with(&format!("{installed}/"))
                        || installed.starts_with(&format!("{path}/")))
            });
            if provides && tag("providerid").as_deref() == Some("api.kde-look.org") {
                let stem = file.file_stem()?.to_string_lossy().to_string();
                return Some((stem, tag("id")?, tag("name").unwrap_or_default()));
            }
        }
    }
    None
}

/// Our own record of what `install` fetched from the KDE Store (KDE's
/// KNewStuff registry only knows its own "Get New…" installs): installed
/// path → (store id, title), in `<theme-profile data>/installed-assets.json`.
fn own_entry(path: &Path) -> Option<(String, String)> {
    crate::dependency_registry::source_entry(path)
}

fn package_owner(path: &Path) -> Option<(String, bool)> {
    if !path.starts_with("/usr") || which::which("pacman").is_err() {
        return None;
    }
    let (status, out) =
        crate::paths::output_bounded(std::process::Command::new("pacman").arg("-Qqo").arg(path))?;
    if !status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&out)
        .lines()
        .next()?
        .trim()
        .to_string();
    if name.is_empty() {
        return None;
    }
    let in_repo = crate::paths::status_bounded(
        std::process::Command::new("pacman")
            .args(["-Si", &name])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null()),
    )
    .is_some_and(|s| s.success());
    Some((name, in_repo))
}

/// KDE Store categories ("typename" in OCS) a kind may come from.
fn store_types(kind: &str) -> &'static [&'static str] {
    match kind {
        "icons" => &["Icon Themes"],
        "cursors" => &["Cursors"],
        "gtk" => &["GTK"],
        "kvantum" => &["Kvantum"],
        "plasma-theme" => &["Plasma Themes"],
        "look-and-feel" => &["Global Themes"],
        "color-scheme" => &["Color Schemes"],
        "aurorae" => &["Aurorae", "Window Decorations"],
        "plasmoid" => &["Plasma Widgets", "Plasma 6 Widgets", "Plasmoid"],
        "kwin-effect" => &["KWin Effects"],
        _ => &[],
    }
}

fn xml_tag(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let s = text.find(&open)? + open.len();
    let e = text[s..].find(&format!("</{tag}>"))? + s;
    Some(text[s..e].trim().to_string())
}

/// Best KDE Store hit for `name` of `kind` (exact title match preferred).
fn store_lookup(kind: &str, name: &str) -> Option<(String, String, bool)> {
    let types = store_types(kind);
    if types.is_empty() || !crate::net::network_enabled() {
        return None;
    }
    // A bare name ("Orchis") fills the page with other categories (GTK,
    // icons…); retry with the category word ("Orchis kvantum").
    let keyword = match kind {
        "icons" => "icons",
        "cursors" => "cursors",
        "gtk" => "gtk",
        "kvantum" => "kvantum",
        "plasma-theme" => "plasma",
        "look-and-feel" => "global theme",
        "color-scheme" => "colors",
        "aurorae" => "aurorae",
        _ => "plasmoid",
    };
    let mut hits: Vec<(String, String)> = Vec::new();
    for query in [name.to_string(), format!("{name} {keyword}")] {
        let url = format!("{OCS}?search={}&pagesize=50", query.replace(' ', "%20"));
        let Ok(xml) = crate::net::http_get(&url, 8 << 20) else {
            continue;
        };
        for c in xml.split("<content details=\"summary\">").skip(1) {
            let typename = xml_tag(c, "typename").unwrap_or_default();
            if types.iter().any(|t| typename.contains(t)) {
                if let (Some(id), Some(title)) = (xml_tag(c, "id"), xml_tag(c, "name")) {
                    hits.push((id, title));
                }
            }
        }
        if !hits.is_empty() {
            break;
        }
    }
    hits.into_iter()
        .map(|(id, title)| (match_score(name, &title), id, title))
        .filter(|(score, _, _)| *score > 0)
        .max_by_key(|(score, _, _)| *score)
        .map(|(_, id, title)| (id, title, false))
}

/// How well a store title matches an asset name: exact title, the name's
/// acronym (YAMIS = Yet Another Monochrome Icon Set), or containing it; KDE /
/// Plasma builds preferred over forks for other desktops. 0 = no match.
fn match_score(name: &str, title: &str) -> i32 {
    let n = name.to_lowercase();
    let t = title.to_lowercase();
    let words: Vec<&str> = t
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let acronym = |len: usize| {
        words
            .iter()
            .take(len)
            .filter_map(|w| w.chars().next())
            .collect::<String>()
    };
    let mut score = if t == n {
        100
    } else if n.len() >= 3
        && (acronym(n.len()) == n
            || words.windows(n.len()).any(|w| {
                w.iter()
                    .filter_map(|x| x.chars().next())
                    .collect::<String>()
                    == n
            }))
    {
        60
    } else if t.contains(&n) {
        40
    } else {
        return 0;
    };
    if t.contains("kde") || t.contains("plasma") {
        score += 10;
    }
    for other in ["fork", "cosmic", "gnome", "xfce", "windows", "macos"] {
        if t.contains(other) {
            score -= 15;
        }
    }
    score.max(1)
}

/// A dependency for asset `name` of `kind` installed at `paths` (any copy
/// may reveal the source; none when not installed here).
fn describe(kind: &str, name: &str, paths: &[PathBuf]) -> Dependency {
    let mut d = Dependency {
        kind: kind.into(),
        name: name.into(),
        source: "none".into(),
        exact: true,
        ..Default::default()
    };
    for path in paths {
        if let Some((_, id, title)) =
            knewstuff_entry(path).or_else(|| own_entry(path).map(|(i, t)| (String::new(), i, t)))
        {
            d.source = "kde-store".into();
            d.url = store_page(&id);
            d.id = id;
            d.title = title;
            return d;
        }
        if let Some((pkg, in_repo)) = package_owner(path) {
            d.source = if in_repo { "package" } else { "aur" }.into();
            d.url = if in_repo {
                format!("https://archlinux.org/packages/?q={pkg}")
            } else {
                format!("https://aur.archlinux.org/packages/{pkg}")
            };
            d.id = pkg;
            return d;
        }
    }
    if let Some((id, title, exact)) = store_lookup(kind, name) {
        d.source = "kde-store".into();
        d.url = store_page(&id);
        d.id = id;
        d.title = title;
        d.exact = exact;
        return d;
    }
    d.url = store_search(name);
    d
}

/// The dependency a theme asset becomes when it is not shipped (`export
/// --public`): its source detected on this machine (KNewStuff registry,
/// package, KDE Store search). Kvantum is looked up by its family folder.
pub fn for_asset(asset_kind: &str, name: &str, asset_path: &str) -> Option<Dependency> {
    let kind = match asset_kind {
        "color-scheme" => "color-scheme",
        "kvantum" => "kvantum",
        "look-and-feel" => "look-and-feel",
        "desktoptheme" => "plasma-theme",
        _ => return None,
    };
    let live = home_dir().join(asset_path);
    let paths: Vec<PathBuf> = if live.exists() {
        vec![live]
    } else {
        locate_all(kind, name)
    };
    let lookup = if kind == "kvantum" {
        asset_path
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(name)
            .to_string()
    } else {
        name.to_string()
    };
    let mut d = describe(kind, &lookup, &paths);
    d.name = name.to_string();
    Some(d)
}

/// Values that name an asset: field id → dependency kind.
const ASSET_FIELDS: &[(&str, &str)] = &[
    ("icon_theme", "icons"),
    ("cursor_theme", "cursors"),
    ("gtk_theme", "gtk"),
    ("kvantum_theme", "kvantum"),
    ("plasma_theme", "plasma-theme"),
    ("look_and_feel", "look-and-feel"),
    ("color_scheme", "color-scheme"),
];

/// Assets KDE always ships: never a dependency.
fn builtin(kind: &str, name: &str) -> bool {
    let n = name.to_lowercase();
    match kind {
        "plasma-theme" => matches!(
            n.as_str(),
            "default" | "breeze-dark" | "breeze-light" | "oxygen"
        ),
        "look-and-feel" => n.starts_with("org.kde."),
        "color-scheme" => n.starts_with("breeze") || n == "oxygen",
        "icons" => matches!(n.as_str(), "breeze" | "breeze-dark" | "hicolor" | "oxygen"),
        "cursors" => matches!(n.as_str(), "breeze_cursors" | "breeze_light" | "default"),
        "gtk" => matches!(n.as_str(), "breeze" | "breeze-dark" | "adwaita" | "default"),
        _ => false,
    }
}

/// Dependencies of the theme stored in `dir` (its `values` already read).
/// Assets bundled in the theme directory are not dependencies.
pub fn detect(
    dir: &Path,
    values: &std::collections::BTreeMap<String, Value>,
    sources: &std::collections::BTreeMap<String, crate::theme::Source>,
) -> Vec<Dependency> {
    let home = home_dir();
    let mut out: Vec<Dependency> = Vec::new();
    let bundled = |path: &Path| {
        path.strip_prefix(&home)
            .is_ok_and(|rel| dir.join(rel).exists())
    };
    let mut add = |d: Dependency| {
        if !out.iter().any(|x| x.kind == d.kind && x.name == d.name) {
            out.push(d);
        }
    };
    for (field, kind) in ASSET_FIELDS {
        let Some(name) = values
            .get(*field)
            .map(crate::field::value_str)
            .filter(|n| safe_name(n))
        else {
            continue;
        };
        if builtin(kind, &name) {
            continue;
        }
        let paths = locate_all(kind, &name);
        if paths.first().is_some_and(|p| bundled(p)) {
            continue;
        }
        add(describe(kind, &name, &paths));
    }
    // Window decoration: a compiled plugin (package) or an Aurorae theme.
    let library = values
        .get("decoration_library")
        .map(crate::field::value_str)
        .unwrap_or_default();
    let deco_theme = values
        .get("decoration_theme")
        .map(crate::field::value_str)
        .unwrap_or_default();
    if library == "org.kde.kwin.aurorae" {
        if let Some(name) = deco_theme
            .strip_prefix("__aurorae__svg__")
            .filter(|n| safe_name(n))
        {
            let paths = locate_all("aurorae", name);
            if !paths.first().is_some_and(|p| bundled(p)) {
                add(describe("aurorae", name, &paths));
            }
        }
    } else if !library.is_empty() && library != "org.kde.breeze" && library != "org.kde.oxygen" {
        let plugin: Vec<PathBuf> = decoration_plugin(&library).into_iter().collect();
        let mut d = describe("decoration", &library, &plugin);
        if d.source == "kde-store" && !d.exact {
            d.source = "none".into(); // a compiled plugin is never a store archive
            d.url = format!(
                "https://aur.archlinux.org/packages?K={}",
                library.rsplit('.').next().unwrap_or(&library)
            );
        }
        add(d);
    }
    // A Qt widget style that is a plugin of its own (Kvantum): its package.
    let style = values
        .get("widget_style")
        .map(crate::field::value_str)
        .unwrap_or_default();
    if let Some(plugin) = widget_style_plugin(&style) {
        let mut d = describe("widget-style", &style, &[plugin]);
        if d.source == "kde-store" && !d.exact {
            d.source = "none".into();
        }
        add(d);
    } else if style.eq_ignore_ascii_case("kvantum") {
        // Not installed here: Kvantum is a package everywhere.
        add(Dependency {
            kind: "widget-style".into(),
            name: "kvantum".into(),
            source: "package".into(),
            id: "kvantum".into(),
            title: "Kvantum (Qt widget style engine)".into(),
            url: "https://archlinux.org/packages/?q=kvantum".into(),
            exact: true,
        });
    }
    // Third-party KWin effects the theme switches on (Better Blur DX, Rounded
    // Corners…): their package, or the store item of a scripted effect.
    if let Ok(kwinrc) = fs::read_to_string(dir.join(".config/kwinrc")) {
        for id in crate::kwin_effects::enabled_third_party(&kwinrc) {
            let paths = locate_all("kwin-effect", &id);
            add(describe("kwin-effect", &id, &paths));
        }
    }
    // Panel widgets the theme styles and the user installed (system ones come
    // with Plasma); a switched-off Panel Colorizer styles nothing.
    if let Ok(text) = fs::read_to_string(dir.join(crate::wallpaper::APPLETSRC_REL)) {
        for plugin in crate::layout::needed_plasmoids(&text) {
            let user = data_home().join("plasma/plasmoids").join(&plugin);
            if safe_name(&plugin) && user.exists() {
                add(describe("plasmoid", &plugin, &[user]));
            }
        }
    }
    // Discord themes need a theming client (stock Discord has none).
    let carries_discord = crate::discord::CLIENTS
        .iter()
        .any(|(_, rel)| dir.join(rel).join("themes").exists());
    if carries_discord {
        add(Dependency {
            kind: "discord-client".into(),
            name: "Vesktop".into(),
            source: "aur".into(),
            id: "vesktop-bin".into(),
            title: "Vesktop (Discord with Vencord) — or install Vencord into Discord: https://vencord.dev/download/".into(),
            url: "https://vencord.dev/download/".into(),
            exact: true,
        });
    }
    // A prompt theme needs oh-my-posh.
    if dir.join(crate::extras::OMP_THEME).is_file() {
        add(Dependency {
            kind: "oh-my-posh".into(),
            name: "oh-my-posh".into(),
            source: "aur".into(),
            id: "oh-my-posh-bin".into(),
            title: "oh-my-posh (prompts usually need a Nerd Font)".into(),
            url: "https://ohmyposh.dev/docs/installation/linux".into(),
            exact: true,
        });
    }
    // Spicetify themes need Spicetify (and Spotify).
    if dir.join(crate::spicetify::CONFIG).is_file() {
        add(Dependency {
            kind: "spicetify".into(),
            name: "spicetify".into(),
            source: "aur".into(),
            id: "spicetify-cli".into(),
            title: "Spicetify (needs Spotify: spotify-launcher)".into(),
            url: "https://spicetify.app/docs/getting-started".into(),
            exact: true,
        });
    }
    // Firefox theme add-on (installed from inside Firefox — link only).
    for user_js in crate::tracked_apps::firefox_config_paths()
        .iter()
        .filter(|p| p.ends_with("/user.js"))
    {
        let theme_copy = crate::profile::theme_file(dir, user_js);
        let Ok(text) = fs::read_to_string(theme_copy) else {
            continue;
        };
        let Some(guid) = text.lines().find_map(|l| {
            let rest = l
                .trim()
                .strip_prefix("user_pref(\"extensions.activeThemeID\",")?;
            Some(
                rest.trim()
                    .trim_end_matches(");")
                    .trim()
                    .trim_matches('"')
                    .to_string(),
            )
        }) else {
            continue;
        };
        let builtin = guid.ends_with("@mozilla.org") || guid.is_empty();
        if !builtin && !guid.contains('/') {
            add(Dependency {
                kind: "firefox-theme".into(),
                url: format!("https://addons.mozilla.org/firefox/addon/{guid}/"),
                name: guid.clone(),
                source: "amo".into(),
                id: guid,
                exact: true,
                ..Default::default()
            });
        }
    }
    // Editor themes from their official stores.
    for (field, kind) in [
        ("vscode_theme", "vscode"),
        ("cursor_ide_theme", "cursor"),
        ("zed_dark", "zed"),
        ("zed_light", "zed"),
    ] {
        if let (Some(src), Some(name)) = (
            sources.get(field),
            values.get(field).map(crate::field::value_str),
        ) {
            let url = match src.store.as_str() {
                "vscode-marketplace" => format!(
                    "https://marketplace.visualstudio.com/items?itemName={}",
                    src.id
                ),
                "open-vsx" => format!(
                    "https://open-vsx.org/extension/{}",
                    src.id.replacen('.', "/", 1)
                ),
                "zed" => format!("https://zed.dev/extensions?query={}", src.id),
                _ => src.url.clone(),
            };
            add(Dependency {
                kind: kind.into(),
                name,
                source: src.store.clone(),
                id: src.id.clone(),
                url,
                exact: true,
                ..Default::default()
            });
        }
    }
    out
}

/// Dependencies not installed here, with their index in `deps`.
/// Dependencies not installed here. Editor themes for an editor this machine
/// does not have are left out — nothing to download, nothing to report.
pub fn missing(deps: &[Dependency]) -> Vec<(usize, &Dependency)> {
    deps.iter()
        .enumerate()
        .filter(|(_, d)| {
            crate::stores::editor_of(&d.kind).is_none_or(crate::stores::editor_installed)
        })
        .filter(|(_, d)| !installed(d))
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Install
// ─────────────────────────────────────────────────────────────────────────────

/// Dependencies that need root (system packages): installed
/// in a terminal the user sees, where they type their own password.
pub fn needs_root(d: &Dependency) -> bool {
    matches!(d.source.as_str(), "package" | "aur")
}

fn safe_package(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.+@".contains(c))
}

/// One shell command for every root dependency in `deps`: one
/// `yay/paru -S --needed …` (or `sudo pacman -S --needed …` when only repo
/// packages and no AUR helper). None when there is
/// nothing to run or AUR packages need a helper that is missing.
pub fn root_command(deps: &[&Dependency]) -> Option<String> {
    let mut packages: Vec<&str> = Vec::new();
    let mut aur = false;
    for d in deps {
        if matches!(d.source.as_str(), "package" | "aur")
            && safe_package(&d.id)
            && !packages.contains(&d.id.as_str())
        {
            packages.push(&d.id);
            aur |= d.source == "aur";
        }
    }
    let helper = ["yay", "paru"]
        .into_iter()
        .find(|h| which::which(h).is_ok());
    if packages.is_empty() {
        return None;
    }
    match helper {
        Some(h) => Some(format!("{h} -S --needed {}", packages.join(" "))),
        None if !aur => Some(format!("sudo pacman -S --needed {}", packages.join(" "))),
        None => None,
    }
}

/// Run `command` in a terminal window the user sees (konsole, kitty,
/// alacritty, ghostty — the first installed). The Rust terminal worker publishes
/// the package result before exit, without an extra Enter prompt.
pub fn run_in_terminal(command: &str) -> Result<String> {
    if std::env::var("KTM_THEME_NO_APPLY").as_deref() == Ok("1") {
        return Ok("no terminal (KTM_THEME_NO_APPLY)".into());
    }
    let work = tempfile::Builder::new().prefix("ktm-package-").tempdir()?;
    let result = work.path().join("result.json");
    let exe = std::env::current_exe()?;
    for (bin, args) in [
        ("konsole", vec!["--nofork", "-e"]),
        ("kitty", vec![]),
        ("alacritty", vec!["-e"]),
        ("ghostty", vec!["-e"]),
    ] {
        if which::which(bin).is_err() {
            continue;
        }
        crate::dependency_jobs::event("waiting", "System packages: see the terminal");
        let mut child = std::process::Command::new(bin)
            .args(args)
            .arg(&exe)
            .args(["dependencies", "terminal-worker"])
            .arg(&result)
            .args(command.split_whitespace())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3600);
        loop {
            if let Ok(bytes) = fs::read(&result) {
                let value: Value = serde_json::from_slice(&bytes)?;
                if value["ok"] == true {
                    return Ok(bin.into());
                }
                bail!("package manager failed; see the terminal output, then refresh and retry");
            }
            if child.try_wait()?.is_some() {
                bail!("terminal closed before the package result was received");
            }
            if std::time::Instant::now() > deadline {
                // Do not terminate a privileged package transaction. Its state
                // must be checked after it finishes in the terminal.
                bail!("package transaction is still running in the terminal; check it and refresh before retrying");
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    bail!("no terminal found (konsole, kitty, alacritty, ghostty) — run: {command}")
}

/// Command the user runs for a package dependency (we never sudo).
pub fn package_command(d: &Dependency) -> Option<String> {
    match d.source.as_str() {
        "package" => Some(format!("sudo pacman -S {}", d.id)),
        "aur" => Some(format!("yay -S {}", d.id)),
        _ => None,
    }
}

fn kpackage_type(kind: &str) -> Option<&'static str> {
    match kind {
        "plasma-theme" => Some("Plasma/Theme"),
        "look-and-feel" => Some("Plasma/LookAndFeel"),
        "plasmoid" => Some("Plasma/Applet"),
        _ => None,
    }
}

fn target_root(kind: &str) -> Option<PathBuf> {
    roots(kind).into_iter().next()
}

/// Theme directories inside an unpacked archive, by what marks a theme of
/// `kind` (index.theme, cursors/, gtk-3.0/, *.kvconfig, …), with the folder
/// name to install them under. A theme packed without a folder (files at the
/// archive root, e.g. `Sandstone.kvconfig`) is named after its file.
fn theme_dirs(root: &Path, kind: &str) -> Vec<(PathBuf, String)> {
    let mut out: Vec<(PathBuf, String)> = theme_subdirs(root, kind)
        .into_iter()
        .filter_map(|d| {
            d.file_name()
                .map(|n| (d.clone(), n.to_string_lossy().to_string()))
        })
        .collect();
    if out.is_empty() && kind == "kvantum" {
        if let Some(stem) = fs::read_dir(root)
            .ok()
            .and_then(|it| {
                it.flatten()
                    .map(|e| e.path())
                    .find(|p| p.extension().is_some_and(|x| x == "kvconfig"))
            })
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
        {
            out.push((root.to_path_buf(), stem));
        }
    }
    out
}

fn theme_subdirs(root: &Path, kind: &str) -> Vec<PathBuf> {
    let marks = |d: &Path| -> bool {
        match kind {
            "icons" => d.join("index.theme").is_file() && !d.join("cursors").exists(),
            "cursors" => d.join("cursors").is_dir(),
            "gtk" => d.join("gtk-3.0").is_dir() || d.join("gtk-4.0").is_dir(),
            "kvantum" => fs::read_dir(d).ok().is_some_and(|mut it| {
                it.any(|e| e.is_ok_and(|e| e.path().extension().is_some_and(|x| x == "kvconfig")))
            }),
            "aurorae" => fs::read_dir(d).ok().is_some_and(|mut it| {
                it.any(|e| {
                    e.is_ok_and(|e| {
                        e.file_name()
                            .to_string_lossy()
                            .starts_with("decoration.svg")
                    })
                })
            }),
            _ => false,
        }
    };
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if marks(&dir) && dir != root {
            out.push(dir);
            continue;
        }
        if let Ok(it) = fs::read_dir(&dir) {
            for e in it.flatten() {
                let p = e.path();
                if fs::symlink_metadata(&p).is_ok_and(|m| m.is_dir()) {
                    stack.push(p);
                }
            }
        }
    }
    out
}

/// Unpack `archive` into `dest` (zip via the zip crate with enclosed names;
/// tarballs via `tar`, which refuses absolute and `..` member paths), then
/// drop symlinks that point outside `dest`.
fn unpack(archive: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    let name = archive
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if name.ends_with(".zip") {
        let mut zip = zip::ZipArchive::new(fs::File::open(archive)?)?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let Some(rel) = entry.enclosed_name() else {
                continue;
            };
            let out = dest.join(rel);
            if entry.is_dir() {
                fs::create_dir_all(&out)?;
            } else if !entry.is_symlink() {
                if let Some(p) = out.parent() {
                    fs::create_dir_all(p)?;
                }
                let mut f = fs::File::create(&out)?;
                std::io::copy(&mut entry, &mut f)?;
            }
        }
    } else {
        let ok = crate::paths::status_bounded(
            std::process::Command::new("tar")
                .arg("-xf")
                .arg(archive)
                .arg("-C")
                .arg(dest)
                .arg("--no-same-owner")
                .arg("--warning=no-unknown-keyword")
                .stderr(std::process::Stdio::null()),
        )
        .is_some_and(|s| s.success());
        if !ok {
            bail!("could not unpack {}", archive.display());
        }
    }
    // Symlinks must stay inside the unpacked tree (icon themes use relative ones).
    let mut stack = vec![dest.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir)?.flatten() {
            let p = e.path();
            let meta = fs::symlink_metadata(&p)?;
            if meta.file_type().is_symlink() {
                let target = fs::read_link(&p)?;
                let escapes = target.is_absolute()
                    || target
                        .components()
                        .filter(|c| matches!(c, std::path::Component::ParentDir))
                        .count()
                        > p.strip_prefix(dest)
                            .map(|r| r.components().count())
                            .unwrap_or(0)
                            .saturating_sub(1);
                if escapes {
                    fs::remove_file(&p)?;
                }
            } else if meta.is_dir() {
                stack.push(p);
            }
        }
    }
    Ok(())
}

/// Download links of a KDE Store item, `(url, file name)`.
fn store_files(id: &str) -> Result<Vec<(String, String)>> {
    if !id.chars().all(|c| c.is_ascii_digit()) {
        bail!("invalid KDE Store id: {id}");
    }
    let xml = crate::net::http_get(&format!("{OCS}/{id}"), 4 << 20)?;
    let mut files = Vec::new();
    for n in 1..=50 {
        let (Some(link), Some(name)) = (
            xml_tag(&xml, &format!("downloadlink{n}")),
            xml_tag(&xml, &format!("downloadname{n}")),
        ) else {
            if n > 1 {
                break;
            }
            continue;
        };
        files.push((link.replace("&amp;", "&"), name));
    }
    if files.is_empty() {
        bail!("KDE Store item {id} has no downloadable files");
    }
    Ok(files)
}

/// Does unpacked `dir` hold the theme the dependency names (e.g. a Kvantum
/// family with `OrchisDark.kvconfig`, an icon theme folder `YAMIS`)?
fn holds(found: &[(PathBuf, String)], kind: &str, name: &str) -> bool {
    found.iter().any(|(dir, folder)| match kind {
        "kvantum" => dir.join(format!("{name}.kvconfig")).is_file() || folder == name,
        _ => folder == name,
    })
}

fn install_from_store(d: &Dependency) -> Result<String> {
    let mut files = store_files(&d.id)?;
    let wanted = d.name.to_lowercase();
    // Files whose name mentions the theme first; an item can hold several
    // variants (Orchis: Orchis-solid.tar.gz, Orchis.tar.gz).
    files.sort_by_key(|(_, n)| !n.to_lowercase().contains(&wanted));
    let work = tempfile::Builder::new()
        .prefix("theme-deps-")
        .tempdir()
        .context("temp dir")?;
    let unpackable = !matches!(
        d.kind.as_str(),
        "plasma-theme" | "look-and-feel" | "plasmoid" | "color-scheme"
    );
    let mut chosen: Option<(PathBuf, String, PathBuf)> = None;
    let mut fallback: Option<(PathBuf, String, PathBuf)> = None;
    for (i, (link, name)) in files.iter().enumerate().take(8) {
        let file_name = Path::new(name)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "download".into());
        let bytes = crate::net::http_get_bytes(link, 512 << 20)?;
        let archive = work.path().join(format!("{i}-{file_name}"));
        fs::write(&archive, &bytes)?;
        let out = work.path().join(format!("x{i}"));
        if !unpackable {
            chosen = Some((archive, file_name, out));
            break;
        }
        if unpack(&archive, &out).is_err() {
            continue;
        }
        let found = theme_dirs(&out, &d.kind);
        if holds(&found, &d.kind, &d.name) {
            chosen = Some((archive, file_name, out));
            break;
        }
        if fallback.is_none() && !found.is_empty() {
            fallback = Some((archive, file_name, out));
        }
    }
    let chosen = chosen.or(fallback);
    let Some((archive, file_name, out)) = chosen else {
        bail!(
            "KDE Store item {} has no file holding a {} theme",
            d.id,
            d.kind
        );
    };

    if let Some(kind) = kpackage_type(&d.kind) {
        let run = |op: &str| {
            crate::paths::status_bounded(
                std::process::Command::new("kpackagetool6")
                    .args(["--type", kind, op])
                    .arg(&archive)
                    .stderr(std::process::Stdio::null()),
            )
            .is_some_and(|s| s.success())
        };
        if !(run("--install") || run("--upgrade")) {
            bail!("kpackagetool6 could not install {file_name}");
        }
        if let Some(p) = locate(&d.kind, &d.name) {
            crate::dependency_registry::record_source(&[p], &d.id, &d.title)?;
        }
        return Ok(format!(
            "installed {} ({file_name}) with kpackagetool6",
            d.title
        ));
    }

    let root = target_root(&d.kind).ok_or_else(|| anyhow!("no install location for {}", d.kind))?;
    fs::create_dir_all(&root)?;
    if d.kind == "color-scheme" {
        let mut copied = 0;
        let scan = if file_name.to_lowercase().ends_with(".colors") {
            vec![archive.clone()]
        } else {
            let out = work.path().join("x");
            unpack(&archive, &out)?;
            let mut found = Vec::new();
            let mut stack = vec![out];
            while let Some(dir) = stack.pop() {
                for e in fs::read_dir(&dir)?.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if p.extension().is_some_and(|x| x == "colors") {
                        found.push(p);
                    }
                }
            }
            found
        };
        for f in scan {
            let basename = if f == archive {
                Path::new(&file_name).file_name().unwrap_or_default()
            } else {
                f.file_name().unwrap_or_default()
            };
            let dst = root.join(basename);
            if !dst.exists() {
                let mut tmp = tempfile::NamedTempFile::new_in(&root)?;
                std::io::copy(&mut fs::File::open(&f)?, &mut tmp)?;
                tmp.persist_noclobber(&dst).map_err(|e| e.error)?;
                crate::dependency_registry::record_source(
                    std::slice::from_ref(&dst),
                    &d.id,
                    &d.title,
                )?;
                copied += 1;
            }
        }
        return Ok(format!(
            "installed {copied} colour scheme(s) from {}",
            d.title
        ));
    }

    let dirs = theme_dirs(&out, &d.kind);
    if dirs.is_empty() {
        bail!("{file_name} holds no {} theme", d.kind);
    }
    let mut placed = Vec::new();
    for (dir, name) in dirs {
        let dst = root.join(&name);
        if dst.exists() {
            continue; // never overwrite the user's own copy
        }
        let staging = tempfile::Builder::new()
            .prefix(".ktm-install-")
            .tempdir_in(&root)?;
        let prepared = staging.path().join("resource");
        crate::profile::copy_tree(&dir, &prepared)?;
        let mut note = String::new();
        if d.kind == "icons" && add_breeze_fallback(&prepared.join("index.theme")) {
            note = " (Breeze added as fallback for icons it lacks)".into();
        }
        fs::rename(&prepared, &dst)?;
        crate::dependency_registry::record_source(std::slice::from_ref(&dst), &d.id, &d.title)?;
        placed.push(format!("{name}{note}"));
    }
    Ok(format!(
        "installed {} into {}: {}",
        d.title,
        root.display(),
        placed.join(", ")
    ))
}

/// Icon themes made for GNOME (Seven Icons: `Inherits=Humanity,…,gnome,hicolor`)
/// lack KDE's action icons — `configure`, `document-edit`… — so icon-only
/// buttons in Plasma (Desktop Effects' "Configure") come out empty. Put
/// `breeze` before `hicolor` in the inheritance chain of an icon theme we
/// installed. Returns whether the file changed.
fn add_breeze_fallback(index: &Path) -> bool {
    let Ok(text) = fs::read_to_string(index) else {
        return false;
    };
    match with_breeze_fallback(&text) {
        Some(new) => fs::write(index, new).is_ok(),
        None => false,
    }
}

fn with_breeze_fallback(index_theme: &str) -> Option<String> {
    let mut changed = false;
    let mut in_main = false;
    let mut out: Vec<String> = Vec::new();
    for line in index_theme.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_main = t == "[Icon Theme]";
        }
        if let Some(value) = t.strip_prefix("Inherits=").filter(|_| in_main) {
            let mut parents: Vec<&str> = value
                .split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .collect();
            if !parents
                .iter()
                .any(|p| p.eq_ignore_ascii_case("breeze") || p.eq_ignore_ascii_case("breeze-dark"))
            {
                let at = parents
                    .iter()
                    .position(|p| p.eq_ignore_ascii_case("hicolor"))
                    .unwrap_or(parents.len());
                parents.insert(at, "breeze");
                out.push(format!("Inherits={}", parents.join(",")));
                changed = true;
                continue;
            }
        }
        out.push(line.to_string());
    }
    changed.then(|| out.join("\n") + "\n")
}

fn store_message(r: &crate::stores::InstallReport) -> String {
    let themes = if r.themes.is_empty() {
        String::new()
    } else {
        format!(" — themes: {}", r.themes.join(", "))
    };
    format!("installed {} for {}{themes}", r.id, r.app)
}

/// Install dependency `d` from its source. Packages are not installed (no
/// sudo): the error carries the command to run.
pub fn install(d: &Dependency) -> Result<String> {
    // Legacy API never launches a package manager by itself. Interactive
    // package jobs and theme batches have their own explicit terminal path.
    if needs_root(d) {
        bail!(
            "install it with: {}",
            package_command(d).unwrap_or_default()
        );
    }
    let _guard = crate::dependency_registry::lock(&crate::dependency_registry::resource_id(d))?;
    if installed(d) {
        return Ok(format!("Already installed: {}", d.name));
    }
    let before = crate::dependency_registry::inventory(d);
    crate::dependency_jobs::event("installing", &d.name);
    let result = install_unrecorded(d);
    crate::dependency_registry::record_result(d, &before, &result)?;
    result
}

pub(crate) fn install_unrecorded(d: &Dependency) -> Result<String> {
    if std::env::var("KTM_THEME_NO_APPLY").as_deref() == Ok("1") {
        bail!("installation disabled by KTM_THEME_NO_APPLY=1");
    }
    let result = match d.source.as_str() {
        "kde-store" => install_from_store(d),
        "vscode-marketplace" | "open-vsx" => crate::stores::install_unrecorded(
            if d.kind == "cursor" {
                "cursor"
            } else {
                "vscode"
            },
            &d.id,
        )
        .map(|r| store_message(&r)),
        "zed" => crate::stores::install_unrecorded("zed", &d.id).map(|r| store_message(&r)),
        "package" | "aur" => {
            let command = root_command(&[d])
                .ok_or_else(|| anyhow!("no supported package manager for {}", d.id))?;
            run_in_terminal(&command).map(|term| format!("installed in {term}: {command}"))
        }
        "amo" => bail!("install it from inside Firefox: {}", d.url),
        _ => bail!("no known source — look at {}", d.url),
    }?;
    crate::dependency_jobs::event("checking", &d.name);
    if !installed(d) {
        bail!(
            "{result}, but \"{}\" is still not found — check {}",
            d.name,
            d.url
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_dependencies_make_one_command() {
        let dep = |kind: &str, source: &str, id: &str| Dependency {
            kind: kind.into(),
            name: id.into(),
            source: source.into(),
            id: id.into(),
            ..Default::default()
        };
        let (a, b, c) = (
            dep("widget-style", "package", "kvantum"),
            dep("kwin-effect", "package", "kvantum"),
            dep("icons", "kde-store", "1"),
        );
        assert!(needs_root(&a) && !needs_root(&c));
        let cmd = root_command(&[&a, &b]).unwrap();
        assert!(
            cmd.ends_with("-S --needed kvantum"),
            "one package once: {cmd}"
        );
        let bad = dep("gtk", "package", "x; rm -rf ~");
        assert_eq!(
            root_command(&[&bad]),
            None,
            "unsafe names never reach a shell"
        );
    }

    #[test]
    fn icon_themes_get_a_breeze_fallback() {
        let seven = "[Icon Theme]\nName=Windows 7\nInherits=Humanity-Dark,Humanity,gnome,hicolor\nDirectories=actions/16x16\n\n[actions/16x16]\nSize=16\n";
        let out = with_breeze_fallback(seven).unwrap();
        assert!(
            out.contains("Inherits=Humanity-Dark,Humanity,gnome,breeze,hicolor\n"),
            "{out}"
        );
        assert!(out.contains("[actions/16x16]\nSize=16"));
        assert_eq!(with_breeze_fallback(&out), None, "idempotent");
        assert_eq!(
            with_breeze_fallback("[Icon Theme]\nInherits=breeze-dark\n"),
            None
        );
        assert!(with_breeze_fallback("[Icon Theme]\nInherits=Papirus\n")
            .unwrap()
            .contains("Inherits=Papirus,breeze\n"));
    }

    #[test]
    fn search_ranking_prefers_the_kde_build() {
        let kde = match_score("YAMIS", "Yet Another Monochrome Icon Set For KDE Plasma");
        let fork = match_score(
            "YAMIS",
            "Yet Another Monochrome Icon Set fork for Cosmic Desktop",
        );
        let single = match_score("YAMIS", "Antigravity IDE - YAMIS White Icon");
        assert!(kde > fork && kde > single, "{kde} {fork} {single}");
        assert_eq!(match_score("Orchis kvantum", "Orchis kvantum"), 100);
        assert_eq!(match_score("Candy", "Plasmorphism"), 0);
    }

    #[test]
    fn store_xml_tags() {
        let xml = "<data><content details=\"full\"><id>1305251</id><downloadlink1>https://x/y?a=1&amp;b=2</downloadlink1><downloadname1>candy-icons.tar.xz</downloadname1></content></data>";
        assert_eq!(xml_tag(xml, "id").as_deref(), Some("1305251"));
        assert_eq!(
            xml_tag(xml, "downloadname1").as_deref(),
            Some("candy-icons.tar.xz")
        );
    }

    #[test]
    fn package_commands_never_run_sudo_themselves() {
        let d = Dependency {
            source: "aur".into(),
            id: "klassy".into(),
            ..Default::default()
        };
        assert_eq!(package_command(&d).as_deref(), Some("yay -S klassy"));
        assert!(install(&d)
            .unwrap_err()
            .to_string()
            .contains("yay -S klassy"));
    }

    #[test]
    fn theme_dirs_found_by_marker() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        fs::create_dir_all(r.join("pkg/Candy/apps")).unwrap();
        fs::write(r.join("pkg/Candy/index.theme"), "[Icon Theme]\n").unwrap();
        fs::create_dir_all(r.join("pkg/Cur/cursors")).unwrap();
        fs::write(r.join("pkg/Cur/index.theme"), "").unwrap();
        fs::create_dir_all(r.join("Orchis")).unwrap();
        fs::write(r.join("Orchis/OrchisDark.kvconfig"), "").unwrap();
        assert_eq!(
            theme_dirs(r, "icons"),
            vec![(r.join("pkg/Candy"), "Candy".to_string())]
        );
        assert_eq!(
            theme_dirs(r, "cursors"),
            vec![(r.join("pkg/Cur"), "Cur".to_string())]
        );
        let kv = theme_dirs(r, "kvantum");
        assert_eq!(kv, vec![(r.join("Orchis"), "Orchis".to_string())]);
        assert!(
            holds(&kv, "kvantum", "OrchisDark"),
            "a variant inside the family folder"
        );
        // Files at the archive root (Sandstone): named after the .kvconfig.
        let flat = tempfile::tempdir().unwrap();
        fs::write(flat.path().join("Sandstone.kvconfig"), "").unwrap();
        assert_eq!(
            theme_dirs(flat.path(), "kvantum"),
            vec![(flat.path().to_path_buf(), "Sandstone".to_string())]
        );
    }

    #[test]
    fn unpack_drops_escaping_symlinks() {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("src");
        fs::create_dir_all(src.join("Theme/a")).unwrap();
        fs::write(src.join("Theme/a/x.svg"), "x").unwrap();
        std::os::unix::fs::symlink("x.svg", src.join("Theme/a/ok.svg")).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", src.join("Theme/a/evil")).unwrap();
        let archive = t.path().join("t.tar.gz");
        assert!(std::process::Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(&src)
            .arg("Theme")
            .status()
            .unwrap()
            .success());
        let out = t.path().join("out");
        unpack(&archive, &out).unwrap();
        assert!(fs::symlink_metadata(out.join("Theme/a/ok.svg")).is_ok());
        assert!(fs::symlink_metadata(out.join("Theme/a/evil")).is_err());
    }
}
