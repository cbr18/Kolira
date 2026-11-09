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
pub const SCHEMA: &str = "kolira/theme@2";

/// Branding does not invalidate packages exported by earlier versions.
pub fn supported_schema(schema: &str) -> bool {
    matches!(schema, SCHEMA | "theme-profile/theme@2")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Asset {
    /// color-scheme | kvantum | look-and-feel | desktoptheme | icons | btop | zed | wallpaper
    pub kind: String,
    pub name: String,
    /// Path inside the package (`$HOME`-relative for snapshot files).
    pub path: String,
    /// "generated" (from a palette — ours) or "user" (third-party/local copy).
    pub origin: String,
    /// May be published (generated assets: yes; third-party copies: unknown → no).
    pub redistributable: bool,
    /// Licence of the asset (`kolira license`), e.g. "CC0-1.0".
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
    /// GSettings' GTK theme when it differs from `settings.ini`: what Wayland
    /// GTK apps really used when this theme was saved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gsettings_gtk_theme: Option<String>,
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
    /// Explicit user choice: apply without starting any dependency installation.
    pub skip_dependencies: bool,
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
    pub operation_id: Option<String>,
    pub applied: Vec<String>,
    /// App settings not touched because the app has no config here.
    pub skipped: Vec<String>,
    pub installed: Vec<String>,
    pub missing: Vec<Missing>,
    pub errors: Vec<String>,
    pub backup: String,
    /// Dependencies not installed here (deps.rs), to offer for installation.
    pub dependencies: Vec<Needed>,
    /// Follow-up actions to offer (restart programs, Telegram…).
    pub actions: Vec<crate::actions::Action>,
    /// Optional dependencies missing here — reported as warnings, not blockers.
    pub optional_warnings: Vec<String>,
}

/// A missing dependency with its index in `theme.json` (for `deps --install`)
/// and, for system packages, the command the user runs.
#[derive(Debug, Clone, Serialize)]
pub struct Needed {
    pub index: usize,
    pub resource_id: String,
    #[serde(flatten)]
    pub dependency: crate::deps::Dependency,
    pub command: Option<String>,
}

/// The user's automation choices for the dependencies an apply found
/// missing: `always` installs them now (store downloads; system packages in a
/// terminal), `never` drops them; only `ask` ones stay to be offered.
/// Required missing dependencies stay in `report.dependencies`; optional ones
/// move to `report.optional_warnings` and never block apply.
fn apply_dependency_policies(name: &str, report: &mut ApplyReport) -> Result<()> {
    use crate::automation::{policy, Policy};
    let (store, packages) = (policy("store"), policy("packages"));
    let of = |n: &Needed| {
        if crate::deps::needs_root(&n.dependency) {
            packages
        } else {
            store
        }
    };
    let auto: Vec<usize> = report
        .dependencies
        .iter()
        .filter(|n| of(n) == Policy::Always && n.dependency.exact)
        .map(|n| n.index)
        .collect();
    if !auto.is_empty() {
        for (i, result) in install_dependencies(name, Some(&auto))? {
            match result {
                Ok(msg) => report.installed.push(msg),
                Err(e) => report.errors.push(format!("dependency {i}: {e:#}")),
            }
        }
        report.missing = check(name)?;
        report.dependencies = needed(name)?;
    }
    // Split remaining into required and optional. Required stay for the user
    // to see; optional go to warnings and never block apply.
    let (required, optional): (Vec<_>, Vec<_>) = report
        .dependencies
        .drain(..)
        .partition(|n| !n.dependency.optional);
    report.dependencies = required
        .into_iter()
        .filter(|n| of(n) == Policy::Ask || (of(n) == Policy::Always && !n.dependency.exact))
        .collect();
    report.optional_warnings = optional
        .into_iter()
        .filter(|n| of(n) == Policy::Ask || (of(n) == Policy::Always && !n.dependency.exact))
        .map(|n| {
            format!(
                "optional: {} ({}) — {}",
                n.dependency.name,
                n.dependency.kind,
                n.command
                    .as_deref()
                    .map(|c| format!("install: {c}"))
                    .unwrap_or_else(|| format!("get: {}", n.dependency.url))
            )
        })
        .collect();
    Ok(())
}

/// Dependencies of theme `name` not installed on this machine.
pub fn needed(name: &str) -> Result<Vec<Needed>> {
    let package = load(&profile_dir(name)?)?;
    Ok(crate::deps::missing(&package.dependencies)
        .into_iter()
        .map(|(index, d)| Needed {
            index,
            resource_id: crate::dependency_registry::resource_id(d),
            dependency: d.clone(),
            command: crate::deps::package_command(d),
        })
        .collect())
}

/// Missing dependencies split into required (block the look) and optional
/// (theme works without them), each with its index in `theme.json`.
pub fn needed_split(name: &str) -> Result<(Vec<Needed>, Vec<Needed>)> {
    let package = load(&profile_dir(name)?)?;
    let all: Vec<_> = crate::deps::missing(&package.dependencies)
        .into_iter()
        .map(|(index, d)| Needed {
            index,
            resource_id: crate::dependency_registry::resource_id(d),
            dependency: d.clone(),
            command: crate::deps::package_command(d),
        })
        .collect();
    let required: Vec<_> = all
        .iter()
        .filter(|n| !n.dependency.optional)
        .cloned()
        .collect();
    let optional: Vec<_> = all
        .iter()
        .filter(|n| n.dependency.optional)
        .cloned()
        .collect();
    Ok((required, optional))
}

/// Re-detect the dependencies of theme `name` on this machine (themes saved
/// before dependencies existed, or after installing something).
pub fn refresh_dependencies(name: &str) -> Result<Vec<crate::deps::Dependency>> {
    let dir = profile_dir(name)?;
    let mut package = load(&dir)?;
    let mut fresh = crate::deps::detect(&dir, &package.values, &package.sources);
    keep_known_titles(&package.dependencies, &mut fresh);
    package.dependencies = fresh;
    write_manifest(&dir, &package)?;
    Ok(package.dependencies)
}

/// A re-detection on a machine without the app (VS Code not installed) finds
/// the source but not its display name: keep the title found earlier.
fn keep_known_titles(old: &[crate::deps::Dependency], fresh: &mut [crate::deps::Dependency]) {
    for d in fresh
        .iter_mut()
        .filter(|d| d.title.is_empty() && !d.id.is_empty())
    {
        if let Some(prev) = old.iter().find(|o| {
            o.kind == d.kind && o.source == d.source && o.id == d.id && !o.title.is_empty()
        }) {
            d.title = prev.title.clone();
        }
    }
}

/// Install dependencies `indexes` (None = all missing) of theme `name`;
/// `(index, result message or error)` per dependency.
pub fn install_dependencies(
    name: &str,
    indexes: Option<&[usize]>,
) -> Result<Vec<(usize, Result<String>)>> {
    let package = load(&profile_dir(name)?)?;
    let mut out = Vec::new();
    let chosen: Vec<(usize, &crate::deps::Dependency)> =
        crate::deps::missing(&package.dependencies)
            .into_iter()
            .filter(|(i, _)| indexes.is_none_or(|ix| ix.contains(i)))
            .collect();
    // System packages need root: one terminal with one
    // command for all of them — the user sees it and types the password.
    let root: Vec<&crate::deps::Dependency> = chosen
        .iter()
        .filter(|(_, d)| crate::deps::needs_root(d))
        .map(|(_, d)| *d)
        .collect();
    let root_before: BTreeMap<String, bool> = root
        .iter()
        .map(|d| {
            (
                crate::dependency_registry::resource_id(d),
                crate::dependency_registry::package_present(&d.id),
            )
        })
        .collect();
    let root_result = (!root.is_empty()).then(|| match crate::deps::root_command(&root) {
        Some(cmd) => {
            crate::deps::run_in_terminal(&cmd).map(|term| format!("installed in {term}: {cmd}"))
        }
        None => Err(anyhow!(
            "AUR packages need an AUR helper (yay or paru): install one first"
        )),
    });
    for (i, d) in chosen {
        if crate::deps::needs_root(d) {
            let r = match &root_result {
                Some(Ok(msg)) if crate::deps::installed(d) => Ok(format!(
                    "{} — {msg}",
                    if d.title.is_empty() {
                        d.name.as_str()
                    } else {
                        &d.title
                    }
                )),
                Some(Ok(_)) => Err(anyhow!(
                    "package command finished but {} is still missing",
                    d.name
                )),
                Some(Err(e)) => Err(anyhow!("{e:#}")),
                None => Err(anyhow!("nothing to run")),
            };
            crate::dependency_registry::record_package_result(
                d,
                root_before[&crate::dependency_registry::resource_id(d)],
                &r,
            )?;
            out.push((i, r));
        } else {
            out.push((i, crate::deps::install(d)));
        }
    }
    Ok(out)
}

pub fn is_package(dir: &Path) -> bool {
    dir.join(MANIFEST).is_file()
}

pub fn load(dir: &Path) -> Result<Package> {
    let text = fs::read_to_string(dir.join(MANIFEST))
        .with_context(|| format!("no {MANIFEST} in {}", dir.display()))?;
    let mut package: Package = serde_json::from_str(&text).context("invalid theme.json")?;
    if !supported_schema(&package.schema) {
        return Err(anyhow!("unsupported theme schema {:?}", package.schema));
    }
    // Not the app's business any more, older themes listed them: the login
    // screen, and a launcher widget nobody should need for a theme.
    package
        .dependencies
        .retain(|d| d.kind != "sddm" && d.name != "org.kde.plasma.simplekickoff");
    package.assets.retain(|asset| {
        asset.kind != "fastfetch-logo" && !crate::appearance::is_preserved(&asset.path)
    });
    package
        .dependencies
        .retain(|dependency| dependency.kind != "fastfetch" && dependency.name != "fastfetch");
    validate_assets(&package)?;
    Ok(package)
}

/// Packages may carry appearance resources, never arbitrary HOME files.
/// Validate metadata before dependency installs or any filesystem mutation.
pub fn validate_assets(package: &Package) -> Result<()> {
    for asset in &package.assets {
        if asset.kind == "fastfetch-logo" || crate::appearance::is_preserved(&asset.path) {
            continue;
        }
        let root = asset_root(&asset.kind, &asset.path)
            .ok_or_else(|| anyhow!("invalid {} asset path: {}", asset.kind, asset.path))?;
        if !normal_relative(&asset.path) || Path::new(&asset.path) == Path::new(root) {
            return Err(anyhow!("invalid asset path: {}", asset.path));
        }
    }
    if let Some(path) = &package.wallpaper {
        if !normal_relative(path) || !path.starts_with("wallpaper/") {
            return Err(anyhow!("invalid wallpaper path: {path}"));
        }
    }
    Ok(())
}

fn normal_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && Path::new(path)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}

