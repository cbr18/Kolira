//! Theme stores of editors: search and install themes the way the editors
//! themselves would, so the app can offer e.g. an Everforest theme for Zed or
//! Cursor instead of generating one.
//!
//! * Zed — `api.zed.dev` extension index (`provides=themes`); install =
//!   download the extension archive and copy its `themes/*.json` into
//!   `~/.config/zed/themes/` (user themes; no extension host needed).
//! * VS Code / Cursor — Open VSX search (category Themes); install = the
//!   editor's own CLI `--install-extension <id>` (it uses its own gallery).
//!
//! Network goes through [`crate::net`] (can be disabled); ids are validated
//! before any request; archives are size-capped and only `themes/*.json`
//! entries are extracted, by file name (no paths from the archive are used).

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use zip::read::ZipArchive;

use crate::net::{http_get, http_get_bytes};

const ZED_SEARCH: &str = "https://api.zed.dev/extensions";
const ZED_DOWNLOAD: &str = "https://api.zed.dev/extensions";
const OPEN_VSX_SEARCH: &str = "https://open-vsx.org/api/-/search";
const MAX_INDEX: u64 = 4 * 1024 * 1024;
const MAX_ARCHIVE: u64 = 20 * 1024 * 1024;
const MAX_THEME_FILE: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StoreItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub downloads: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct InstallReport {
    pub app: String,
    pub id: String,
    /// Theme names/ids the editor can now select.
    pub themes: Vec<String>,
    pub location: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Store {
    Zed,
    OpenVsx,
}

fn store_for(app: &str) -> Result<Store> {
    match app {
        "zed" => Ok(Store::Zed),
        "vscode" | "cursor" => Ok(Store::OpenVsx),
        other => Err(anyhow!(
            "no theme store for {other:?} (zed, vscode, cursor)"
        )),
    }
}

pub(crate) fn valid_zed_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// `publisher.extension` (VS Code extension id).
pub(crate) fn valid_vsx_id(id: &str) -> bool {
    let Some((publisher, name)) = id.split_once('.') else {
        return false;
    };
    let part = |s: &str| {
        !s.is_empty()
            && s.len() <= 100
            && s.as_bytes()[0].is_ascii_alphanumeric()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    };
    part(publisher) && part(name)
}

fn encode(q: &str) -> String {
    q.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub fn parse_zed_search(json: &str) -> Result<Vec<StoreItem>> {
    let value: serde_json::Value = serde_json::from_str(json).context("invalid Zed index")?;
    let data = value
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| anyhow!("Zed index has no data"))?;
    Ok(data
        .iter()
        .filter(|x| {
            x.get("provides")
                .and_then(|p| p.as_array())
                .is_some_and(|p| p.iter().any(|v| v == "themes"))
        })
        .filter_map(|x| {
            let id = x.get("id")?.as_str()?.to_string();
            valid_zed_id(&id).then(|| StoreItem {
                name: x
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&id)
                    .to_string(),
                description: x
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                version: x
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                downloads: x
                    .get("download_count")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
                id,
            })
        })
        .collect())
}

pub fn parse_open_vsx_search(json: &str) -> Result<Vec<StoreItem>> {
    let value: serde_json::Value =
        serde_json::from_str(json).context("invalid Open VSX response")?;
    let list = value
        .get("extensions")
        .and_then(|d| d.as_array())
        .ok_or_else(|| anyhow!("Open VSX response has no extensions"))?;
    Ok(list
        .iter()
        .filter_map(|x| {
            let id = format!(
                "{}.{}",
                x.get("namespace")?.as_str()?,
                x.get("name")?.as_str()?
            );
            valid_vsx_id(&id).then(|| StoreItem {
                name: x
                    .get("displayName")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&id)
                    .to_string(),
                description: x
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                version: x
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                downloads: x
                    .get("downloadCount")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
                id,
            })
        })
        .collect())
}

