//! Theme package — what a profile *is*: a full snapshot of every file that
//! shapes the look (KDE, KWin incl. window rules, panels, GTK, Kvantum,
//! terminals, launchers, editors, browser, notes… at their `$HOME`-relative
//! paths, plus `wallpaper/`) described by `<profiles>/<name>/theme.json`.
//!
//! * `values` — the theme field values read from the snapshot (field.rs /
//!   theme_fields.json): what the GUI shows and what `check` verifies.
//! * `assets` — local assets the theme uses (generated/user colour schemes,
//!   Kvantum, Look & Feel, Plasma styles, btop and Zed theme files, the
//!   wallpaper) with origin and redistributability; system assets are
//!   references by name.
//! * `sources` — official origin of app themes (VS Code Marketplace / Open VSX
//!   id, Zed extension id) so a missing one can be installed automatically.
//!
//! Applying restores the snapshot (the same path legacy profiles always used,
//! verified against the old snapshots key by key), installs assets kept
//! outside the tracked files, reports missing references with where to get
//! them, and keeps the current wallpaper when the theme has none.
//! (2026-11-04: a values-only `theme@1` format lost terminal colours, GTK4,
//! wofi, Zen, KWin effect toggles… — never store only registry values.)

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::field::{self, Base};
use crate::palette::{self, Palette};
use crate::paths::{data_home, home_dir, live_path, profile_dir, validate_profile_name};
use crate::profile::copy_tree;
use crate::wallpaper::{plasma_wallpaper_from, APPLETSRC_REL};