fn asset_root(kind: &str, path: &str) -> Option<&'static str> {
    let (root, suffix) = match kind {
        "color-scheme" => (".local/share/color-schemes", ".colors"),
        "kvantum" => (".config/Kvantum", ""),
        "look-and-feel" => (".local/share/plasma/look-and-feel", ""),
        "desktoptheme" => (".local/share/plasma/desktoptheme", ""),
        "btop" => (".config/btop/themes", ".theme"),
        "aurorae" => (".local/share/aurorae/themes", ""),
        "zed" => (".config/zed/themes", ".json"),
        "konsole-scheme" => (".local/share/konsole", ".colorscheme"),
        "icons" => (".local/share/icons", ""),
        "wallpaper" => ("wallpaper", ""),
        _ => return None,
    };
    let tail = Path::new(path).strip_prefix(root).ok()?;
    if tail.as_os_str().is_empty() || !path.ends_with(suffix) {
        return None;
    }
    // Directory assets name exactly one package. File assets may be nested.
    if suffix.is_empty() && tail.components().count() != 1 {
        return None;
    }
    Some(root)
}

/// Refuse links in package assets or below their destination container. A
/// user may relocate the container itself (e.g. XDG_DATA_HOME), but a theme
/// cannot use a link below it to redirect writes to unrelated files.
pub(crate) fn validate_asset_restore(dir: &Path, relative: &str) -> Result<()> {
    let root = [
        "color-scheme",
        "kvantum",
        "look-and-feel",
        "desktoptheme",
        "btop",
        "aurorae",
        "zed",
        "konsole-scheme",
        "icons",
    ]
    .into_iter()
    .find_map(|kind| asset_root(kind, relative))
    .ok_or_else(|| anyhow!("invalid asset destination: {relative}"))?;
    if !normal_relative(relative) {
        return Err(anyhow!("invalid asset destination: {relative}"));
    }
    let source = dir.join(relative);
    let mut ancestor = source.parent();
    while let Some(path) = ancestor {
        if path == dir {
            break;
        }
        reject_link(path)?;
        ancestor = path.parent();
    }
    let destination = live_path(relative);
    let container = live_path(root);
    let mut current = Some(destination.as_path());
    while let Some(path) = current {
        if path == container {
            break;
        }
        reject_link(path)?;
        current = path.parent();
    }
    validate_resource_tree(&source, &destination)
}

