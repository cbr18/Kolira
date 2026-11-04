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
//! through the stores module. System packages are never installed by us
//! (no sudo): the report carries the command.

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
    format!("https://store.kde.org/search?search={}", name.replace(' ', "+"))
}

fn safe_name(name: &str) -> bool {
    !name.is_empty() && !name.contains('/') && !name.starts_with('.')
}

// ─────────────────────────────────────────────────────────────────────────────
// Where assets live
// ─────────────────────────────────────────────────────────────────────────────

fn roots(kind: &str) -> Vec<PathBuf> {
    let home = home_dir();
    let data = data_home();
    match kind {
        "icons" | "cursors" => vec![data.join("icons"), home.join(".icons"), "/usr/share/icons".into()],
        "gtk" => vec![data.join("themes"), home.join(".themes"), "/usr/share/themes".into()],
        "kvantum" => vec![config_home().join("Kvantum"), "/usr/share/Kvantum".into()],
        "plasma-theme" => vec![data.join("plasma/desktoptheme"), "/usr/share/plasma/desktoptheme".into()],
        "look-and-feel" => vec![data.join("plasma/look-and-feel"), "/usr/share/plasma/look-and-feel".into()],
        "color-scheme" => vec![data.join("color-schemes"), "/usr/share/color-schemes".into()],
        "aurorae" => vec![data.join("aurorae/themes"), "/usr/share/aurorae/themes".into()],
        "plasmoid" => vec![data.join("plasma/plasmoids"), "/usr/share/plasma/plasmoids".into()],
        "sddm" => vec!["/usr/share/sddm/themes".into()],
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
fn locate_all(kind: &str, name: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !safe_name(name) {
        return out;
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
                        it.flatten().map(|e| e.path()).find(|d| d.join(format!("{name}.kvconfig")).is_file())
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
    for dir in ["/usr/lib/qt6/plugins/org.kde.kdecoration3", "/usr/lib/qt6/plugins/org.kde.kdecoration2"] {
        if let Ok(it) = fs::read_dir(dir) {
            if let Some(p) = it.flatten().map(|e| e.path()).find(|p| {
                p.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase().contains(&short))
            }) {
                return Some(p);
            }
        }
    }
    None
}

/// SDDM's current theme (`[Theme] Current`): KDE writes it to
/// `/etc/sddm.conf.d/kde_settings.conf`; `/etc/sddm.conf` is the fallback.
pub fn sddm_current() -> Option<String> {
    let mut files: Vec<PathBuf> = fs::read_dir("/etc/sddm.conf.d")
        .map(|it| it.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    files.sort();
    files.insert(0, PathBuf::from("/etc/sddm.conf"));
    // Later files win, as in SDDM itself.
    files
        .iter()
        .rev()
        .filter_map(|f| fs::read_to_string(f).ok())
        .filter_map(|t| crate::field::ini_get(&t, "Theme", "Current"))
        .find(|v| !v.is_empty())
}

/// Is dependency `d` satisfied on this machine?
pub fn installed(d: &Dependency) -> bool {
    match d.kind.as_str() {
        "sddm" => locate("sddm", &d.name).is_some() && sddm_current().as_deref() == Some(d.name.as_str()),
        "decoration" => decoration_plugin(&d.name).is_some(),
        "spicetify" => which::which("spicetify").is_ok(),
        "oh-my-posh" => which::which("oh-my-posh").is_ok() || home_dir().join(".local/bin/oh-my-posh").is_file(),
        "discord-client" => crate::discord::CLIENTS.iter().any(|(_, rel)| home_dir().join(rel).exists()),
        "firefox-theme" => crate::tracked_apps::firefox_profile_dir().is_some_and(|dir| {
            home_dir().join(".mozilla/firefox").join(dir).join("extensions").join(format!("{}.xpi", d.name)).exists()
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
        let Ok(text) = fs::read_to_string(&file) else { continue };
        for stuff in text.split("<stuff").skip(1) {
            let tag = |t: &str| -> Option<String> {
                let open = format!("<{t}>");
                let s = stuff.find(&open)? + open.len();
                let e = stuff[s..].find(&format!("</{t}>"))? + s;
                Some(stuff[s..e].trim().to_string())
            };
            let provides = stuff.match_indices("<installedfile>").any(|(i, _)| {
                let rest = &stuff[i + "<installedfile>".len()..];
                let installed = rest.split("</installedfile>").next().unwrap_or("").trim_end_matches("/*").trim_end_matches('/');
                !installed.is_empty() && (path == installed || path.starts_with(&format!("{installed}/")) || installed.starts_with(&format!("{path}/")))
            });
            if provides && tag("providerid").as_deref() == Some("api.kde-look.org") {
                let stem = file.file_stem()?.to_string_lossy().to_string();
                return Some((stem, tag("id")?, tag("name").unwrap_or_default()));
            }
        }
    }
    None
}

fn package_owner(path: &Path) -> Option<(String, bool)> {
    if !path.starts_with("/usr") || which::which("pacman").is_err() {
        return None;
    }
    let out = std::process::Command::new("pacman").arg("-Qqo").arg(path).output().ok()?;
    let name = String::from_utf8_lossy(&out.stdout).lines().next()?.trim().to_string();
    if name.is_empty() {
        return None;
    }
    let in_repo = std::process::Command::new("pacman")
        .args(["-Si", &name])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
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
        "aurorae" => &["Aurorae"],
        "plasmoid" => &["Plasma Widgets", "Plasma 6 Widgets", "Plasmoid"],
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
    let url = format!("{OCS}?search={}&pagesize=20", name.replace(' ', "%20"));
    let xml = crate::net::http_get(&url, 4 << 20).ok()?;
    let mut hits: Vec<(String, String)> = Vec::new();
    for c in xml.split("<content details=\"summary\">").skip(1) {
        let typename = xml_tag(c, "typename").unwrap_or_default();
        if types.iter().any(|t| typename.contains(t)) {
            if let (Some(id), Some(title)) = (xml_tag(c, "id"), xml_tag(c, "name")) {
                hits.push((id, title));
            }
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
    let words: Vec<&str> = t.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let acronym = |len: usize| words.iter().take(len).filter_map(|w| w.chars().next()).collect::<String>();
    let mut score = if t == n {
        100
    } else if n.len() >= 3 && (acronym(n.len()) == n || words.windows(n.len()).any(|w| w.iter().filter_map(|x| x.chars().next()).collect::<String>() == n)) {
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
    let mut d = Dependency { kind: kind.into(), name: name.into(), source: "none".into(), exact: true, ..Default::default() };
    for path in paths {
        if let Some((_, id, title)) = knewstuff_entry(path) {
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
        "plasma-theme" => matches!(n.as_str(), "default" | "breeze-dark" | "breeze-light" | "oxygen"),
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
pub fn detect(dir: &Path, values: &std::collections::BTreeMap<String, Value>, sources: &std::collections::BTreeMap<String, crate::theme::Source>) -> Vec<Dependency> {
    let home = home_dir();
    let mut out: Vec<Dependency> = Vec::new();
    let bundled = |path: &Path| path.strip_prefix(&home).is_ok_and(|rel| dir.join(rel).exists());
    let mut add = |d: Dependency| {
        if !out.iter().any(|x| x.kind == d.kind && x.name == d.name) {
            out.push(d);
        }
    };
    for (field, kind) in ASSET_FIELDS {
        let Some(name) = values.get(*field).map(crate::field::value_str).filter(|n| safe_name(n)) else { continue };
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
    let library = values.get("decoration_library").map(crate::field::value_str).unwrap_or_default();
    let deco_theme = values.get("decoration_theme").map(crate::field::value_str).unwrap_or_default();
    if library == "org.kde.kwin.aurorae" {
        if let Some(name) = deco_theme.strip_prefix("__aurorae__svg__").filter(|n| safe_name(n)) {
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
            d.url = format!("https://aur.archlinux.org/packages?K={}", library.rsplit('.').next().unwrap_or(&library));
        }
        add(d);
    }
    // Panel widgets the user installed (system ones come with Plasma).
    if let Ok(text) = fs::read_to_string(dir.join(crate::wallpaper::APPLETSRC_REL)) {
        for plugin in text.lines().filter_map(|l| l.strip_prefix("plugin=")).map(str::trim) {
            let user = data_home().join("plasma/plasmoids").join(plugin);
            if safe_name(plugin) && user.exists() {
                add(describe("plasmoid", plugin, &[user]));
            }
        }
    }
    // Discord themes need a theming client (stock Discord has none).
    let carries_discord = crate::discord::CLIENTS.iter().any(|(_, rel)| dir.join(rel).join("themes").exists());
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
    // Login screen: SDDM's theme lives in /etc (root) — recorded with its
    // source; selecting it is a command for the user.
    if let Some(name) = sddm_current().filter(|n| safe_name(n) && n != "breeze") {
        let paths = locate_all("sddm", &name);
        add(describe("sddm", &name, &paths));
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
    for user_js in crate::tracked_apps::firefox_config_paths().iter().filter(|p| p.ends_with("/user.js")) {
        let theme_copy = crate::profile::theme_file(dir, user_js);
        let Ok(text) = fs::read_to_string(theme_copy) else { continue };
        let Some(guid) = text.lines().find_map(|l| {
            let rest = l.trim().strip_prefix("user_pref(\"extensions.activeThemeID\",")?;
            Some(rest.trim().trim_end_matches(");").trim().trim_matches('"').to_string())
        }) else { continue };
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
    for (field, kind) in [("vscode_theme", "vscode"), ("cursor_ide_theme", "cursor"), ("zed_dark", "zed"), ("zed_light", "zed")] {
        if let (Some(src), Some(name)) = (sources.get(field), values.get(field).map(crate::field::value_str)) {
            let url = match src.store.as_str() {
                "vscode-marketplace" => format!("https://marketplace.visualstudio.com/items?itemName={}", src.id),
                "open-vsx" => format!("https://open-vsx.org/extension/{}", src.id.replacen('.', "/", 1)),
                "zed" => format!("https://zed.dev/extensions?query={}", src.id),
                _ => src.url.clone(),
            };
            add(Dependency { kind: kind.into(), name, source: src.store.clone(), id: src.id.clone(), url, exact: true, ..Default::default() });
        }
    }
    out
}

/// Dependencies not installed here, with their index in `deps`.
pub fn missing(deps: &[Dependency]) -> Vec<(usize, &Dependency)> {
    deps.iter().enumerate().filter(|(_, d)| !installed(d)).collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Install
// ─────────────────────────────────────────────────────────────────────────────

/// Command the user runs for a package dependency (we never sudo).
pub fn package_command(d: &Dependency) -> Option<String> {
    let install = match d.source.as_str() {
        "package" => Some(format!("sudo pacman -S {}", d.id)),
        "aur" => Some(format!("yay -S {}", d.id)),
        _ => None,
    };
    if d.kind != "sddm" {
        return install;
    }
    // SDDM's theme is selected in /etc (root), the way the KDE settings page writes it.
    let select = format!("sudo kwriteconfig6 --file /etc/sddm.conf.d/kde_settings.conf --group Theme --key Current {}", d.name);
    let need_install = locate("sddm", &d.name).is_none();
    Some(match install.filter(|_| need_install) {
        Some(i) => format!("{i} && {select}"),
        None => select,
    })
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
/// `kind` (index.theme, cursors/, gtk-3.0/, *.kvconfig, …).
fn theme_dirs(root: &Path, kind: &str) -> Vec<PathBuf> {
    let marks = |d: &Path| -> bool {
        match kind {
            "icons" => d.join("index.theme").is_file() && !d.join("cursors").exists(),
            "cursors" => d.join("cursors").is_dir(),
            "gtk" => d.join("gtk-3.0").is_dir() || d.join("gtk-4.0").is_dir(),
            "kvantum" => fs::read_dir(d).ok().is_some_and(|mut it| it.any(|e| e.is_ok_and(|e| e.path().extension().is_some_and(|x| x == "kvconfig")))),
            "aurorae" => fs::read_dir(d).ok().is_some_and(|mut it| it.any(|e| e.is_ok_and(|e| e.file_name().to_string_lossy().starts_with("decoration.svg")))),
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
    let name = archive.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
    if name.ends_with(".zip") {
        let mut zip = zip::ZipArchive::new(fs::File::open(archive)?)?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let Some(rel) = entry.enclosed_name() else { continue };
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
        let ok = std::process::Command::new("tar")
            .arg("-xf")
            .arg(archive)
            .arg("-C")
            .arg(dest)
            .arg("--no-same-owner")
            .status()
            .with_context(|| "run tar")?
            .success();
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
                let escapes = target.is_absolute() || target.components().filter(|c| matches!(c, std::path::Component::ParentDir)).count() > p.strip_prefix(dest).map(|r| r.components().count()).unwrap_or(0).saturating_sub(1);
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
        let (Some(link), Some(name)) = (xml_tag(&xml, &format!("downloadlink{n}")), xml_tag(&xml, &format!("downloadname{n}"))) else {
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

fn install_from_store(d: &Dependency) -> Result<String> {
    let files = store_files(&d.id)?;
    let wanted = d.name.to_lowercase();
    let (link, file_name) = files
        .iter()
        .find(|(_, n)| n.to_lowercase().contains(&wanted))
        .unwrap_or(&files[0])
        .clone();
    let file_name = Path::new(&file_name).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "download".into());
    let bytes = crate::net::http_get_bytes(&link, 512 << 20)?;
    let work = tempfile::Builder::new().prefix("theme-deps-").tempdir().context("temp dir")?;
    let archive = work.path().join(&file_name);
    fs::write(&archive, &bytes)?;

    if let Some(kind) = kpackage_type(&d.kind) {
        let run = |op: &str| {
            std::process::Command::new("kpackagetool6")
                .args(["--type", kind, op])
                .arg(&archive)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        };
        if !(run("--install") || run("--upgrade")) {
            bail!("kpackagetool6 could not install {file_name}");
        }
        return Ok(format!("installed {} ({file_name}) with kpackagetool6", d.title));
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
            let dst = root.join(f.file_name().unwrap_or_default());
            if !dst.exists() {
                fs::copy(&f, &dst)?;
                copied += 1;
            }
        }
        return Ok(format!("installed {copied} colour scheme(s) from {}", d.title));
    }

    let out = work.path().join("x");
    unpack(&archive, &out)?;
    let dirs = theme_dirs(&out, &d.kind);
    if dirs.is_empty() {
        bail!("{file_name} holds no {} theme", d.kind);
    }
    let mut placed = Vec::new();
    for dir in dirs {
        let Some(name) = dir.file_name().map(|n| n.to_string_lossy().to_string()) else { continue };
        let dst = root.join(&name);
        if dst.exists() {
            continue; // never overwrite the user's own copy
        }
        crate::profile::copy_tree(&dir, &dst)?;
        placed.push(name);
    }
    Ok(format!("installed {} into {}: {}", d.title, root.display(), placed.join(", ")))
}

/// Install dependency `d` from its source. Packages are not installed (no
/// sudo): the error carries the command to run.
pub fn install(d: &Dependency) -> Result<String> {
    let result = match d.source.as_str() {
        _ if d.kind == "sddm" => bail!("SDDM themes need root — run: {} (or System Settings → Login Screen)", package_command(d).unwrap_or_default()),
        "kde-store" => install_from_store(d),
        "vscode-marketplace" | "open-vsx" => crate::stores::install(if d.kind == "cursor" { "cursor" } else { "vscode" }, &d.id).map(|r| format!("{r:?}")),
        "zed" => crate::stores::install("zed", &d.id).map(|r| format!("{r:?}")),
        "package" | "aur" => bail!("install it with: {}", package_command(d).unwrap_or_default()),
        "amo" => bail!("install it from inside Firefox: {}", d.url),
        _ => bail!("no known source — look at {}", d.url),
    }?;
    if !installed(d) {
        bail!("{result}, but \"{}\" is still not found — check {}", d.name, d.url);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_ranking_prefers_the_kde_build() {
        let kde = match_score("YAMIS", "Yet Another Monochrome Icon Set For KDE Plasma");
        let fork = match_score("YAMIS", "Yet Another Monochrome Icon Set fork for Cosmic Desktop");
        let single = match_score("YAMIS", "Antigravity IDE - YAMIS White Icon");
        assert!(kde > fork && kde > single, "{kde} {fork} {single}");
        assert_eq!(match_score("Orchis kvantum", "Orchis kvantum"), 100);
        assert_eq!(match_score("Candy", "Plasmorphism"), 0);
    }

    #[test]
    fn store_xml_tags() {
        let xml = "<data><content details=\"full\"><id>1305251</id><downloadlink1>https://x/y?a=1&amp;b=2</downloadlink1><downloadname1>candy-icons.tar.xz</downloadname1></content></data>";
        assert_eq!(xml_tag(xml, "id").as_deref(), Some("1305251"));
        assert_eq!(xml_tag(xml, "downloadname1").as_deref(), Some("candy-icons.tar.xz"));
    }

    #[test]
    fn package_commands_never_run_sudo_themselves() {
        let d = Dependency { source: "aur".into(), id: "klassy".into(), ..Default::default() };
        assert_eq!(package_command(&d).as_deref(), Some("yay -S klassy"));
        assert!(install(&d).unwrap_err().to_string().contains("yay -S klassy"));
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
        assert_eq!(theme_dirs(r, "icons"), vec![r.join("pkg/Candy")]);
        assert_eq!(theme_dirs(r, "cursors"), vec![r.join("pkg/Cur")]);
        assert_eq!(theme_dirs(r, "kvantum"), vec![r.join("Orchis")]);
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
        assert!(std::process::Command::new("tar").arg("-czf").arg(&archive).arg("-C").arg(&src).arg("Theme").status().unwrap().success());
        let out = t.path().join("out");
        unpack(&archive, &out).unwrap();
        assert!(fs::symlink_metadata(out.join("Theme/a/ok.svg")).is_ok());
        assert!(fs::symlink_metadata(out.join("Theme/a/evil")).is_err());
    }
}

#[cfg(test)]
mod sddm_tests {
    use super::*;

    #[test]
    fn sddm_dependency_carries_the_select_command() {
        let d = Dependency { kind: "sddm".into(), name: "sddm-astronaut-theme".into(), source: "aur".into(), id: "sddm-astronaut-theme".into(), ..Default::default() };
        let cmd = package_command(&d).unwrap();
        assert!(cmd.contains("--group Theme --key Current sddm-astronaut-theme"), "{cmd}");
        assert!(install(&d).unwrap_err().to_string().contains("need root"));
    }
}