/// Theme extensions matching `query`, most downloaded first.
pub fn search(app: &str, query: &str) -> Result<Vec<StoreItem>> {
    let query = query.trim();
    let mut items = match store_for(app)? {
        Store::Zed => parse_zed_search(&http_get(
            &format!(
                "{ZED_SEARCH}?filter={}&provides=themes&max_schema_version=1",
                encode(query)
            ),
            MAX_INDEX,
        )?)?,
        Store::OpenVsx => parse_open_vsx_search(&http_get(
            &format!(
                "{OPEN_VSX_SEARCH}?query={}&category=Themes&size=30",
                encode(query)
            ),
            MAX_INDEX,
        )?)?,
    };
    items.sort_by_key(|i| std::cmp::Reverse(i.downloads));
    Ok(items)
}

/// Theme names declared by a Zed theme family file.
fn zed_theme_names(json: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("themes")?.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|t| Some(t.get("name")?.as_str()?.to_string()))
        .collect()
}

/// Extract `themes/*.json` of a Zed extension archive into `out_dir`
/// (files named `<id>--<file>.json`); returns the theme names.
pub fn install_zed_archive(archive: &[u8], id: &str, out_dir: &Path) -> Result<Vec<String>> {
    if !valid_zed_id(id) {
        return Err(anyhow!("invalid Zed extension id {id:?}"));
    }
    let mut reader = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    let mut written: Vec<(PathBuf, String)> = Vec::new();
    for entry in reader.entries().context("not a tar.gz archive")? {
        let mut entry = entry?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry
            .path()?
            .to_string_lossy()
            .trim_start_matches("./")
            .to_string();
        let Some(file) = path.strip_prefix("themes/") else {
            continue;
        };
        if file.contains('/') || !file.ends_with(".json") || file.starts_with('.') {
            continue;
        }
        let mut text = String::new();
        entry
            .by_ref()
            .take(MAX_THEME_FILE + 1)
            .read_to_string(&mut text)?;
        if text.len() as u64 > MAX_THEME_FILE {
            return Err(anyhow!("theme file {file} is too large"));
        }
        if zed_theme_names(&text).is_empty() {
            continue; // not a theme family
        }
        written.push((out_dir.join(format!("{id}--{file}")), text));
    }
    if written.is_empty() {
        return Err(anyhow!("extension {id:?} contains no Zed themes"));
    }
    std::fs::create_dir_all(out_dir)?;
    let mut names = Vec::new();
    for (path, text) in &written {
        let mut tmp = tempfile::NamedTempFile::new_in(out_dir)?;
        use std::io::Write;
        tmp.write_all(text.as_bytes())?;
        if let Ok(meta) = std::fs::metadata(path) {
            tmp.as_file().set_permissions(meta.permissions())?;
        }
        tmp.persist(path).map_err(|e| e.error)?;
        names.extend(zed_theme_names(text));
    }
    Ok(names)
}

/// Whether editor `app` (`vscode`, `cursor`, `zed`) is installed here — its
/// binary on PATH or its Flatpak. Themes for an editor that is not installed
/// are neither downloaded nor reported.
pub fn editor_installed(app: &str) -> bool {
    let (bins, flatpaks): (&[&str], &[&str]) = match app {
        "cursor" => (&["cursor"], &[]),
        "zed" => (&["zeditor", "zed", "zedit"], &["dev.zed.Zed"]),
        _ => (
            &["code", "code-oss", "codium", "vscodium"],
            &["com.visualstudio.code", "com.vscodium.codium"],
        ),
    };
    let home = crate::paths::home_dir();
    bins.iter().any(|b| which::which(b).is_ok())
        || flatpaks.iter().any(|id| {
            std::path::Path::new("/var/lib/flatpak/app")
                .join(id)
                .is_dir()
                || home.join(".local/share/flatpak/app").join(id).is_dir()
        })
}