fn reject_link(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(anyhow!(
            "symlink in theme asset restore: {}",
            path.display()
        )),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn validate_resource_tree(source: &Path, destination: &Path) -> Result<()> {
    reject_link(source)?;
    reject_link(destination)?;
    if source.is_dir() {
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            validate_resource_tree(&entry.path(), &destination.join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn write_manifest(dir: &Path, package: &Package) -> Result<()> {
    fs::write(
        dir.join(MANIFEST),
        serde_json::to_string_pretty(package)? + "\n",
    )?;
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
    /// Prefix of the field value before the asset name (Aurorae:
    /// `__aurorae__svg__<theme>`); a value without it names no asset here.
    prefix: &'static str,
}

impl AssetKind {
    /// Asset name a field value refers to (None: not this kind's asset).
    fn asset_name(&self, value: &str) -> Option<String> {
        value
            .strip_prefix(self.prefix)
            .filter(|n| safe_name(n))
            .map(str::to_string)
    }
}

const BUNDLED_KINDS: &[AssetKind] = &[
    AssetKind {
        field: "icon_theme",
        kind: "icons",
        user_rel: ".local/share/icons",
        system: &["/usr/share/icons"],
        suffix: "",
        prefix: "",
    },
    AssetKind {
        field: "color_scheme",
        kind: "color-scheme",
        user_rel: ".local/share/color-schemes",
        system: &["/usr/share/color-schemes"],
        suffix: ".colors",
        prefix: "",
    },
    AssetKind {
        field: "kvantum_theme",
        kind: "kvantum",
        user_rel: ".config/Kvantum",
        system: &["/usr/share/Kvantum"],
        suffix: "",
        prefix: "",
    },
    AssetKind {
        field: "look_and_feel",
        kind: "look-and-feel",
        user_rel: ".local/share/plasma/look-and-feel",
        system: &["/usr/share/plasma/look-and-feel"],
        suffix: "",
        prefix: "",
    },
    AssetKind {
        field: "plasma_theme",
        kind: "desktoptheme",
        user_rel: ".local/share/plasma/desktoptheme",
        system: &["/usr/share/plasma/desktoptheme"],
        suffix: "",
        prefix: "",
    },
    AssetKind {
        field: "btop_theme",
        kind: "btop",
        user_rel: ".config/btop/themes",
        system: &["/usr/share/btop/themes", "/usr/local/share/btop/themes"],
        suffix: ".theme",
        prefix: "",
    },
    AssetKind {
        field: "decoration_theme",
        kind: "aurorae",
        user_rel: ".local/share/aurorae/themes",
        system: &["/usr/share/aurorae/themes"],
        suffix: "",
        prefix: "__aurorae__svg__",
    },
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
    let mut dirs: Vec<PathBuf> = fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .collect();
    dirs.sort();
    dirs.into_iter()
        .find(|d| d.join(format!("{name}.kvconfig")).is_file())
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
        "icon_theme" => [
            data_home().join("icons"),
            home_dir().join(".icons"),
            PathBuf::from("/usr/share/icons"),
        ]
        .iter()
        .any(|r| r.join(&name).join("index.theme").is_file()),
        "gtk_theme" => [
            data_home().join("themes"),
            home_dir().join(".themes"),
            PathBuf::from("/usr/share/themes"),
        ]
        .iter()
        .any(|r| r.join(&name).is_dir()),
        "zed_dark" | "zed_light" => {
            zed_theme_file(&name, &Base::Live).is_some() || ZED_BUILTIN.contains(&name.as_str())
        }
        "vscode_theme" => {
            vscode_theme_extension("vscode", &name).is_some()
                || VSCODE_BUILTIN.contains(&name.as_str())
        }
        "cursor_ide_theme" => {
            vscode_theme_extension("cursor", &name).is_some()
                || name.starts_with("Cursor ")
                || VSCODE_BUILTIN.contains(&name.as_str())
        }
        other => BUNDLED_KINDS
            .iter()
            .find(|k| k.field == other)
            .is_none_or(|k| k.asset_name(&name).is_none_or(|n| kind_available(k, &n))),
    }
}

const ZED_BUILTIN: &[&str] = &[
    "Zed (Default)",
    "One Dark",
    "One Light",
    "Ayu Dark",
    "Ayu Light",
    "Ayu Mirage",
    "Gruvbox Dark",
    "Gruvbox Dark Hard",
    "Gruvbox Dark Soft",
    "Gruvbox Light",
    "Gruvbox Light Hard",
    "Gruvbox Light Soft",
];
const VSCODE_BUILTIN: &[&str] = &[
    "Default Dark Modern",
    "Default Light Modern",
    "Default Dark+",
    "Default Light+",
    "Visual Studio Dark",
    "Visual Studio Light",
    "Abyss",
    "Kimbie Dark",
    "Monokai",
    "Solarized Dark",
    "Solarized Light",
    "Tomorrow Night Blue",
    "Red",
    "Quiet Light",
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
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        files.sort();
        for file in files {
            let Ok(text) = fs::read_to_string(&file) else {
                continue;
            };
            let Ok(json) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            let declares = json
                .get("themes")
                .and_then(Value::as_array)
                .is_some_and(|t| {
                    t.iter()
                        .any(|x| x.get("name").and_then(Value::as_str) == Some(theme_name))
                });
            if declares {
                return Some(file);
            }
        }
    }
    None
}

/// Zed extension id providing theme file `file`: our installs are named
/// `<extension>--<file>.json`; Zed's own installs live in
/// `zed/extensions/installed/<extension>/themes/`.
fn zed_extension_of(file: &Path, file_name: &str) -> Option<String> {
    if let Some((ext, _)) = file_name.split_once("--") {
        return Some(ext.to_string());
    }
    let themes = file.parent()?;
    let ext = themes.parent()?;
    (themes.file_name()? == "themes" && ext.parent()?.file_name()? == "installed")
        .then(|| ext.file_name()?.to_str().map(str::to_string))
        .flatten()
}

/// `publisher.name` of the installed extension contributing a VS Code/Cursor
/// theme (matched by theme `id` or `label`).
fn vscode_theme_extension(app: &str, theme: &str) -> Option<String> {
    let home = home_dir();
    let roots: Vec<PathBuf> = if app == "cursor" {
        vec![home.join(".cursor/extensions")]
    } else {
        vec![
            home.join(".vscode/extensions"),
            home.join(".vscode-oss/extensions"),
        ]
    };
    for root in roots {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(text) = fs::read_to_string(entry.path().join("package.json")) else {
                continue;
            };
            let Ok(pkg) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
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
        generator: format!("kolira {}", env!("CARGO_PKG_VERSION")),
        ..Default::default()
    };

    let home = home_dir();
    for k in BUNDLED_KINDS {
        let Some(value) = values.get(k.field) else {
            continue;
        };
        let Some(asset) = k.asset_name(&field::value_str(value)) else {
            continue;
        };
        let in_snapshot = asset_in(k, &dir.join(k.user_rel), &asset);
        let path = match in_snapshot {
            Some(p) => p,
            None => {
                // Not tracked: copy the live user asset in (system ones stay references).
                let Some(live) = asset_in(k, &live_path(k.user_rel), &asset) else {
                    continue;
                };
                let rel = live
                    .strip_prefix(&home)
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|_| {
                        Path::new(k.user_rel).join(live.file_name().unwrap_or_default())
                    });
                copy_into(&live, &dir.join(&rel))?;
                if k.kind == "look-and-feel" {
                    // A Global Theme's layout scripts are an arrangement, not a look.
                    let _ = fs::remove_dir_all(dir.join(&rel).join(crate::layout::LAF_LAYOUTS));
                }
                dir.join(rel)
            }
        };
        let rel = path
            .strip_prefix(dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        // Ours: palette-generated assets (Kolira…) and the Look & Feel
        // packages `kolira look-and-feel` writes (kolira-…).
        let generated = asset.starts_with("Kolira")
            || (k.kind == "look-and-feel" && asset.starts_with("kolira-"));
        // Generated from the palette: ours (colour scheme: MIT). A Kvantum
        // recolour keeps its base's licence: GPL for the themes that ship with
        // Kvantum; another base (store theme, unknown licence) stays private.
        let (license, redistributable) = match (generated, k.kind) {
            (true, "kvantum") => {
                let kv =
                    fs::read_to_string(path.join(format!("{asset}.kvconfig"))).unwrap_or_default();
                match crate::kvantum::provenance(&kv) {
                    Some((base, true)) => (
                        format!("GPL-3.0-or-later (recoloured from Kvantum's {base})"),
                        true,
                    ),
                    Some((base, false)) => (format!("licence of {base} (recoloured)"), false),
                    // Generated before provenance was recorded: always from KvArc/KvArcDark.
                    None => (
                        "GPL-3.0-or-later (recoloured from Kvantum's KvArc)".to_string(),
                        true,
                    ),
                }
            }
            (true, _) => ("MIT (generated by kolira)".to_string(), true),
            _ => (String::new(), false),
        };
        package.assets.push(Asset {
            kind: k.kind.into(),
            name: asset,
            path: rel,
            origin: if generated { "generated" } else { "user" }.into(),
            redistributable,
            license,
            source: String::new(),
        });
    }

    for id in ["zed_dark", "zed_light"] {
        let Some(theme) = values.get(id).map(field::value_str) else {
            continue;
        };
        if let Some(file) =
            zed_theme_file(&theme, &snapshot).or_else(|| zed_theme_file(&theme, &Base::Live))
        {
            let file_name = file
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if let Ok(rel) = file.strip_prefix(dir) {
                package.assets.push(Asset {
                    kind: "zed".into(),
                    name: theme.clone(),
                    path: rel.to_string_lossy().into(),
                    origin: "user".into(),
                    redistributable: false,
                    license: String::new(),
                    source: String::new(),
                });
            }
            if let Some(ext) = zed_extension_of(&file, &file_name) {
                package.sources.insert(
                    id.into(),
                    Source {
                        store: "zed".into(),
                        id: ext,
                        url: String::new(),
                    },
                );
            }
        }
    }
    for (id, app, store) in [
        ("vscode_theme", "vscode", "vscode-marketplace"),
        ("cursor_ide_theme", "cursor", "open-vsx"),
    ] {
        if let Some(theme) = values.get(id).map(field::value_str) {
            if let Some(ext) = vscode_theme_extension(app, &theme) {
                package.sources.insert(
                    id.into(),
                    Source {
                        store: store.into(),
                        id: ext,
                        url: String::new(),
                    },
                );
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
            let generated = name.starts_with("Kolira");
            package.assets.push(Asset {
                kind: "konsole-scheme".into(),
                name,
                path: format!(".local/share/konsole/{file}"),
                origin: if generated { "generated" } else { "user" }.into(),
                redistributable: generated,
                license: if generated {
                    "MIT (generated by kolira)".into()
                } else {
                    String::new()
                },
                source: String::new(),
            });
        }
    }

    if let Ok(p) = palette::load(&snapshot.resolve(palette::LIVE_PALETTE_REL)) {
        if values.get("color_scheme").map(field::value_str).as_deref()
            == Some(palette::kde_scheme_id(&p).as_str())
        {
            package.palette = Some(p);
        }
    }

    // Wallpaper: `<theme>/wallpaper/` (copy + source) written by the snapshot
    // save; without it applying leaves the wallpaper alone.
    let wallpaper_dir = dir.join(WALLPAPER_DIR);
    if options.include_wallpaper {
        if let Some(file) = fs::read_dir(&wallpaper_dir).ok().and_then(|it| {
            it.flatten()
                .map(|e| e.path())
                .find(|p| p.is_file() && p.file_name().is_some_and(|n| n != "source"))
        }) {
            let file_name = file
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let rel = format!("{WALLPAPER_DIR}/{file_name}");
            package.wallpaper = Some(rel.clone());
            package.assets.push(Asset {
                kind: "wallpaper".into(),
                name: file_name,
                path: rel,
                origin: "user".into(),
                redistributable: false,
                license: String::new(),
                source: String::new(),
            });
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
    copy_tree(dir, &backup)
        .with_context(|| format!("back up {} to {}", dir.display(), backup.display()))?;
    for rel in crate::profile::resolve_cli_paths(None)? {
        let path = dir.join(&rel);
        if !path.is_file()
            || matches!(
                crate::appearance::policy(&rel),
                crate::appearance::Policy::Whole
            )
        {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let has_include =
            crate::appearance::include_of(&rel).is_some_and(|inc| dir.join(inc).is_file());
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
    let _lock = crate::operation::MutationGuard::acquire()?;
    validate_profile_name(name)?;
    let target = profile_dir(name)?;
    let mut options = options.clone();
    if options.description.is_none() && target.is_dir() {
        options.description =
            Some(crate::profile::get_profile_description(name).unwrap_or_default());
    }
    let previous = load(&target).ok();
    crate::profile::save_profile_cli(name, None)?;
    for rel in crate::appearance::unused_files_in(&target) {
        let _ = fs::remove_file(target.join(rel));
    }
    if !options.include_wallpaper {
        drop_stored_wallpaper(&target);
    }
    let mut package = build(name, &target, &options)?;
    if let Some(prev) = previous {
        carry_licences(&prev, &mut package);
        keep_known_titles(&prev.dependencies, &mut package.dependencies);
    }
    // What Wayland GTK apps really use can differ from settings.ini; returning
    // to this theme must not change them, nor the file.
    let ini = crate::config_parser::ini_group_key(
        &target.join(".config/gtk-3.0/settings.ini"),
        "Settings",
        "gtk-theme-name",
    );
    package.gsettings_gtk_theme =
        crate::undo::live_gtk_theme().filter(|live| ini.as_deref() != Some(live.as_str()));
    write_manifest(&target, &package)?;
    Ok(package)
}

/// A theme without a wallpaper names none: not the author's path, not a plugin.
fn drop_stored_wallpaper(dir: &Path) {
    let path = dir.join(crate::wallpaper::APPLETSRC_REL);
    if let Ok(text) = fs::read_to_string(&path) {
        let _ = fs::write(&path, crate::layout::without_wallpaper(&text));
    }
}

/// Keep the author's licence marks (`kolira license`) across re-saves.
fn carry_licences(from: &Package, to: &mut Package) {
    // Generated assets get their licence computed on every save (a Kvantum
    // recolour follows its base): an old computed label is not carried, a
    // mark the author set by hand (e.g. "derived from Breeze") is.
    let computed = |l: &str| {
        l.starts_with("MIT (generated by kolira)")
            || l.starts_with("GPL-3.0-or-later (recoloured from Kvantum's")
            || l.starts_with("licence of ")
    };
    for a in to.assets.iter_mut() {
        if let Some(old) = from.assets.iter().find(|o| {
            o.kind == a.kind
                && o.name == a.name
                && !o.license.is_empty()
                && !(a.origin == "generated" && computed(&o.license))
        }) {
            a.license = old.license.clone();
            a.source = old.source.clone();
            a.redistributable = old.redistributable;
        }
    }
}

/// Mark asset `asset` (its name, e.g. a wallpaper file or a Kvantum theme) of
/// theme `name` with a licence and source; `redistributable` decides whether
/// `export --public` may ship it.
pub fn set_licence(
    name: &str,
    asset: &str,
    licence: &str,
    source: &str,
    redistributable: bool,
) -> Result<Asset> {
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

/// Files whose layout part a theme does not carry (`layout.rs`).
const LAYOUT_FILES: &[&str] = &[
    crate::wallpaper::APPLETSRC_REL,
    ".config/plasmashellrc",
    ".config/kwinrulesrc",
    ".config/kwinrc",
];

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
    let mut removed = 0;
    // First, so the description below (values, dependencies) sees the result.
    // Layout files: only their look stays (themes saved before 2026-11-05
    // carried the whole panel/widget layout, desktop icons, widget settings);
    // kwinrc without the settings of effects that are off.
    for rel in LAYOUT_FILES {
        let path = dir.join(rel);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        match crate::appearance::for_theme(rel, &text) {
            Some(look) if look != text => fs::write(&path, look)?,
            Some(_) => {}
            None => {
                fs::remove_file(&path)?;
                removed += 1;
            }
        }
    }
    if old.wallpaper.is_none() {
        drop_stored_wallpaper(&dir);
    }
    // Breeze / Klassy settings this theme's style and decoration do not read.
    for rel in crate::appearance::unused_files_in(&dir) {
        if fs::remove_file(dir.join(rel)).is_ok() {
            removed += 1;
        }
    }
    let options = SaveOptions {
        description: Some(old.description.clone()),
        include_wallpaper: old.wallpaper.is_some(),
    };
    let mut package = build(name, &dir, &options)?;
    package.created = old.created.clone();
    package.gsettings_gtk_theme = old.gsettings_gtk_theme.clone();
    carry_licences(&old, &mut package);
    write_manifest(&dir, &package)?;
    let keep: std::collections::HashSet<PathBuf> =
        crate::share::theme_files(&dir, true)?.into_iter().collect();
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let path = dir.join(&rel);
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
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
        let mut package = load(&target).map_err(|e| {
            anyhow!("{name}: {e:#} (values-only theme@1 packages must be re-saved)")
        })?;
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
            "vscode-marketplace" => format!(
                "https://marketplace.visualstudio.com/items?itemName={}",
                s.id
            ),
            "open-vsx" => format!(
                "https://open-vsx.org/extension/{}",
                s.id.replacen('.', "/", 1)
            ),
            "zed" => format!("Zed extension \"{}\" (zed.dev/extensions)", s.id),
            _ => s.url.clone(),
        };
    }
    match field_id {
        "icon_theme" | "gtk_theme" | "kvantum_theme" | "look_and_feel" | "plasma_theme"
        | "color_scheme" => {
            format!(
                "https://store.kde.org/search?search={}",
                name.replace(' ', "+")
            )
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
/// Fields whose value must exist on this machine to be used.
pub(crate) const KEPT_WHEN_MISSING: &[&str] = &[
    "icon_theme",
    "cursor_theme",
    "gtk_theme",
    "plasma_theme",
    "decoration_theme",
    "widget_style",
];

pub fn apply(name: &str, options: &ApplyOptions) -> Result<ApplyReport> {
    crate::operation::run(name, || apply_inner(name, options))
}
fn apply_inner(name: &str, options: &ApplyOptions) -> Result<ApplyReport> {
    validate_profile_name(name)?;
    let dir = profile_dir(name)?;
    let package = load(&dir)?;
    for asset in package
        .assets
        .iter()
        .filter(|asset| asset.kind != "wallpaper")
    {
        validate_asset_restore(&dir, &asset.path)?;
    }
    let preflight_assets: Vec<String> = package
        .assets
        .iter()
        .filter(|a| a.kind != "wallpaper")
        .map(|a| a.path.clone())
        .collect();
    crate::profile::preflight(name, &preflight_assets)?;
    let mut report = ApplyReport {
        operation_id: crate::operation::current_id(),
        ..Default::default()
    };

    if options.install_missing && !options.skip_dependencies {
        for (id, value) in &package.values {
            if available(id, value) {
                continue;
            }
            let Some(src) = package.sources.get(id) else {
                continue;
            };
            let Some(app) = store_app(id, &src.store) else {
                continue;
            };
            match crate::stores::install(app, &src.id) {
                Ok(_) => report.installed.push(format!("{app} {}", src.id)),
                Err(e) => report
                    .errors
                    .push(format!("{id}: install {}: {e:#}", src.id)),
            }
        }
    }
    report.missing = check(name)?;
    if !options.skip_dependencies {
        let (required, optional) = needed_split(name)?;
        report.dependencies = required;
        report.optional_warnings = optional
            .into_iter()
            .map(|n| {
                format!(
                    "optional: {} ({}) — {}",
                    n.dependency.name,
                    n.dependency.kind,
                    n.command
                        .as_deref()
                        .map(|c| format!("install: {c}"))
                        .unwrap_or_else(|| format!("get: {}", n.dependency.url))
                )
            })
            .collect();
        let outcome = apply_dependency_policies(name, &mut report);
        crate::operation::note_report(serde_json::to_value(&report)?)?;
        outcome?;
    } else {
        report.dependencies = needed(name)?;
    }
    crate::operation::note_report(serde_json::to_value(&report)?)?;

    // Pre-apply warning: required dependencies missing means parts of the
    // look will fall back to whatever is currently live. List them clearly
    // so the user knows before the theme is restored.
    if !report.dependencies.is_empty() {
        let required: Vec<String> = report
            .dependencies
            .iter()
            .map(|d| format!("{} ({})", d.dependency.name, d.dependency.kind))
            .collect();
        eprintln!(
            "warning: required dependencies not installed — apply anyway: {}",
            required.join(", ")
        );
        eprintln!("hint: install with `kolira deps {name} --install`, or skip with `--without-dependencies`");
    }

    if !report.errors.is_empty() {
        anyhow::bail!(
            "Dependency preparation failed: {}",
            report.errors.join("; ")
        );
    }

    // The theme's own assets (colour scheme, Kvantum family, Look & Feel,
    // Plasma style, btop/Zed/Konsole themes) are restored with
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

    // What this machine lacks stays as it is: an icon theme or a window
    // decoration that is not here would leave blank icons or a broken KWin.
    let keep_fields: Vec<String> = report
        .missing
        .iter()
        .filter(|m| KEPT_WHEN_MISSING.contains(&m.field.as_str()))
        .map(|m| m.field.clone())
        .collect();
    for m in report
        .missing
        .iter()
        .filter(|m| KEPT_WHEN_MISSING.contains(&m.field.as_str()))
    {
        report.skipped.push(format!(
            "{}: {} is not installed, kept the current one",
            m.field, m.name
        ));
    }
    let backup = crate::profile::apply_profile_keeping(
        name,
        &extras,
        package.wallpaper.is_none() || options.keep_wallpaper,
        &keep_fields,
    )?;
    // Telegram: nothing to run — once the user chose the palette file in
    // Telegram, it watches the file and re-applies it after the restore.
    if dir.join(crate::spicetify::CONFIG).is_file() {
        crate::spicetify::refresh();
    }
    if dir.join(crate::extras::CAVA).is_file() {
        crate::extras::reload_cava();
    }
    if dir.join(".config/bat/themes").is_dir()
        && std::env::var("KTM_THEME_NO_APPLY").as_deref() != Ok("1")
        && which::which("bat").is_ok()
    {
        let status = crate::paths::status_bounded(
            std::process::Command::new("bat")
                .args(["cache", "--build"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null()),
        );
        if !status.is_some_and(|status| status.success()) {
            report
                .errors
                .push("bat cache --build failed or timed out".into());
        }
    }
    report.backup = backup.display().to_string();
    report.applied = crate::profile::get_profile_components(name).unwrap_or_default();
    // Restarts, Telegram's one-time choice: offered, not done (actions.rs) —
    // a program may be in a call.
    for action in crate::actions::for_changes(&dir, &crate::operation::changed_paths()?) {
        let policy = if action.kind == "restart" {
            crate::automation::policy(&format!("restart_{}", action.id))
        } else {
            crate::automation::Policy::Ask
        };
        match policy {
            crate::automation::Policy::Always => match crate::actions::run(&action.id) {
                Ok(msg) => report.installed.push(msg),
                Err(e) => report.errors.push(format!("{}: {e:#}", action.title)),
            },
            crate::automation::Policy::Never => {}
            crate::automation::Policy::Ask => report.actions.push(action),
        }
    }

    if let Some(path) = keep {
        if plasma_wallpaper_from(&live_path(APPLETSRC_REL)).as_deref()
            != Some(path.to_string_lossy().as_ref())
        {
            crate::profile::apply_wallpaper_live(&path);
        }
    }
    crate::operation::note_report(serde_json::to_value(&report)?)?;
    if !report.errors.is_empty() {
        anyhow::bail!("Apply follow-up failed: {}", report.errors.join("; "));
    }
    Ok(report)
}

/// What applying `name` would miss (no changes made).
pub fn check(name: &str) -> Result<Vec<Missing>> {
    validate_profile_name(name)?;
    let dir = profile_dir(name)?;
    let package = load(&dir)?;
    let bundled: Vec<(&str, &str)> = package
        .assets
        .iter()
        .map(|a| (a.kind.as_str(), a.name.as_str()))
        .collect();
    Ok(package
        .values
        .iter()
        .filter(|(id, value)| {
            let name = field::value_str(value);
            let in_package = BUNDLED_KINDS.iter().any(|k| {
                k.field == id.as_str()
                    && k.asset_name(&name)
                        .is_some_and(|n| bundled.contains(&(k.kind, n.as_str())))
            }) || (id.starts_with("zed_")
                && bundled.iter().any(|(k, n)| *k == "zed" && *n == name));
            let editor_here =
                crate::stores::editor_of(id).is_none_or(crate::stores::editor_installed);
            editor_here && !in_package && !available(id, value)
        })
        .map(|(id, value)| {
            let name = field::value_str(value);
            Missing {
                field: id.clone(),
                source: source_hint(&package, id, &name),
                name,
            }
        })
        .collect())
}

/// VS Code and Cursor share the extension ecosystem: when one editor is not
/// installed here, the same theme name in the other one tells which extension
/// provides it (the id is the same in both galleries).
fn share_editor_sources(values: &BTreeMap<String, Value>, sources: &mut BTreeMap<String, Source>) {
    for (id, other, store) in [
        ("vscode_theme", "cursor_ide_theme", "vscode-marketplace"),
        ("cursor_ide_theme", "vscode_theme", "open-vsx"),
    ] {
        let same = values
            .get(id)
            .map(field::value_str)
            .is_some_and(|t| values.get(other).map(field::value_str) == Some(t));
        if same && !sources.contains_key(id) {
            if let Some(ext) = sources.get(other).map(|s| s.id.clone()) {
                sources.insert(
                    id.into(),
                    Source {
                        store: store.into(),
                        id: ext,
                        url: String::new(),
                    },
                );
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
        sources.insert(
            "cursor_ide_theme".to_string(),
            Source {
                store: "open-vsx".into(),
                id: "sdras.night-owl".into(),
                url: String::new(),
            },
        );
        share_editor_sources(&values, &mut sources);
        assert_eq!(sources["vscode_theme"].store, "vscode-marketplace");
        assert_eq!(sources["vscode_theme"].id, "sdras.night-owl");
        values.insert("vscode_theme".to_string(), Value::from("Other"));
        let mut sources2 = BTreeMap::new();
        sources2.insert(
            "cursor_ide_theme".to_string(),
            sources["cursor_ide_theme"].clone(),
        );
        share_editor_sources(&values, &mut sources2);
        assert!(
            !sources2.contains_key("vscode_theme"),
            "a different theme name says nothing"
        );
    }
    use super::*;

    #[test]
    fn licence_marks_carry_but_computed_labels_refresh() {
        let asset = |kind: &str, name: &str, origin: &str, licence: &str| Asset {
            kind: kind.into(),
            name: name.into(),
            path: String::new(),
            origin: origin.into(),
            redistributable: true,
            license: licence.into(),
            source: String::new(),
        };
        let old = Package {
            assets: vec![
                asset(
                    "color-scheme",
                    "KoliraPaper",
                    "generated",
                    "LGPL-2.0-or-later (derived from KDE Breeze Light)",
                ),
                asset(
                    "kvantum",
                    "KoliraX",
                    "generated",
                    "GPL-3.0-or-later (recoloured from Kvantum's KvArc)",
                ),
                asset("wallpaper", "w.jpg", "user", "CC0-1.0"),
            ],
            ..Default::default()
        };
        let mut new = Package {
            assets: vec![
                asset(
                    "color-scheme",
                    "KoliraPaper",
                    "generated",
                    "MIT (generated by kolira)",
                ),
                asset(
                    "kvantum",
                    "KoliraX",
                    "generated",
                    "GPL-3.0-or-later (recoloured from Kvantum's KvRoughGlass)",
                ),
                asset("wallpaper", "w.jpg", "user", ""),
            ],
            ..Default::default()
        };
        carry_licences(&old, &mut new);
        assert!(new.assets[0].license.starts_with("LGPL"), "hand mark kept");
        assert!(
            new.assets[1].license.ends_with("KvRoughGlass)"),
            "computed label refreshed"
        );
        assert_eq!(new.assets[2].license, "CC0-1.0");
    }

    #[test]
    fn zed_extension_ids() {
        let ours = Path::new("/h/.config/zed/themes/night-owlz--NightOwl.json");
        assert_eq!(
            zed_extension_of(ours, "night-owlz--NightOwl.json").as_deref(),
            Some("night-owlz")
        );
        let zeds =
            Path::new("/h/.local/share/zed/extensions/installed/glass-theme/themes/glass.json");
        assert_eq!(
            zed_extension_of(zeds, "glass.json").as_deref(),
            Some("glass-theme")
        );
        assert_eq!(
            zed_extension_of(Path::new("/h/.config/zed/themes/mine.json"), "mine.json"),
            None
        );
    }

    #[test]
    fn redetection_keeps_known_titles() {
        let dep = |title: &str| crate::deps::Dependency {
            kind: "vscode".into(),
            name: "dfd05a73".into(),
            source: "vscode-marketplace".into(),
            id: "unthrottled.doki-theme".into(),
            title: title.into(),
            ..Default::default()
        };
        let old = vec![dep("Doki Theme")];
        let mut fresh = vec![dep("")];
        keep_known_titles(&old, &mut fresh);
        assert_eq!(fresh[0].title, "Doki Theme");
    }

    #[test]
    fn package_json_roundtrip() {
        let mut p = Package {
            schema: SCHEMA.into(),
            name: "x".into(),
            ..Default::default()
        };
        p.values.insert("color_scheme".into(), Value::from("Nord"));
        p.sources.insert(
            "vscode_theme".into(),
            Source {
                store: "vscode-marketplace".into(),
                id: "sainnhe.everforest".into(),
                url: String::new(),
            },
        );
        let tmp = tempfile::tempdir().unwrap();
        write_manifest(tmp.path(), &p).unwrap();
        assert_eq!(load(tmp.path()).unwrap(), p);
        fs::write(
            tmp.path().join(MANIFEST),
            r#"{"schema":"other","name":"x"}"#,
        )
        .unwrap();
        assert!(load(tmp.path()).is_err());
    }

    #[test]
    fn source_hints() {
        let mut p = Package::default();
        p.sources.insert(
            "vscode_theme".into(),
            Source {
                store: "vscode-marketplace".into(),
                id: "sainnhe.everforest".into(),
                url: String::new(),
            },
        );
        p.sources.insert(
            "cursor_ide_theme".into(),
            Source {
                store: "open-vsx".into(),
                id: "sainnhe.everforest".into(),
                url: String::new(),
            },
        );
        assert!(source_hint(&p, "vscode_theme", "x").ends_with("itemName=sainnhe.everforest"));
        assert!(source_hint(&p, "cursor_ide_theme", "x")
            .ends_with("open-vsx.org/extension/sainnhe/everforest"));
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
    fn bundled_icons_are_captured_without_an_external_dependency() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".config")).unwrap();
        fs::write(
            root.join(".config/kdeglobals"),
            "[Icons]\nTheme=KoliraTestPixels\n",
        )
        .unwrap();
        let icons = root.join(".local/share/icons/KoliraTestPixels");
        fs::create_dir_all(&icons).unwrap();
        fs::write(icons.join("index.theme"), "[Icon Theme]\nName=Pixels\n").unwrap();
        let package = build("pixels", root, &SaveOptions::default()).unwrap();
        assert!(package
            .assets
            .iter()
            .any(|a| { a.kind == "icons" && a.path == ".local/share/icons/KoliraTestPixels" }));
        assert!(!package.dependencies.iter().any(|d| d.kind == "icons"));
        validate_assets(&package).unwrap();
        let mut unsafe_package = package.clone();
        unsafe_package.assets[0].path = ".local/share/icons/../outside".into();
        assert!(validate_assets(&unsafe_package).is_err());
        unsafe_package.assets[0].path = ".local/share/icons".into();
        assert!(validate_assets(&unsafe_package).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/tmp", icons.join("redirect")).unwrap();
            assert!(validate_asset_restore(root, ".local/share/icons/KoliraTestPixels").is_err());
        }
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
