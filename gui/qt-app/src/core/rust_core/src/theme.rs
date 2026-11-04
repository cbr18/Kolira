//! Theme package — what a profile *is*: `<profiles>/<name>/theme.json` plus
//! `assets/`.
//!
//! * `values` — theme field values (field.rs / theme_fields.json): scheme,
//!   accent, styles, blur, terminal opacity, app themes… Only theme keys:
//!   personal settings (fonts and key bindings in kitty, Zed settings, panels,
//!   window rules) are never part of a theme.
//! * `assets/` — only what the receiver may not have: user / generated colour
//!   schemes, Kvantum, Look & Feel and Plasma styles, btop and Zed theme
//!   files, the wallpaper. System assets are referenced by name; icon and GTK
//!   themes are references too (often tens of MB).
//! * `sources` — official origin of app themes (VS Code Marketplace / Open VSX
//!   id, Zed extension id) so a missing one can be installed automatically.
//!
//! Applying changes only what the package lists ("absent = untouched"),
//! installs bundled assets without overwriting, skips fields whose asset is
//! missing (reported with where to get it) and backs up touched files first.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::field::{self, Base};
use crate::palette::{self, Palette};
use crate::paths::{backups_dir, config_home, data_home, home_dir, live_path, profile_dir, validate_profile_name};
use crate::profile::{copy_tree, reject_case_duplicate, unique_dir, write_active};
use crate::wallpaper::{plasma_wallpaper_from, wallpaper_dir, APPLETSRC_REL};

pub const MANIFEST: &str = "theme.json";
pub const SCHEMA: &str = "theme-profile/theme@1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Asset {
    /// color-scheme | kvantum | look-and-feel | desktoptheme | btop | zed | wallpaper
    pub kind: String,
    pub name: String,
    /// Path inside the package.
    pub path: String,
    /// "generated" (from a palette — ours) or "user" (third-party/local copy).
    pub origin: String,
    /// May be published (generated assets: yes; third-party copies: unknown → no).
    pub redistributable: bool,
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
    /// Package path of the wallpaper file (`assets/wallpaper/<file>`).
    #[serde(default)]
    pub wallpaper: Option<String>,
    #[serde(default)]
    pub assets: Vec<Asset>,
    /// field id → where to get what it references.
    #[serde(default)]
    pub sources: BTreeMap<String, Source>,
    /// Whole config files of user-defined components (`theme-profile
    /// components add`): the user chose these files explicitly, so they are
    /// stored and restored whole (`assets/files/<home-relative path>`).
    #[serde(default)]
    pub files: Vec<String>,
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

fn kind_dir(kind: &str) -> Option<PathBuf> {
    BUNDLED_KINDS.iter().find(|k| k.kind == kind).map(|k| live_path(k.user_rel))
}

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