/// The editor a theme field / dependency kind belongs to.
pub fn editor_of(field_or_kind: &str) -> Option<&'static str> {
    match field_or_kind {
        "vscode" | "vscode_theme" => Some("vscode"),
        "cursor" | "cursor_ide_theme" => Some("cursor"),
        "zed" | "zed_dark" | "zed_light" | "zed_mode" => Some("zed"),
        _ => None,
    }
}

/// Editor CLI binary for an Open VSX app.
fn editor_cli(app: &str) -> Option<String> {
    let candidates: &[&str] = match app {
        "cursor" => &["cursor"],
        _ => &["code", "code-oss", "codium", "vscodium"],
    };
    candidates
        .iter()
        .find_map(|b| which::which(b).ok())
        .map(|p| p.to_string_lossy().to_string())
}

/// Themes contributed by an installed VS Code-style extension (`id` or label).
pub(crate) fn installed_vsx_themes(app: &str, id: &str) -> (Vec<String>, String) {
    let home = crate::paths::home_dir();
    let roots: Vec<PathBuf> = match app {
        "cursor" => vec![home.join(".cursor/extensions")],
        _ => vec![
            home.join(".vscode/extensions"),
            home.join(".vscode-oss/extensions"),
        ],
    };
    let prefix = format!("{}-", id.to_lowercase());
    for root in roots {
        let Ok(dirs) = std::fs::read_dir(&root) else {
            continue;
        };
        let mut matches: Vec<PathBuf> = dirs
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().to_lowercase().starts_with(&prefix))
            })
            .collect();
        matches.sort();
        if let Some(dir) = matches.pop() {
            let themes = std::fs::read_to_string(dir.join("package.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .and_then(|v| v.get("contributes")?.get("themes")?.as_array().cloned())
                .unwrap_or_default()
                .iter()
                .filter_map(|t| {
                    let label = t.get("label").and_then(|l| l.as_str());
                    t.get("id")
                        .and_then(|i| i.as_str())
                        .or(label)
                        .map(str::to_string)
                })
                .collect();
            return (themes, dir.display().to_string());
        }
    }
    (Vec::new(), String::new())
}

/// Install a theme extension for `app` (see module docs).
pub fn install(app: &str, id: &str) -> Result<InstallReport> {
    if std::env::var("KTM_THEME_NO_APPLY").as_deref() == Ok("1") {
        return Err(anyhow!("installation disabled by KTM_THEME_NO_APPLY=1"));
    }
    let d = crate::deps::Dependency {
        kind: app.into(),
        name: id.into(),
        id: id.into(),
        source: if app == "zed" { "zed" } else { "open-vsx" }.into(),
        exact: true,
        ..Default::default()
    };
    let _guard = crate::dependency_registry::lock(&crate::dependency_registry::resource_id(&d))?;
    let before = crate::dependency_registry::inventory(&d);
    let result = install_unrecorded(app, id);
    let recorded = result
        .as_ref()
        .map(|r| format!("Installed {}", r.id))
        .map_err(|e| anyhow!("{e:#}"));
    let mut d = d;
    if let Ok(report) = &result {
        if let Some(name) = report.themes.first() {
            d.name = name.clone();
        }
    }
    crate::dependency_registry::record_result(&d, &before, &recorded)?;
    result
}