pub const MANIFEST: &str = "theme.json";
pub const SCHEMA: &str = "theme-profile/theme@2";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Asset {
    /// color-scheme | kvantum | look-and-feel | desktoptheme | btop | zed | wallpaper
    pub kind: String,
    pub name: String,
    /// Path inside the package (`$HOME`-relative for snapshot files).
    pub path: String,
    /// "generated" (from a palette — ours) or "user" (third-party/local copy).
    pub origin: String,
    /// May be published (generated assets: yes; third-party copies: unknown → no).
    pub redistributable: bool,
    /// Licence of the asset (`theme-profile license`), e.g. "CC0-1.0".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub license: String,
    /// Where it comes from (URL), for attribution.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Source {
    /// "vscode-marketplace" | "open-vsx" | "zed" | "url"
    pub store: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Package {
    pub schema: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub generator: String,
    #[serde(default)]
    pub values: BTreeMap<String, Value>,
    #[serde(default)]
    pub palette: Option<Palette>,
    /// Package path of the wallpaper file (`wallpaper/<file>`); none → applying
    /// leaves the wallpaper alone.
    #[serde(default)]
    pub wallpaper: Option<String>,
    #[serde(default)]
    pub assets: Vec<Asset>,
    /// field id → where to get what it references.
    #[serde(default)]
    pub sources: BTreeMap<String, Source>,
    /// The stored files hold only the look (appearance.rs). Themes saved
    /// before 2026-11-04 evening are converted by `migrate`.
    #[serde(default)]
    pub appearance_only: bool,
    /// What the theme needs that does not travel inside it, with the source
    /// to install it from (deps.rs).
    #[serde(default)]
    pub dependencies: Vec<crate::deps::Dependency>,
}

#[derive(Debug, Clone, Default)]
pub struct SaveOptions {
    pub description: Option<String>,
    pub include_wallpaper: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ApplyOptions {
    /// Install missing app themes from their official store before applying.
    pub install_missing: bool,
    /// Leave the current wallpaper even when the theme carries one.
    pub keep_wallpaper: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Missing {
    pub field: String,
    pub name: String,
    /// Human hint: store id, URL or where to look.
    pub source: String,
}

#[derive(Debug, Default, Serialize)]
pub struct ApplyReport {
    pub applied: Vec<String>,
    /// App settings not touched because the app has no config here.
    pub skipped: Vec<String>,
    pub installed: Vec<String>,
    pub missing: Vec<Missing>,
    pub errors: Vec<String>,
    pub backup: String,
    /// Dependencies not installed here (deps.rs), to offer for installation.
    pub dependencies: Vec<Needed>,
}

/// A missing dependency with its index in `theme.json` (for `deps --install`)
/// and, for system packages, the command the user runs.
#[derive(Debug, Clone, Serialize)]
pub struct Needed {
    pub index: usize,
    #[serde(flatten)]
    pub dependency: crate::deps::Dependency,
    pub command: Option<String>,
}

/// Dependencies of theme `name` not installed on this machine.
pub fn needed(name: &str) -> Result<Vec<Needed>> {
    let package = load(&profile_dir(name)?)?;
    Ok(crate::deps::missing(&package.dependencies)
        .into_iter()
        .map(|(index, d)| Needed { index, dependency: d.clone(), command: crate::deps::package_command(d) })
        .collect())
}

/// Re-detect the dependencies of theme `name` on this machine (themes saved
/// before dependencies existed, or after installing something).
pub fn refresh_dependencies(name: &str) -> Result<Vec<crate::deps::Dependency>> {
    let dir = profile_dir(name)?;
    let mut package = load(&dir)?;
    package.dependencies = crate::deps::detect(&dir, &package.values, &package.sources);
    write_manifest(&dir, &package)?;
    Ok(package.dependencies)
}

/// Install dependencies `indexes` (None = all missing) of theme `name`;
/// `(index, result message or error)` per dependency.
pub fn install_dependencies(name: &str, indexes: Option<&[usize]>) -> Result<Vec<(usize, Result<String>)>> {
    let package = load(&profile_dir(name)?)?;
    let mut out = Vec::new();
    for (i, d) in crate::deps::missing(&package.dependencies) {
        if indexes.is_some_and(|ix| !ix.contains(&i)) {
            continue;
        }
        out.push((i, crate::deps::install(d)));
    }
    Ok(out)
}

pub fn is_package(dir: &Path) -> bool {
    dir.join(MANIFEST).is_file()
}

pub fn load(dir: &Path) -> Result<Package> {
    let text = fs::read_to_string(dir.join(MANIFEST)).with_context(|| format!("no {MANIFEST} in {}", dir.display()))?;
    let package: Package = serde_json::from_str(&text).context("invalid theme.json")?;
    if package.schema != SCHEMA {
        return Err(anyhow!("unsupported theme schema {:?}", package.schema));
    }
    Ok(package)
}

fn write_manifest(dir: &Path, package: &Package) -> Result<()> {
    fs::write(dir.join(MANIFEST), serde_json::to_string_pretty(package)? + "\n")?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Where assets live
// ─────────────────────────────────────────────────────────────────────────────

/// A field whose value names an installable asset: its user directory
/// (relative to `$HOME`, resolved through XDG for the live system) and the
/// system directories that make bundling unnecessary.
struct AssetKind {
    field: &'static str,
    kind: &'static str,
    user_rel: &'static str,
    system: &'static [&'static str],
    suffix: &'static str,
}

const BUNDLED_KINDS: &[AssetKind] = &[
    AssetKind { field: "color_scheme", kind: "color-scheme", user_rel: ".local/share/color-schemes", system: &["/usr/share/color-schemes"], suffix: ".colors" },
    AssetKind { field: "kvantum_theme", kind: "kvantum", user_rel: ".config/Kvantum", system: &["/usr/share/Kvantum"], suffix: "" },
    AssetKind { field: "look_and_feel", kind: "look-and-feel", user_rel: ".local/share/plasma/look-and-feel", system: &["/usr/share/plasma/look-and-feel"], suffix: "" },
    AssetKind { field: "plasma_theme", kind: "desktoptheme", user_rel: ".local/share/plasma/desktoptheme", system: &["/usr/share/plasma/desktoptheme"], suffix: "" },
    AssetKind { field: "btop_theme", kind: "btop", user_rel: ".config/btop/themes", system: &["/usr/share/btop/themes", "/usr/local/share/btop/themes"], suffix: ".theme" },
];

fn safe_name(name: &str) -> bool {
    !name.is_empty() && !name.contains('/') && !name.starts_with('.') && name != ".."
}

/// Directory holding Kvantum theme `name` under `root`: `<root>/<name>/` or a
/// variant inside a family directory (`Orchis/OrchisDark.kvconfig`).
fn kvantum_dir(root: &Path, name: &str) -> Option<PathBuf> {
    let direct = root.join(name);
    if direct.join(format!("{name}.kvconfig")).is_file() {
        return Some(direct);
    }
    let mut dirs: Vec<PathBuf> = fs::read_dir(root).ok()?.flatten().map(|e| e.path()).collect();
    dirs.sort();
    dirs.into_iter().find(|d| d.join(format!("{name}.kvconfig")).is_file())
}

/// Where asset `name` of kind `k` lives under `root` (file or directory).
fn asset_in(k: &AssetKind, root: &Path, name: &str) -> Option<PathBuf> {
    if k.kind == "kvantum" {
        return kvantum_dir(root, name);
    }
    let path = root.join(format!("{name}{}", k.suffix));
    path.exists().then_some(path)
}

fn kind_available(k: &AssetKind, name: &str) -> bool {
    if !safe_name(name) {
        return false;
    }
    if k.kind == "kvantum" {
        return std::iter::once(live_path(k.user_rel))
            .chain(k.system.iter().map(PathBuf::from))
            .any(|root| kvantum_dir(&root, name).is_some());
    }
    let file = format!("{name}{}", k.suffix);
    std::iter::once(live_path(k.user_rel))
        .chain(k.system.iter().map(PathBuf::from))
        .any(|root| root.join(&file).exists())
}

/// Is the asset a field value refers to installed? Unknown kinds → true.
pub fn available(field_id: &str, value: &Value) -> bool {
    let name = field::value_str(value);
    if name.is_empty() {
        return true;
    }
    match field_id {
        "plasma_theme" if name == "default" => true,
        "btop_theme" if name.eq_ignore_ascii_case("Default") => true,
        "icon_theme" => [data_home().join("icons"), home_dir().join(".icons"), PathBuf::from("/usr/share/icons")]
                .iter()
                .any(|r| r.join(&name).join("index.theme").is_file()),
        "gtk_theme" => [data_home().join("themes"), home_dir().join(".themes"), PathBuf::from("/usr/share/themes")]
            .iter()
            .any(|r| r.join(&name).is_dir()),
        "zed_dark" | "zed_light" => zed_theme_file(&name, &Base::Live).is_some() || ZED_BUILTIN.contains(&name.as_str()),
        "vscode_theme" => vscode_theme_extension("vscode", &name).is_some() || VSCODE_BUILTIN.contains(&name.as_str()),
        "cursor_ide_theme" => vscode_theme_extension("cursor", &name).is_some()
            || name.starts_with("Cursor ")
            || VSCODE_BUILTIN.contains(&name.as_str()),
        other => BUNDLED_KINDS
            .iter()
            .find(|k| k.field == other)
            .is_none_or(|k| kind_available(k, &name)),
    }
}

const ZED_BUILTIN: &[&str] = &[
    "Zed (Default)", "One Dark", "One Light", "Ayu Dark", "Ayu Light", "Ayu Mirage",
    "Gruvbox Dark", "Gruvbox Dark Hard", "Gruvbox Dark Soft",
    "Gruvbox Light", "Gruvbox Light Hard", "Gruvbox Light Soft",
];
const VSCODE_BUILTIN: &[&str] = &[
    "Default Dark Modern", "Default Light Modern", "Default Dark+", "Default Light+",
    "Visual Studio Dark", "Visual Studio Light", "Abyss", "Kimbie Dark", "Monokai",
    "Solarized Dark", "Solarized Light", "Tomorrow Night Blue", "Red", "Quiet Light",
];

/// The live Zed theme family file declaring `theme_name`.
pub fn zed_theme_file_pub(theme_name: &str) -> Option<PathBuf> {
    zed_theme_file(theme_name, &Base::Live)
}

/// Zed theme family file (user themes, live extensions) declaring `theme_name`.
fn zed_theme_file(theme_name: &str, base: &Base) -> Option<PathBuf> {
    let mut roots = vec![base.resolve(".config/zed/themes")];
    if matches!(base, Base::Live) {
        if let Ok(exts) = fs::read_dir(data_home().join("zed/extensions/installed")) {
            roots.extend(exts.flatten().map(|e| e.path().join("themes")));
        }
    }
    for root in roots {
        let Ok(entries) = fs::read_dir(&root) else { continue };
        let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
        files.sort();
        for file in files {
            let Ok(text) = fs::read_to_string(&file) else { continue };
            let Ok(json) = serde_json::from_str::<Value>(&text) else { continue };
            let declares = json
                .get("themes")
                .and_then(Value::as_array)
                .is_some_and(|t| t.iter().any(|x| x.get("name").and_then(Value::as_str) == Some(theme_name)));
            if declares {
                return Some(file);
            }
        }
    }
    None
}

/// `publisher.name` of the installed extension contributing a VS Code/Cursor
/// theme (matched by theme `id` or `label`).
fn vscode_theme_extension(app: &str, theme: &str) -> Option<String> {
    let home = home_dir();
    let roots: Vec<PathBuf> = if app == "cursor" {
        vec![home.join(".cursor/extensions")]
    } else {
        vec![home.join(".vscode/extensions"), home.join(".vscode-oss/extensions")]
    };
    for root in roots {
        let Ok(entries) = fs::read_dir(&root) else { continue };
        for entry in entries.flatten() {
            let Ok(text) = fs::read_to_string(entry.path().join("package.json")) else { continue };
            let Ok(pkg) = serde_json::from_str::<Value>(&text) else { continue };
            let provides = pkg
                .get("contributes")
                .and_then(|c| c.get("themes"))
                .and_then(Value::as_array)
                .is_some_and(|t| {
                    t.iter().any(|x| {
                        x.get("id").and_then(Value::as_str) == Some(theme)
                            || x.get("label").and_then(Value::as_str) == Some(theme)
                    })
                });
            if provides {
                let publisher = pkg.get("publisher").and_then(Value::as_str)?;
                let name = pkg.get("name").and_then(Value::as_str)?;
                return Some(format!("{publisher}.{name}"));
            }
        }
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// Save
// ─────────────────────────────────────────────────────────────────────────────

fn copy_into(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    copy_tree(src, dst)
}

/// Describe the snapshot in `dir` (already holding every tracked file at its
/// `$HOME`-relative path) with a `theme.json`. Local assets a value names but
/// the snapshot lacks (e.g. a user Plasma style) are copied in at their
/// `$HOME`-relative path, so apply can put them back.
pub fn build(name: &str, dir: &Path, options: &SaveOptions) -> Result<Package> {
    let snapshot = Base::Snapshot(dir.to_path_buf());
    let values: BTreeMap<String, Value> = field::capture(&snapshot).into_iter().collect();
    let mut package = Package {
        schema: SCHEMA.into(),
        name: name.into(),
        description: options.description.clone().unwrap_or_default(),
        created: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        generator: format!("theme-profile {}", env!("CARGO_PKG_VERSION")),
        ..Default::default()
    };

    let home = home_dir();
    for k in BUNDLED_KINDS {
        let Some(value) = values.get(k.field) else { continue };
        let asset = field::value_str(value);
        if !safe_name(&asset) {
            continue;
        }
        let in_snapshot = asset_in(k, &dir.join(k.user_rel), &asset);
        let path = match in_snapshot {
            Some(p) => p,
            None => {
                // Not tracked: copy the live user asset in (system ones stay references).
                let Some(live) = asset_in(k, &live_path(k.user_rel), &asset) else { continue };
                let rel = live.strip_prefix(&home).map(Path::to_path_buf).unwrap_or_else(|_| Path::new(k.user_rel).join(live.file_name().unwrap_or_default()));
                copy_into(&live, &dir.join(&rel))?;
                dir.join(rel)
            }
        };
        let rel = path.strip_prefix(dir).unwrap_or(&path).to_string_lossy().to_string();
        let generated = asset.starts_with("ThemeProfile");
        // Generated from the palette: ours (colour scheme: MIT; Kvantum: a
        // recolour of Kvantum's GPL KvArc, so GPL).
        let license = match (generated, k.kind) {
            (true, "kvantum") => "GPL-3.0-or-later (recoloured from Kvantum's KvArc)",
            (true, _) => "MIT (generated by theme-profile)",
            _ => "",
        };
        package.assets.push(Asset {
            kind: k.kind.into(),
            name: asset,
            path: rel,
            origin: if generated { "generated" } else { "user" }.into(),
            redistributable: generated,
            license: license.into(),
            source: String::new(),
        });
    }

    for id in ["zed_dark", "zed_light"] {
        let Some(theme) = values.get(id).map(field::value_str) else { continue };
        if let Some(file) = zed_theme_file(&theme, &snapshot).or_else(|| zed_theme_file(&theme, &Base::Live)) {
            let file_name = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if let Ok(rel) = file.strip_prefix(dir) {
                package.assets.push(Asset { kind: "zed".into(), name: theme.clone(), path: rel.to_string_lossy().into(), origin: "user".into(), redistributable: false, license: String::new(), source: String::new() });
            }
            if let Some(ext) = file_name.split_once("--").map(|(e, _)| e.to_string()) {
                package.sources.insert(id.into(), Source { store: "zed".into(), id: ext, url: String::new() });
            }
        }
    }
    for (id, app, store) in [("vscode_theme", "vscode", "vscode-marketplace"), ("cursor_ide_theme", "cursor", "open-vsx")] {
        if let Some(theme) = values.get(id).map(field::value_str) {
            if let Some(ext) = vscode_theme_extension(app, &theme) {
                package.sources.insert(id.into(), Source { store: store.into(), id: ext, url: String::new() });
            }
        }
    }
    share_editor_sources(&values, &mut package.sources);

    // Konsole colour schemes: the profile may name one the receiver lacks.
    if let Ok(entries) = fs::read_dir(dir.join(".local/share/konsole")) {
        let mut schemes: Vec<String> = entries
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".colorscheme"))
            .collect();
        schemes.sort();
        for file in schemes {
            let name = file.trim_end_matches(".colorscheme").to_string();
            let generated = name.starts_with("ThemeProfile");
            package.assets.push(Asset {
                kind: "konsole-scheme".into(),
                name,
                path: format!(".local/share/konsole/{file}"),
                origin: if generated { "generated" } else { "user" }.into(),
                redistributable: generated,
                license: if generated { "MIT (generated by theme-profile)".into() } else { String::new() },
                source: String::new(),
            });
        }
    }

    // fastfetch logo kept next to its config.
    if let Ok(config) = fs::read_to_string(dir.join(".config/fastfetch/config.jsonc")) {
        if let Some(source) = field::json_get(&config, &["logo".to_string(), "source".to_string()]).map(|v| field::value_str(&v)) {
            if let Some(rest) = source.strip_prefix("~/").or_else(|| source.strip_prefix("$HOME/")) {
                if rest.starts_with(".config/fastfetch/") && !rest.contains("..") && dir.join(rest).is_file() {
                    package.assets.push(Asset { kind: "fastfetch-logo".into(), name: rest.rsplit('/').next().unwrap_or(rest).into(), path: rest.into(), origin: "user".into(), redistributable: false, license: String::new(), source: String::new() });
                }
            }
        }
    }

    if let Ok(p) = palette::load(&snapshot.resolve(palette::LIVE_PALETTE_REL)) {
        if values.get("color_scheme").map(field::value_str).as_deref() == Some(palette::kde_scheme_id(&p).as_str()) {
            package.palette = Some(p);
        }
    }

    // Wallpaper: `<theme>/wallpaper/` (copy + source) written by the snapshot
    // save; without it applying leaves the wallpaper alone.
    let wallpaper_dir = dir.join(WALLPAPER_DIR);
    if options.include_wallpaper {
        if let Some(file) = fs::read_dir(&wallpaper_dir).ok().and_then(|it| {
            it.flatten().map(|e| e.path()).find(|p| p.is_file() && p.file_name().is_some_and(|n| n != "source"))
        }) {
            let file_name = file.file_name().unwrap_or_default().to_string_lossy().to_string();
            let rel = format!("{WALLPAPER_DIR}/{file_name}");
            package.wallpaper = Some(rel.clone());
            package.assets.push(Asset { kind: "wallpaper".into(), name: file_name, path: rel, origin: "user".into(), redistributable: false, license: String::new(), source: String::new() });
        }
    } else if wallpaper_dir.exists() {
        fs::remove_dir_all(&wallpaper_dir)?;
    }

    package.dependencies = crate::deps::detect(dir, &values, &package.sources);
    package.values = values;
    package.appearance_only = true;
    write_manifest(dir, &package)?;
    Ok(package)
}

/// Keep only the look in a stored theme (appearance.rs): mixed files are
/// filtered, kitty.conf / alacritty.toml become their include files. The
/// theme is copied to `backups/pre-normalize-<name>-…` first.
fn normalize_dir(name: &str, dir: &Path) -> Result<()> {
    let backups = crate::paths::backups_dir();
    fs::create_dir_all(&backups)?;
    let backup = crate::profile::unique_dir(&backups, &format!("pre-normalize-{name}"))?;
    copy_tree(dir, &backup).with_context(|| format!("back up {} to {}", dir.display(), backup.display()))?;
    for rel in crate::profile::resolve_cli_paths(None)? {
        let path = dir.join(&rel);
        if !path.is_file() || matches!(crate::appearance::policy(&rel), crate::appearance::Policy::Whole) {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else { continue };
        let has_include = crate::appearance::include_of(&rel).is_some_and(|inc| dir.join(inc).is_file());
        let entries = crate::appearance::normalize(&rel, &text, has_include)?;
        fs::remove_file(&path)?;
        for (k, t) in entries {
            let dst = dir.join(k);
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(dst, t)?;
        }
    }
    Ok(())
}

/// Save the live look as theme `name`: a full snapshot of every tracked file
/// (KDE, KWin incl. window rules, panels, GTK, Kvantum, terminals, launchers,
/// editors, browser, notes…) plus `theme.json`.
pub fn save(name: &str, options: &SaveOptions) -> Result<Package> {
    validate_profile_name(name)?;
    let target = profile_dir(name)?;
    let mut options = options.clone();
    if options.description.is_none() && target.is_dir() {
        options.description = Some(crate::profile::get_profile_description(name).unwrap_or_default());
    }
    let previous = load(&target).ok();
    crate::profile::save_profile_cli(name, None)?;
    let mut package = build(name, &target, &options)?;
    if let Some(prev) = previous {
        carry_licences(&prev, &mut package);
        write_manifest(&target, &package)?;
    }
    Ok(package)
}

/// Keep the author's licence marks (`theme-profile license`) across re-saves.
fn carry_licences(from: &Package, to: &mut Package) {
    for a in to.assets.iter_mut() {
        if let Some(old) = from.assets.iter().find(|o| o.kind == a.kind && o.name == a.name && !o.license.is_empty()) {
            a.license = old.license.clone();
            a.source = old.source.clone();
            a.redistributable = old.redistributable;
        }
    }
}

/// Mark asset `asset` (its name, e.g. a wallpaper file or a Kvantum theme) of
/// theme `name` with a licence and source; `redistributable` decides whether
/// `export --public` may ship it.
pub fn set_licence(name: &str, asset: &str, licence: &str, source: &str, redistributable: bool) -> Result<Asset> {
    let dir = profile_dir(name)?;
    let mut package = load(&dir)?;
    // `kind:name` when two assets share a name (Look & Feel and btop "paper").
    let (kind, asset_name) = match asset.split_once(':') {
        Some((k, n)) if package.assets.iter().any(|a| a.kind == k) => (Some(k), n),
        _ => (None, asset),
    };
    let a = package
        .assets
        .iter_mut()
        .find(|a| a.name == asset_name && kind.is_none_or(|k| a.kind == k))
        .ok_or_else(|| anyhow!("{name} has no asset named {asset:?}"))?;
    a.license = licence.into();
    a.source = source.into();
    a.redistributable = redistributable;
    let out = a.clone();
    write_manifest(&dir, &package)?;
    Ok(out)
}

/// Drop from stored theme `name` everything that is not part of it: files no
/// longer tracked (QtProject.conf, Obsidian vault list…), the `metadata/`
/// diagnostics, other themes' assets from library folders. `theme.json` is
/// rebuilt first so its asset list names only what the theme uses. The whole
/// theme is copied to `backups/pre-prune-<name>-…` before anything is removed.
/// Returns the number of files removed.
pub fn prune(name: &str) -> Result<usize> {
    validate_profile_name(name)?;
    let dir = profile_dir(name)?;
    let old = load(&dir)?;
    let backups = crate::paths::backups_dir();
    fs::create_dir_all(&backups)?;
    let backup = crate::profile::unique_dir(&backups, &format!("pre-prune-{name}"))?;
    copy_tree(&dir, &backup).with_context(|| format!("back up {}", dir.display()))?;
    let options = SaveOptions { description: Some(old.description.clone()), include_wallpaper: old.wallpaper.is_some() };
    let mut package = build(name, &dir, &options)?;
    package.created = old.created.clone();
    carry_licences(&old, &mut package);
    write_manifest(&dir, &package)?;
    let keep: std::collections::HashSet<PathBuf> = crate::share::theme_files(&dir, true)?.into_iter().collect();
    let mut removed = 0;
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let path = dir.join(&rel);
        let Ok(entries) = fs::read_dir(&path) else { continue };
        for e in entries.flatten() {
            let child = rel.join(e.file_name());
            let meta = fs::symlink_metadata(e.path())?;
            if meta.is_dir() {
                stack.push(child);
            } else if child != Path::new(MANIFEST) && !keep.contains(&child) {
                fs::remove_file(e.path())?;
                removed += 1;
            }
        }
    }
    // Empty folders left behind.
    fn drop_empty(p: &Path) {
        if let Ok(entries) = fs::read_dir(p) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    drop_empty(&e.path());
                    let _ = fs::remove_dir(e.path());
                }
            }
        }
    }
    drop_empty(&dir);
    Ok(removed)
}