/// Build a package into `dir` from the live system (`Base::Live`) or from a
/// legacy snapshot directory (`Base::Snapshot`, used by [`migrate`]).
pub fn build(name: &str, dir: &Path, options: &SaveOptions, base: &Base) -> Result<Package> {
    let values: BTreeMap<String, Value> = field::capture(base).into_iter().collect();
    let mut package = Package {
        schema: SCHEMA.into(),
        name: name.into(),
        description: options.description.clone().unwrap_or_default(),
        created: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        generator: format!("theme-profile {}", env!("CARGO_PKG_VERSION")),
        ..Default::default()
    };
    fs::create_dir_all(dir)?;

    // Local assets the receiver may not have.
    for k in BUNDLED_KINDS {
        let Some(value) = values.get(k.field) else { continue };
        let asset = field::value_str(value);
        if !safe_name(&asset) {
            continue;
        }
        // A legacy snapshot may lack the asset: take it from this machine.
        let src = asset_in(k, &base.resolve(k.user_rel), &asset)
            .or_else(|| asset_in(k, &live_path(k.user_rel), &asset));
        let Some(src) = src else {
            continue; // system asset (or not installed) → reference only
        };
        let file_name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let rel = format!("assets/{}/{file_name}", k.kind);
        if dir.join(&rel).exists() {
            continue;
        }
        copy_into(&src, &dir.join(&rel))?;
        let generated = asset.starts_with("ThemeProfile");
        package.assets.push(Asset {
            kind: k.kind.into(),
            name: asset,
            path: rel,
            origin: if generated { "generated" } else { "user" }.into(),
            redistributable: generated && k.kind == "color-scheme",
        });
    }

    // Zed theme files (user/store themes) and VS Code/Cursor extension ids.
    for id in ["zed_dark", "zed_light"] {
        let Some(theme) = values.get(id).map(field::value_str) else { continue };
        if let Some(file) = zed_theme_file(&theme, base) {
            let file_name = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let rel = format!("assets/zed/{file_name}");
            if !dir.join(&rel).exists() {
                copy_into(&file, &dir.join(&rel))?;
                package.assets.push(Asset { kind: "zed".into(), name: theme.clone(), path: rel, origin: "user".into(), redistributable: false });
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

    // Palette, when the colour scheme is the one generated from it.
    if let Ok(p) = palette::load(&base.resolve(palette::LIVE_PALETTE_REL)) {
        if values.get("color_scheme").map(field::value_str).as_deref() == Some(palette::kde_scheme_id(&p).as_str()) {
            package.palette = Some(p);
        }
    }

    // Wallpaper (opt-out): the one Plasma shows, or a legacy profile's copy.
    if options.include_wallpaper {
        let current = match base {
            Base::Live => plasma_wallpaper_from(&live_path(APPLETSRC_REL)).map(PathBuf::from),
            Base::Snapshot(root) => crate::profile::profile_wallpaper(root),
        };
        if let Some(src) = current {
            if let Some(file_name) = src.file_name().filter(|_| src.is_file()) {
                let rel = format!("assets/wallpaper/{}", file_name.to_string_lossy());
                copy_into(&src, &dir.join(&rel))?;
                package.wallpaper = Some(rel.clone());
                package.assets.push(Asset {
                    kind: "wallpaper".into(),
                    name: file_name.to_string_lossy().into(),
                    path: rel,
                    origin: "user".into(),
                    redistributable: false,
                });
            }
        }
    }

    // User-defined components: whole files, by explicit choice.
    if let Ok(custom) = crate::custom_components::load_custom_components() {
        for rel in custom.iter().flat_map(|c| c.config_paths.iter()) {
            if rel.contains("..") || rel.starts_with('/') {
                continue;
            }
            let src = base.resolve(rel);
            if src.exists() && !package.files.contains(rel) {
                copy_into(&src, &dir.join("assets/files").join(rel))?;
                package.files.push(rel.clone());
            }
        }
    }

    package.values = values;
    write_manifest(dir, &package)?;
    Ok(package)
}

/// Save the live theme as profile `name` (atomic swap, old one backed up).
pub fn save(name: &str, options: &SaveOptions) -> Result<Package> {
    validate_profile_name(name)?;
    reject_case_duplicate(name)?;
    let target = profile_dir(name)?;
    let profiles = target.parent().ok_or_else(|| anyhow!("bad profiles dir"))?.to_path_buf();
    fs::create_dir_all(&profiles)?;
    let tmp = unique_dir(&profiles, &format!(".save-{name}"))?;
    let mut options = options.clone();
    if options.description.is_none() && target.is_dir() {
        options.description = load(&target).ok().map(|p| p.description);
    }
    let package = match build(name, &tmp, &options, &Base::Live) {
        Ok(p) => p,
        Err(e) => {
            let _ = fs::remove_dir_all(&tmp);
            return Err(e);
        }
    };
    if target.exists() {
        let backups = backups_dir();
        fs::create_dir_all(&backups)?;
        let old = unique_dir(&backups, &format!("pre-save-{name}"))?;
        fs::rename(&target, &old)?;
    }
    fs::rename(&tmp, &target)?;
    write_active(name)?;
    Ok(package)
}

/// Convert a legacy snapshot profile into a package in place: values and
/// assets are read from the snapshot (the same readers as the inspector's
/// profile view); the old snapshot goes to `backups/pre-migrate-<name>-…`.
pub fn migrate(name: &str) -> Result<Package> {
    validate_profile_name(name)?;
    let target = profile_dir(name)?;
    if !target.is_dir() {
        return Err(anyhow!("Profile not found: {name}"));
    }
    if is_package(&target) {
        return load(&target);
    }
    let profiles = target.parent().ok_or_else(|| anyhow!("bad profiles dir"))?.to_path_buf();
    let tmp = unique_dir(&profiles, &format!(".migrate-{name}"))?;
    let options = SaveOptions {
        description: Some(crate::profile::get_profile_description(name).unwrap_or_default()),
        include_wallpaper: true,
    };
    let package = match build(name, &tmp, &options, &Base::Snapshot(target.clone())) {
        Ok(p) => p,
        Err(e) => {
            let _ = fs::remove_dir_all(&tmp);
            return Err(e);
        }
    };
    let backups = backups_dir();
    fs::create_dir_all(&backups)?;
    let old = unique_dir(&backups, &format!("pre-migrate-{name}"))?;
    fs::rename(&target, &old)?;
    fs::rename(&tmp, &target)?;
    Ok(package)
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

fn install_asset(dir: &Path, asset: &Asset) -> Result<bool> {
    let src = dir.join(&asset.path);
    if !src.exists() || !safe_name(&asset.name) {
        return Ok(false);
    }
    let dst = match asset.kind.as_str() {
        "zed" => config_home().join("zed/themes").join(src.file_name().unwrap_or_default()),
        // Same file/dir name as inside the package (Kvantum: the family dir).
        kind => match kind_dir(kind) {
            Some(root) => root.join(src.file_name().unwrap_or_default()),
            None => return Ok(false),
        },
    };
    // Never overwrite the user's own files; our generated ones may be refreshed.
    if dst.exists() && asset.origin != "generated" {
        return Ok(false);
    }
    if dst.is_dir() {
        fs::remove_dir_all(&dst)?;
    }
    copy_into(&src, &dst)?;
    Ok(true)
}

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

/// Copy the files the values will touch into `backups/pre-apply-<name>-…`.
fn backup_targets(name: &str, values: &[(String, Value)]) -> Result<PathBuf> {
    let backups = backups_dir();
    fs::create_dir_all(&backups)?;
    let dir = unique_dir(&backups, &format!("pre-apply-{name}"))?;
    let home = home_dir();
    for (id, _) in values {
        let Some(f) = field::field(id) else { continue };
        let Some(path) = field::file_for(f, &Base::Live) else { continue };
        if !path.is_file() {
            continue;
        }
        let rel = path.strip_prefix(&home).map(Path::to_path_buf).unwrap_or_else(|_| PathBuf::from("_external").join(path.file_name().unwrap_or_default()));
        let dst = dir.join(rel);
        if !dst.exists() {
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &dst)?;
        }
    }
    Ok(dir)
}

fn run(cmd: &str, args: &[&str]) -> bool {
    if std::env::var("KTM_THEME_NO_APPLY").as_deref() == Ok("1") {
        return true;
    }
    std::process::Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Apply profile `name` (a theme package).
pub fn apply(name: &str, options: &ApplyOptions) -> Result<ApplyReport> {
    validate_profile_name(name)?;
    let dir = profile_dir(name)?;
    let package = load(&dir)?;
    let mut report = ApplyReport::default();

    for asset in &package.assets {
        if asset.kind != "wallpaper" && install_asset(&dir, asset)? {
            report.installed.push(format!("{} {}", asset.kind, asset.name));
        }
    }

    let mut values: Vec<(String, Value)> = Vec::new();
    for (id, value) in &package.values {
        if !available(id, value) && options.install_missing {
            if let Some(src) = package.sources.get(id) {
                if let Some(app) = store_app(id, &src.store) {
                    match crate::stores::install(app, &src.id) {
                        Ok(_) => report.installed.push(format!("{app} {}", src.id)),
                        Err(e) => report.errors.push(format!("{id}: install {}: {e:#}", src.id)),
                    }
                }
            }
        }
        // App configs (JSON/user.js/kv/TOML) are edited, never created: no
        // file means the app is not set up here — skip, don't fail.
        let app_absent = field::field(id).is_some_and(|f| {
            matches!(f.source.as_str(), "json" | "userjs" | "kv" | "toml")
                && field::file_for(f, &Base::Live).is_none_or(|p| !p.is_file())
        });
        if app_absent {
            report.skipped.push(id.clone());
            continue;
        }
        if available(id, value) {
            values.push((id.clone(), value.clone()));
        } else {
            let name = field::value_str(value);
            report.missing.push(Missing { field: id.clone(), source: source_hint(&package, id, &name), name });
        }
    }

    let backup = backup_targets(name, &values)?;
    report.backup = backup.display().to_string();

    // A fixed (not wallpaper) accent that the theme leaves unset means "the
    // colour scheme's own accent": drop the previous theme's AccentColor
    // before the scheme is applied, or it tints the new scheme.
    let from_wallpaper = package.values.get("accent_from_wallpaper").map(field::value_str);
    if from_wallpaper.as_deref() == Some("false") && !package.values.contains_key("accent_color") {
        run("kwriteconfig6", &["--file", "kdeglobals", "--group", "General", "--key", "AccentColor", "--delete"]);
    }

    // The theme carries every value a Global Theme would set, so its key is
    // only recorded: `plasma-apply-lookandfeel -a` resets icons, colours and
    // the widget style asynchronously, after the theme's own values.
    let applied = field::apply_with(&values, &["look_and_feel"]);

    // Whole files of user-defined components, then their reload commands.
    let home = home_dir();
    for rel in &package.files {
        if rel.contains("..") || rel.starts_with('/') {
            continue;
        }
        let src = dir.join("assets/files").join(rel);
        let dst = live_path(rel);
        if !src.exists() {
            continue;
        }
        if dst.is_file() {
            let keep = backup.join(dst.strip_prefix(&home).unwrap_or(Path::new(rel)));
            if let Some(parent) = keep.parent() {
                fs::create_dir_all(parent)?;
            }
            let _ = fs::copy(&dst, keep);
        }
        if dst.is_dir() {
            let _ = fs::remove_dir_all(&dst);
        }
        copy_into(&src, &dst)?;
        report.applied.push(format!("file:{rel}"));
    }
    if !package.files.is_empty() {
        crate::profile::run_custom_reload_hooks(&package.files);
    }
    report.applied = applied.applied;
    report.errors.extend(applied.errors);

    // Accent from the wallpaper: wake the kded accent service (flip the flag
    // with --notify; an unchanged write notifies nothing). Never flip a fixed
    // accent through `true`: the service would derive an accent from the
    // wallpaper and re-tint the theme's selection colours.
    if from_wallpaper.as_deref() == Some("true") {
        for v in ["false", "true"] {
            run("kwriteconfig6", &["--notify", "--file", "kdeglobals", "--group", "General", "--key", "accentColorFromWallpaper", v]);
        }
    }
    // Tell running apps that the palette changed.
    run("dbus-send", &["--session", "--type=signal", "/KGlobalSettings", "org.kde.KGlobalSettings.notifyChange", "int32:0", "int32:0"]);

    if let Some(p) = &package.palette {
        palette::save(p, &palette::live_palette_path())?;
    }

    if let Some(rel) = package.wallpaper.as_ref().filter(|_| !options.keep_wallpaper) {
        let src = dir.join(rel);
        if let Some(file_name) = src.file_name() {
            let target = wallpaper_dir().join(file_name);
            if !target.exists() && src.is_file() {
                copy_into(&src, &target)?;
            }
            if target.is_file() && !run("plasma-apply-wallpaperimage", &[&target.to_string_lossy()]) {
                report.errors.push("wallpaper: plasma-apply-wallpaperimage failed".into());
            }
        }
    }

    write_active(name)?;
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

#[cfg(test)]
mod tests {
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
    fn unsafe_asset_names_are_never_available_or_installed() {
        let k = &BUNDLED_KINDS[0];
        assert!(!kind_available(k, "../x"));
        let tmp = tempfile::tempdir().unwrap();
        let asset = Asset { kind: "color-scheme".into(), name: "../evil".into(), path: "assets/x".into(), origin: "user".into(), redistributable: false };
        assert!(!install_asset(tmp.path(), &asset).unwrap());
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