pub(crate) fn install_unrecorded(app: &str, id: &str) -> Result<InstallReport> {
    if std::env::var("KTM_THEME_NO_APPLY").as_deref() == Ok("1") {
        return Err(anyhow!("installation disabled by KTM_THEME_NO_APPLY=1"));
    }
    match store_for(app)? {
        Store::Zed => {
            if !valid_zed_id(id) {
                return Err(anyhow!("invalid Zed extension id {id:?}"));
            }
            let archive = http_get_bytes(
                &format!(
                    "{ZED_DOWNLOAD}/{id}/download?min_schema_version=0&max_schema_version=1\
                     &min_wasm_api_version=0.0.0&max_wasm_api_version=0.9.0"
                ),
                MAX_ARCHIVE,
            )?;
            let out = crate::paths::config_home().join("zed").join("themes");
            let themes = install_zed_archive(&archive, id, &out)?;
            Ok(InstallReport {
                app: app.into(),
                id: id.into(),
                themes,
                location: out.display().to_string(),
            })
        }
        Store::OpenVsx => {
            if !valid_vsx_id(id) {
                return Err(anyhow!("invalid extension id {id:?} (publisher.name)"));
            }
            crate::net::network_enabled()
                .then_some(())
                .ok_or_else(|| anyhow!("network is disabled"))?;
            let cli =
                editor_cli(app).ok_or_else(|| anyhow!("{app} is not installed (no CLI found)"))?;
            if std::env::var("KTM_THEME_NO_APPLY").as_deref() != Ok("1") {
                let (status, _) = crate::paths::output_bounded_for(
                    std::process::Command::new(&cli)
                        .args(["--install-extension", id, "--force"])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null()),
                    std::time::Duration::from_secs(180),
                )
                .ok_or_else(|| anyhow!("{cli} did not install {id} in time"))?;
                if !status.success() {
                    return Err(anyhow!("{cli} --install-extension {id} failed"));
                }
            }
            let (themes, location) = installed_vsx_themes(app, id);
            if themes.is_empty() || location.is_empty() {
                return Err(anyhow!(
                    "{app} reported success but extension {id} was not found"
                ));
            }
            Ok(InstallReport {
                app: app.into(),
                id: id.into(),
                themes,
                location,
            })
        }
    }
}

pub(crate) fn uninstall(app: &str, id: &str) -> Result<()> {
    if !valid_vsx_id(id) {
        return Err(anyhow!("invalid extension id"));
    }
    let cli = editor_cli(app).ok_or_else(|| anyhow!("editor CLI not found"))?;
    let (status, _) = crate::paths::output_bounded_for(
        std::process::Command::new(cli).args(["--uninstall-extension", id]),
        std::time::Duration::from_secs(180),
    )
    .ok_or_else(|| anyhow!("extension removal timed out"))?;
    if !status.success() {
        return Err(anyhow!("editor could not remove extension {id}"));
    }
    Ok(())
}

/// Directory name for cached editor themes inside a theme package:
/// `assets/editor-themes/<editor>/<store-id>/`.
pub fn editor_cache_subdir(editor: &str, store_id: &str) -> PathBuf {
    PathBuf::from(crate::theme::EDITOR_THEMES_DIR)
        .join(editor)
        .join(store_id)
}

/// Download a VS Code / Open VSX extension (.vsix) to `dst_dir`, returning
/// the path to the downloaded file. The URL is the Open VSX latest download
/// endpoint (`https://open-vsx.org/api/{namespace}/{name}/latest`) or a
/// direct VS Code Marketplace vspackage URL.
pub fn download_vsix(url: &str, dst_dir: &Path) -> Result<PathBuf> {
    // Open VSX's latest endpoint is metadata, not a VSIX archive.
    let url = if url.starts_with("https://open-vsx.org/api/") && url.ends_with("/latest") {
        let metadata: serde_json::Value = serde_json::from_str(&http_get(url, MAX_INDEX)?)?;
        metadata["files"]["download"]
            .as_str()
            .filter(|url| url.starts_with("https://"))
            .context("Open VSX response has no HTTPS download URL")?
            .to_string()
    } else {
        url.to_string()
    };
    anyhow::ensure!(
        url.starts_with("https://"),
        "editor theme URL must use HTTPS"
    );
    let bytes = http_get_bytes(&url, MAX_ARCHIVE)?;
    std::fs::create_dir_all(dst_dir)?;
    let dst = dst_dir.join("extension.vsix");
    crate::gui_api::write_bytes(&dst, &bytes, None)?;
    Ok(dst)
}