/// Add `theme.json` to a legacy snapshot profile. The snapshot itself is not
/// changed (lossless). Packages of the old values-only format (`theme@1`)
/// cannot be upgraded — re-save them from the live session.
pub fn migrate(name: &str) -> Result<Package> {
    validate_profile_name(name)?;
    let target = profile_dir(name)?;
    if !target.is_dir() {
        return Err(anyhow!("Profile not found: {name}"));
    }
    if target.join(MANIFEST).is_file() {
        let mut package = load(&target).map_err(|e| anyhow!("{name}: {e:#} (values-only theme@1 packages must be re-saved)"))?;
        if !package.appearance_only {
            normalize_dir(name, &target)?;
            package.appearance_only = true;
            write_manifest(&target, &package)?;
        }
        return Ok(package);
    }
    normalize_dir(name, &target).context("keep only the look")?;
    let options = SaveOptions {
        description: Some(crate::profile::get_profile_description(name).unwrap_or_default()),
        include_wallpaper: true,
    };
    build(name, &target, &options)
}

/// Point a copied package at its new profile name (`clone`).
pub fn rename_manifest(dir: &Path, name: &str) -> Result<()> {
    let mut package = load(dir)?;
    package.name = name.into();
    write_manifest(dir, &package)
}

pub fn set_description(name: &str, description: &str) -> Result<()> {
    let dir = profile_dir(name)?;
    let mut package = load(&dir)?;
    package.description = description.into();
    write_manifest(&dir, &package)
}

// ─────────────────────────────────────────────────────────────────────────────
// Apply
// ─────────────────────────────────────────────────────────────────────────────

const WALLPAPER_DIR: &str = "wallpaper";

fn source_hint(package: &Package, field_id: &str, name: &str) -> String {
    if let Some(s) = package.sources.get(field_id) {
        return match s.store.as_str() {
            "vscode-marketplace" => format!("https://marketplace.visualstudio.com/items?itemName={}", s.id),
            "open-vsx" => format!("https://open-vsx.org/extension/{}", s.id.replacen('.', "/", 1)),
            "zed" => format!("Zed extension \"{}\" (zed.dev/extensions)", s.id),
            _ => s.url.clone(),
        };
    }
    match field_id {
        "icon_theme" | "gtk_theme" | "kvantum_theme" | "look_and_feel" | "plasma_theme" | "color_scheme" => {
            format!("https://store.kde.org/search?search={}", name.replace(' ', "+"))
        }
        _ => String::new(),
    }
}

fn store_app(field_id: &str, store: &str) -> Option<&'static str> {
    match (field_id, store) {
        ("zed_dark" | "zed_light", "zed") => Some("zed"),
        ("vscode_theme", _) => Some("vscode"),
        ("cursor_ide_theme", _) => Some("cursor"),
        _ => None,
    }
}