/// Extract into a fresh staging directory; preserve nested names and cap the
/// uncompressed size as well as the download. Existing paths/links are refused.
pub fn extract_vsix(archive: &Path, dst_dir: &Path) -> Result<PathBuf> {
    let mut zip = ZipArchive::new(std::fs::File::open(archive)?)?;
    let mut total = 0u64;
    anyhow::ensure!(zip.len() <= 10000, "too many VSIX entries");
    std::fs::create_dir_all(dst_dir)?;
    anyhow::ensure!(
        std::fs::read_dir(dst_dir)?.next().is_none(),
        "VSIX extraction needs an empty directory"
    );
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let rel = entry.enclosed_name().context("invalid path in VSIX")?;
        anyhow::ensure!(
            !entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000),
            "symlink in VSIX"
        );
        total = total
            .checked_add(entry.size())
            .context("VSIX size overflow")?;
        anyhow::ensure!(total <= MAX_ARCHIVE, "uncompressed VSIX exceeds size limit");
        let target = dst_dir.join(&rel);
        for ancestor in target.ancestors().take_while(|p| *p != dst_dir) {
            anyhow::ensure!(!ancestor.is_symlink(), "symlink in VSIX destination");
        }
        if entry.is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            std::fs::create_dir_all(target.parent().context("VSIX parent")?)?;
            let mut out = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)?;
            let copied = std::io::copy(&mut (&mut entry).take(MAX_ARCHIVE + 1), &mut out)?;
            anyhow::ensure!(copied <= MAX_ARCHIVE, "VSIX file exceeds size limit");
        }
    }
    [
        dst_dir.join("extension"),
        dst_dir.join("package"),
        dst_dir.to_path_buf(),
    ]
    .into_iter()
    .find(|p| p.join("package.json").is_file())
    .context("VSIX has no extension package.json")
}

/// Patch metadata without copying a file onto itself (which truncates it).
pub fn create_native_extension(
    src_dir: &Path,
    ext_dir: &Path,
    publisher: &str,
    name: &str,
) -> Result<PathBuf> {
    let mut plan = crate::change_plan::Plan::default();
    if src_dir != ext_dir {
        plan.copy(src_dir, ext_dir, "editor_theme")?;
    }
    let path = ext_dir.join("package.json");
    let text = plan.text(&path)?.unwrap_or_default();
    let mut pkg: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::json!({}));
    let obj = pkg
        .as_object_mut()
        .context("package.json is not an object")?;
    obj.insert("_watch".into(), true.into());
    obj.insert("publisher".into(), publisher.into());
    obj.insert("name".into(), name.into());
    plan.write_text(&path, serde_json::to_string_pretty(&pkg)?, "editor_theme")?;
    plan.commit()?;
    Ok(ext_dir.to_path_buf())
}

pub(crate) fn extension_root(editor: &str) -> Result<PathBuf> {
    let home = crate::paths::home_dir();
    Ok(home.join(match editor {
        "vscode" => ".vscode/extensions",
        "vscodium" => ".vscode-oss/extensions",
        "cursor" => ".cursor/extensions",
        "windsurf" => ".windsurf/extensions",
        other => anyhow::bail!("no extension directory for {other:?}"),
    }))
}

/// Plan a complete extension in its own namespace, never delete an installed
/// extension or copy its internal folders directly into extensions/.
fn strict_destination(path: &Path, container: &Path) -> Result<()> {
    for current in path.ancestors().take_while(|p| *p != container) {
        anyhow::ensure!(
            !current.is_symlink(),
            "symlink in editor destination: {}",
            current.display()
        );
    }
    Ok(())
}

pub(crate) fn plan_from_cache(
    editor: &str,
    id: &str,
    cache: &Path,
    watch: bool,
    plan: &mut crate::change_plan::Plan,
) -> Result<Vec<String>> {
    if editor == "zed" {
        let out = crate::paths::config_home().join("zed/themes");
        let mut files = Vec::new();
        let input = if cache.join("themes").is_dir() {
            cache.join("themes")
        } else {
            cache.to_path_buf()
        };
        for entry in std::fs::read_dir(input)? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|e| e == "json") {
                let target = out.join(entry.file_name());
                strict_destination(&target, &out)?;
                plan.copy(&entry.path(), &target, "editor_theme")?;
                files.push(target.display().to_string());
            }
        }
        anyhow::ensure!(!files.is_empty(), "Zed cache has no theme JSON files");
        return Ok(files);
    }
    anyhow::ensure!(valid_vsx_id(id), "invalid extension id");
    let root = extension_root(editor)?;
    let owned_id = if id.starts_with("kolira-cache.") {
        id.to_string()
    } else {
        let (publisher, name) = id.split_once('.').context("extension id")?;
        format!("kolira-cache.{publisher}-{name}")
    };
    let target = root.join(format!("{owned_id}-kolira"));
    strict_destination(&target, &root)?;
    let mut pkg: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(cache.join("package.json"))?)?;
    let obj = pkg
        .as_object_mut()
        .context("extension manifest must be an object")?;
    let (publisher, name) = id.split_once('.').context("extension id")?;
    anyhow::ensure!(
        obj.get("publisher").and_then(|x| x.as_str()) == Some(publisher)
            && obj.get("name").and_then(|x| x.as_str()) == Some(name),
        "extension manifest id mismatch"
    );
    anyhow::ensure!(
        obj.get("contributes")
            .and_then(|c| c.get("themes"))
            .and_then(|t| t.as_array())
            .is_some_and(|t| !t.is_empty()),
        "extension has no theme contributions"
    );
    // Theme packages cannot activate an extension host or replace editor
    // behaviour. Only theme contributions are installed from bundled archives.
    let themes = obj["contributes"]["themes"].clone();
    for theme in themes.as_array().context("theme contributions")? {
        let path = theme["path"].as_str().context("theme path")?;
        anyhow::ensure!(
            !path.contains('\\')
                && !Path::new(path).is_absolute()
                && Path::new(path).components().all(|c| matches!(
                    c,
                    std::path::Component::Normal(_) | std::path::Component::CurDir
                )),
            "theme contribution escapes extension"
        );
    }
    obj.remove("main");
    obj.remove("browser");
    obj.remove("activationEvents");
    obj.remove("extensionDependencies");
    obj.remove("extensionPack");
    obj.insert("contributes".into(), serde_json::json!({"themes":themes}));
    let (_, owned_name) = owned_id.split_once('.').context("owned extension id")?;
    obj.insert("publisher".into(), "kolira-cache".into());
    obj.insert("name".into(), owned_name.into());
    if watch {
        obj.insert("_watch".into(), true.into());
        if let Some(themes) = obj
            .get_mut("contributes")
            .and_then(|v| v.get_mut("themes"))
            .and_then(|v| v.as_array_mut())
        {
            for theme in themes {
                theme["_watch"] = true.into();
            }
        }
    }
    fn check_targets(src: &Path, dst: &Path, container: &Path) -> Result<()> {
        strict_destination(dst, container)?;
        if src.is_dir() {
            for entry in std::fs::read_dir(src)? {
                let entry = entry?;
                check_targets(&entry.path(), &dst.join(entry.file_name()), container)?;
            }
        }
        Ok(())
    }
    check_targets(cache, &target, &root)?;
    plan.copy(cache, &target, "editor_theme")?;
    plan.write_text(
        &target.join("package.json"),
        serde_json::to_string_pretty(&pkg)?,
        "editor_theme",
    )?;
    let registry = root.join("extensions.json");
    strict_destination(&registry, &root)?;
    let mut entries: Vec<serde_json::Value> = match plan.text(&registry)? {
        Some(text) => serde_json::from_str(&text)
            .context("Invalid editor extensions.json; refusing to overwrite")?,
        None => Vec::new(),
    };
    let version = pkg
        .get("version")
        .and_then(|v| v.as_str())
        .context("Extension version missing")?;
    entries.retain(|entry| {
        entry["relativeLocation"].as_str() != target.file_name().and_then(|s| s.to_str())
    });
    entries.push(serde_json::json!({
        "identifier":{"id":owned_id},"version":version,
        "location":{"$mid":1,"scheme":"file","path":target,"fsPath":target,"external":format!("file://{}",target.display())},
        "relativeLocation":target.file_name().and_then(|s|s.to_str()).context("Extension folder")?,
    }));
    plan.write_text(
        &registry,
        serde_json::to_string_pretty(&entries)?,
        "editor_registry",
    )?;
    Ok(vec![target.display().to_string()])
}