/// Apply theme `name`: the snapshot restore (pre-apply backup, plasmashell
/// stopped, every file put back, colour scheme / widget style / KWin effects /
/// kitty re-applied, plasmashell restarted) plus the package extras: assets
/// outside the tracked files, store installs, the missing-parts report and the
/// wallpaper rule (no wallpaper in the theme, or `keep_wallpaper` → the
/// current wallpaper stays).
pub fn apply(name: &str, options: &ApplyOptions) -> Result<ApplyReport> {
    validate_profile_name(name)?;
    let dir = profile_dir(name)?;
    let package = load(&dir)?;
    let mut report = ApplyReport::default();

    if options.install_missing {
        for (id, value) in &package.values {
            if available(id, value) {
                continue;
            }
            let Some(src) = package.sources.get(id) else { continue };
            let Some(app) = store_app(id, &src.store) else { continue };
            match crate::stores::install(app, &src.id) {
                Ok(_) => report.installed.push(format!("{app} {}", src.id)),
                Err(e) => report.errors.push(format!("{id}: install {}: {e:#}", src.id)),
            }
        }
    }
    report.missing = check(name)?;
    report.dependencies = needed(name)?;

    // The theme's own assets (colour scheme, Kvantum family, Look & Feel,
    // Plasma style, btop/Zed/Konsole themes, fastfetch logo) are restored with
    // the tracked files — backed up first, like everything else.
    let extras: Vec<String> = package
        .assets
        .iter()
        .filter(|a| a.kind != "wallpaper" && dir.join(&a.path).exists())
        .map(|a| a.path.clone())
        .collect();

    let keep = (package.wallpaper.is_none() || options.keep_wallpaper)
        .then(|| plasma_wallpaper_from(&live_path(APPLETSRC_REL)))
        .flatten()
        .map(PathBuf::from)
        .filter(|p| p.is_file());

    let backup = crate::profile::apply_profile_with(name, &extras)?;
    if dir.join(crate::spicetify::CONFIG).is_file() {
        crate::spicetify::refresh();
    }
    if dir.join(crate::extras::CAVA).is_file() {
        crate::extras::reload_cava();
    }
    if dir.join(".config/bat/themes").is_dir() && std::env::var("KTM_THEME_NO_APPLY").as_deref() != Ok("1") && which::which("bat").is_ok() {
        let _ = std::process::Command::new("bat").args(["cache", "--build"]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    }
    report.backup = backup.display().to_string();
    report.applied = crate::profile::get_profile_components(name).unwrap_or_default();

    if let Some(path) = keep {
        if plasma_wallpaper_from(&live_path(APPLETSRC_REL)).as_deref() != Some(path.to_string_lossy().as_ref()) {
            crate::profile::apply_wallpaper_live(&path);
        }
    }
    Ok(report)
}

/// What applying `name` would miss (no changes made).
pub fn check(name: &str) -> Result<Vec<Missing>> {
    validate_profile_name(name)?;
    let dir = profile_dir(name)?;
    let package = load(&dir)?;
    let bundled: Vec<(&str, &str)> = package.assets.iter().map(|a| (a.kind.as_str(), a.name.as_str())).collect();
    Ok(package
        .values
        .iter()
        .filter(|(id, value)| {
            let name = field::value_str(value);
            let in_package = BUNDLED_KINDS.iter().any(|k| k.field == id.as_str() && bundled.contains(&(k.kind, name.as_str())))
                || (id.starts_with("zed_") && bundled.iter().any(|(k, n)| *k == "zed" && *n == name));
            !in_package && !available(id, value)
        })
        .map(|(id, value)| {
            let name = field::value_str(value);
            Missing { field: id.clone(), source: source_hint(&package, id, &name), name }
        })
        .collect())
}

/// VS Code and Cursor share the extension ecosystem: when one editor is not
/// installed here, the same theme name in the other one tells which extension
/// provides it (the id is the same in both galleries).
fn share_editor_sources(values: &BTreeMap<String, Value>, sources: &mut BTreeMap<String, Source>) {
    for (id, other, store) in [("vscode_theme", "cursor_ide_theme", "vscode-marketplace"), ("cursor_ide_theme", "vscode_theme", "open-vsx")] {
        let same = values.get(id).map(field::value_str).is_some_and(|t| values.get(other).map(field::value_str) == Some(t));
        if same && !sources.contains_key(id) {
            if let Some(ext) = sources.get(other).map(|s| s.id.clone()) {
                sources.insert(id.into(), Source { store: store.into(), id: ext, url: String::new() });
            }
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn editors_share_extension_sources() {
        let mut values = BTreeMap::new();
        values.insert("vscode_theme".to_string(), Value::from("Night Owl"));
        values.insert("cursor_ide_theme".to_string(), Value::from("Night Owl"));
        let mut sources = BTreeMap::new();
        sources.insert("cursor_ide_theme".to_string(), Source { store: "open-vsx".into(), id: "sdras.night-owl".into(), url: String::new() });
        share_editor_sources(&values, &mut sources);
        assert_eq!(sources["vscode_theme"].store, "vscode-marketplace");
        assert_eq!(sources["vscode_theme"].id, "sdras.night-owl");
        values.insert("vscode_theme".to_string(), Value::from("Other"));
        let mut sources2 = BTreeMap::new();
        sources2.insert("cursor_ide_theme".to_string(), sources["cursor_ide_theme"].clone());
        share_editor_sources(&values, &mut sources2);
        assert!(!sources2.contains_key("vscode_theme"), "a different theme name says nothing");
    }
    use super::*;

    #[test]
    fn package_json_roundtrip() {
        let mut p = Package { schema: SCHEMA.into(), name: "x".into(), ..Default::default() };
        p.values.insert("color_scheme".into(), Value::from("Nord"));
        p.sources.insert("vscode_theme".into(), Source { store: "vscode-marketplace".into(), id: "sainnhe.everforest".into(), url: String::new() });
        let tmp = tempfile::tempdir().unwrap();
        write_manifest(tmp.path(), &p).unwrap();
        assert_eq!(load(tmp.path()).unwrap(), p);
        fs::write(tmp.path().join(MANIFEST), r#"{"schema":"other","name":"x"}"#).unwrap();
        assert!(load(tmp.path()).is_err());
    }

    #[test]
    fn source_hints() {
        let mut p = Package::default();
        p.sources.insert("vscode_theme".into(), Source { store: "vscode-marketplace".into(), id: "sainnhe.everforest".into(), url: String::new() });
        p.sources.insert("cursor_ide_theme".into(), Source { store: "open-vsx".into(), id: "sainnhe.everforest".into(), url: String::new() });
        assert!(source_hint(&p, "vscode_theme", "x").ends_with("itemName=sainnhe.everforest"));
        assert!(source_hint(&p, "cursor_ide_theme", "x").ends_with("open-vsx.org/extension/sainnhe/everforest"));
        assert!(source_hint(&p, "icon_theme", "YAMIS Dark").contains("search=YAMIS+Dark"));
        assert_eq!(store_app("zed_dark", "zed"), Some("zed"));
        assert_eq!(store_app("icon_theme", "url"), None);
    }

    #[test]
    fn unsafe_asset_names_are_never_available() {
        let k = &BUNDLED_KINDS[0];
        assert!(!kind_available(k, "../x"));
        assert!(!safe_name(".."));
        assert!(!safe_name("a/b"));
    }

    #[test]
    fn kvantum_variants_inside_family_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("Orchis")).unwrap();
        fs::write(root.join("Orchis/OrchisDark.kvconfig"), "").unwrap();
        fs::create_dir_all(root.join("Sandstone")).unwrap();
        fs::write(root.join("Sandstone/Sandstone.kvconfig"), "").unwrap();
        assert_eq!(kvantum_dir(root, "OrchisDark"), Some(root.join("Orchis")));
        assert_eq!(kvantum_dir(root, "Sandstone"), Some(root.join("Sandstone")));
        assert_eq!(kvantum_dir(root, "Nope"), None);
    }
}