/// The Open VSX metadata endpoint for an extension `publisher.name`.
pub fn open_vsx_download_url(id: &str) -> String {
    let Some((ns, name)) = id.split_once('.') else {
        return String::new();
    };
    format!("https://open-vsx.org/api/{ns}/{name}/latest")
}

/// The VS Code Marketplace download URL for a publisher/name.
pub fn vscode_marketplace_download_url(id: &str) -> String {
    let Some((publisher, name)) = id.split_once('.') else {
        return String::new();
    };
    format!(
        "https://marketplace.visualstudio.com/_apis/public/gallery/publishers/{publisher}/vsextensions/{name}/latest/vspackage"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_validated() {
        assert!(valid_zed_id("everforest") && valid_zed_id("catppuccin-icons"));
        assert!(!valid_zed_id("../x") && !valid_zed_id("A") && !valid_zed_id(""));
        assert!(valid_vsx_id("sainnhe.everforest") && valid_vsx_id("jdinhlife.gruvbox"));
        for bad in ["noperiod", ".x", "a.", "a/b.c", "a.b.c/d", "a b.c"] {
            assert!(!valid_vsx_id(bad), "{bad}");
        }
        assert_eq!(encode("rosé pine"), "ros%C3%A9%20pine");
    }

    #[test]
    fn search_responses_are_parsed() {
        let zed = r#"{"data":[
            {"id":"everforest","name":"Everforest Theme","version":"0.2.0","provides":["themes"],"download_count":118839,"description":"d"},
            {"id":"html","name":"HTML","provides":["languages"]},
            {"id":"../evil","name":"x","provides":["themes"]}]}"#;
        let items = parse_zed_search(zed).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "everforest");
        assert_eq!(items[0].downloads, 118_839);

        let vsx = r#"{"extensions":[{"namespace":"sainnhe","name":"everforest","displayName":"Everforest",
            "version":"0.3.0","downloadCount":5},{"namespace":"bad/ns","name":"x"}]}"#;
        let items = parse_open_vsx_search(vsx).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "sainnhe.everforest");
        assert!(parse_open_vsx_search("{}").is_err());
    }

    fn tar_gz(files: &[(&str, &str)]) -> Vec<u8> {
        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::default(),
        ));
        for (path, text) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(text.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, path, text.as_bytes())
                .unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn zed_archive_extracts_only_theme_families() {
        let family = r#"{"name":"Everforest","themes":[{"name":"Everforest Dark"},{"name":"Everforest Light"}]}"#;
        let archive = tar_gz(&[
            ("./extension.toml", "id = \"everforest\""),
            ("./themes/everforest.json", family),
            ("./themes/notes.json", "{\"x\":1}"),
            ("./themes/sub/deep.json", family),
            ("./languages/x.json", family),
        ]);
        let tmp = tempfile::tempdir().unwrap();
        let names = install_zed_archive(&archive, "everforest", tmp.path()).unwrap();
        assert_eq!(names, vec!["Everforest Dark", "Everforest Light"]);
        let files: Vec<String> = std::fs::read_dir(tmp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(files, vec!["everforest--everforest.json"]);

        assert!(
            install_zed_archive(&tar_gz(&[("./themes/x.json", "{}")]), "x", tmp.path()).is_err()
        );
        assert!(install_zed_archive(b"not an archive", "x", tmp.path()).is_err());
        assert!(install_zed_archive(&archive, "../x", tmp.path()).is_err());
    }

    #[test]
    fn unknown_app_has_no_store() {
        assert!(search("btop", "x").is_err());
        assert!(install("kitty", "x").is_err());
    }
}
