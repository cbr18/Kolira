//! Export / import of themes: a portable copy of a theme package.
//!
//! An export holds the theme's `theme.json`, its manifests, the wallpaper
//! (optional) and every tracked appearance file / local asset — not the
//! diagnostics in `metadata/` nor files the theme no longer tracks. It is
//! made portable on the way out:
//!
//! * the home directory in text files becomes the literal `$HOME` (import
//!   expands it to the receiver's home; KDE does not expand `$HOME` in
//!   appletsrc and friends, so this must be done by us);
//! * desktop-icon positions are dropped from appletsrc (not part of the look,
//!   and they list the user's desktop files);
//! * `manifest.txt` loses its `user=` line.
//!
//! `export.json` lists every file with its SHA-256. Import accepts a `.zip`
//! or an unpacked directory (the themes in the repository), refuses unsafe
//! paths (absolute, `..`, symlinks), unlisted or modified files and oversized
//! archives, never overwrites an existing theme without `force` (the old one
//! goes to `backups/pre-import-<name>-…`), and reports what the theme needs
//! but this machine lacks.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::paths::{backups_dir, home_dir, profile_dir, validate_profile_name};
use crate::profile::{case_duplicate_of, keep_desktop_icons, resolve_cli_paths, unique_dir};
use crate::theme::{self, Missing};
use crate::wallpaper::APPLETSRC_REL;

pub const FORMAT: &str = "kolira/export@1";
pub const EXPORT_MANIFEST: &str = "export.json";
pub const HOME_TOKEN: &str = "$HOME";
const MAX_FILES: usize = 20_000;

/// Directories that collect many themes (every installed colour scheme, all
/// Kvantum families, …). Locally the whole directory is tracked; an export
/// carries only the entries the theme uses (its `assets`) — the rest are
/// other people's themes the receiver neither needs nor may get from us.
const LIBRARY_ROOTS: &[&str] = &[
    ".local/share/icons",
    ".local/share/color-schemes",
    ".config/Kvantum",
    ".local/share/plasma/look-and-feel",
    ".local/share/plasma/desktoptheme",
    ".local/share/aurorae/themes",
    ".config/btop/themes",
    ".config/zed/themes",
];

/// Keep `rel` in an export? Files directly under `.config/Kvantum` (the
/// `kvantum.kvconfig` selector) always; other library entries only when they
/// belong to one of the theme's assets.
fn exported(rel: &Path, asset_paths: &[PathBuf]) -> bool {
    let Some(root) = LIBRARY_ROOTS
        .iter()
        .map(Path::new)
        .find(|r| rel.starts_with(r))
    else {
        return true;
    };
    if root == Path::new(".config/Kvantum") && rel.parent() == Some(root) {
        return true;
    }
    asset_paths.iter().any(|a| rel.starts_with(a))
}
const MAX_BYTES: u64 = 1 << 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportManifest {
    pub format: String,
    pub theme: String,
    pub exported: String,
    pub generator: String,
    pub home_token: String,
    /// path → sha256 (hex) of the file as stored in the export.
    pub files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct ExportOptions {
    pub include_wallpaper: bool,
    /// For publishing (the themes in the repository): ship only assets marked
    /// redistributable (`kolira license`), turn the others into
    /// dependencies with their source, drop Look & Feel preview screenshots.
    pub public: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    /// Install under this name instead of the one in theme.json.
    pub name: Option<String>,
    /// Replace an existing theme of that name (backed up first).
    pub force: bool,
}

#[derive(Debug, Serialize)]
pub struct ImportReport {
    pub name: String,
    pub files: usize,
    pub replaced: Option<String>,
    pub missing: Vec<Missing>,
    /// Dependencies not installed here, offered for installation.
    pub dependencies: Vec<theme::Needed>,
}

// ─────────────────────────────────────────────────────────────────────────────
// $HOME
// ─────────────────────────────────────────────────────────────────────────────

fn path_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.')
}

/// Replace `needle` with `with` where it is a whole path prefix (the next
/// character does not continue a name: `/home/cbr18` is not `/home/cbr`).
fn replace_prefix(text: &str, needle: &str, with: &str) -> String {
    if needle.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(needle) {
        let after = &rest[at + needle.len()..];
        out.push_str(&rest[..at]);
        if after.chars().next().is_some_and(path_char) {
            out.push_str(needle);
        } else {
            out.push_str(with);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

pub fn home_to_token(text: &str, home: &str) -> String {
    replace_prefix(text, home.trim_end_matches('/'), HOME_TOKEN)
}

/// Expand `$HOME` back to this machine's home — except in KConfig entries
/// marked `[$e]`, where KDE itself writes and expands a literal `$HOME`
/// (e.g. `SteamLibraryPath[$e]=file:$HOME/…` in appletsrc).
pub fn token_to_home(text: &str, home: &str) -> String {
    let home = home.trim_end_matches('/');
    let mut out: Vec<String> = text
        .split('\n')
        .map(|line| {
            if line.contains("[$e]=") {
                line.to_string()
            } else {
                replace_prefix(line, HOME_TOKEN, home)
            }
        })
        .collect();
    if out.last().is_some_and(String::is_empty) && !text.ends_with('\n') {
        out.pop();
    }
    out.join("\n")
}

fn is_text(bytes: &[u8]) -> bool {
    !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok()
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Export
// ─────────────────────────────────────────────────────────────────────────────

fn walk(root: &Path, rel: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let path = root.join(rel);
    let meta = fs::symlink_metadata(&path)?;
    if meta.file_type().is_symlink() {
        return Ok(()); // never follow links out of the theme
    }
    if meta.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(&path)?
            .flatten()
            .map(|e| e.file_name())
            .collect();
        entries.sort();
        for name in entries {
            walk(root, &rel.join(name), out)?;
        }
    } else if meta.is_file() && !out.contains(&rel.to_path_buf()) {
        out.push(rel.to_path_buf());
    }
    Ok(())
}

/// The files of theme `dir` that belong in an export, `(relative path, bytes)`.
/// The files that make up theme `dir`: manifests, tracked appearance files,
/// its own assets (only those from library folders) and, optionally, its
/// wallpaper. Shared by export and by pruning stored themes.
pub(crate) fn theme_files(dir: &Path, include_wallpaper: bool) -> Result<Vec<PathBuf>> {
    theme_files_for(dir, include_wallpaper, false)
}

/// Assets `export --public` leaves out (not marked redistributable).
fn withheld(package: &theme::Package) -> Vec<&theme::Asset> {
    package
        .assets
        .iter()
        .filter(|a| !a.redistributable)
        .collect()
}

fn theme_files_for(dir: &Path, include_wallpaper: bool, public: bool) -> Result<Vec<PathBuf>> {
    let package = theme::load(dir)?;
    let mut rels: Vec<PathBuf> = Vec::new();
    for rel in ["manifest.json", "manifest.txt", "LICENSE"] {
        if dir.join(rel).is_file() {
            rels.push(PathBuf::from(rel));
        }
    }
    let mut roots: Vec<String> = resolve_cli_paths(None)?;
    roots.extend(
        package
            .assets
            .iter()
            .filter(|a| a.kind != "wallpaper")
            .map(|a| a.path.clone()),
    );
    // The exporting machine may use another default profile (or lack Konsole).
    // Keep the snapshot's appearance profile so the receiver can map it to its own.
    let profile = crate::profile::theme_file(dir, ".local/share/konsole/Theme Profile.profile");
    if profile.is_file() {
        roots.push(profile.strip_prefix(dir)?.to_string_lossy().into_owned());
    }
    if include_wallpaper && package.wallpaper.is_some() {
        roots.push("wallpaper".into());
    }
    for rel in roots {
        let rel = PathBuf::from(&rel);
        if safe_rel(&rel).is_err() || !dir.join(&rel).exists() {
            continue;
        }
        walk(dir, &rel, &mut rels)?;
    }

    let asset_paths: Vec<PathBuf> = package
        .assets
        .iter()
        .map(|a| PathBuf::from(&a.path))
        .collect();
    rels.retain(|rel| exported(rel, &asset_paths));
    rels.retain(|rel| !crate::paths::is_agent_metadata(rel));
    // Look & Feel layout scripts would bring the author's panels (layout.rs).
    rels.retain(|rel| {
        !crate::layout::is_laf_layout(rel)
            && !crate::appearance::is_preserved(&rel.to_string_lossy())
    });
    let unused = crate::appearance::unused_files_in(dir);
    rels.retain(|rel| !unused.iter().any(|u| rel == Path::new(u)));
    if public {
        let held: Vec<PathBuf> = withheld(&package)
            .iter()
            .map(|a| PathBuf::from(&a.path))
            .collect();
        rels.retain(|rel| !held.iter().any(|h| rel.starts_with(h)));
        // Look & Feel previews are screenshots of the author's desktop.
        rels.retain(|rel| !rel.to_string_lossy().contains("/contents/previews/"));
    }
    Ok(rels)
}

fn export_files(name: &str, dir: &Path, options: &ExportOptions) -> Result<Vec<(String, Vec<u8>)>> {
    let package = theme::load(dir)?;
    let wallpaper_ok = !options.public
        || package
            .assets
            .iter()
            .any(|a| a.kind == "wallpaper" && a.redistributable);
    let include_wallpaper = options.include_wallpaper && wallpaper_ok;
    let rels = theme_files_for(dir, include_wallpaper, options.public)?;

    // Only the look leaves the machine, even from themes saved before the
    // appearance filters existed (appearance.rs).
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for rel in &rels {
        let key = rel.to_string_lossy().to_string();
        let bytes = fs::read(dir.join(rel)).with_context(|| format!("read {key}"))?;
        match String::from_utf8(bytes.clone()) {
            Ok(text)
                if !matches!(
                    crate::appearance::policy(&key),
                    crate::appearance::Policy::Whole
                ) =>
            {
                let has_include = crate::appearance::include_of(&key)
                    .is_some_and(|inc| rels.iter().any(|r| r == Path::new(inc)));
                for (k, t) in crate::appearance::normalize(&key, &text, has_include)? {
                    entries.push((k, t.into_bytes()));
                }
            }
            _ => entries.push((key, bytes)),
        }
    }

    let home = home_dir().to_string_lossy().to_string();
    let mut files = Vec::new();
    for (key, mut bytes) in entries {
        if is_text(&bytes) {
            let mut text = String::from_utf8(bytes).unwrap_or_default();
            if key == APPLETSRC_REL {
                text = keep_desktop_icons(&text, "");
                if !include_wallpaper {
                    text = crate::layout::without_wallpaper(&text);
                }
            }
            if key == "manifest.txt" {
                text = text
                    .lines()
                    .filter(|l| !l.starts_with("user="))
                    .collect::<Vec<_>>()
                    .join("\n")
                    + "\n";
            }
            bytes = home_to_token(&text, &home).into_bytes();
        }
        files.push((key, bytes));
    }

    // theme.json last, adjusted: name, and no wallpaper when it is left out.
    let mut json: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join(theme::MANIFEST))?)?;
    json["name"] = serde_json::Value::from(name);
    json["assets"] = serde_json::to_value(&package.assets)?;
    json["dependencies"] = serde_json::to_value(&package.dependencies)?;
    if options.public {
        let held = withheld(&package);
        let mut deps: Vec<crate::deps::Dependency> = package.dependencies.clone();
        for a in &held {
            if let Some(d) = crate::deps::for_asset(&a.kind, &a.name, &a.path) {
                if !deps.iter().any(|x| x.kind == d.kind && x.name == d.name) {
                    deps.push(d);
                }
            }
        }
        if let Some(assets) = json.get_mut("assets").and_then(|a| a.as_array_mut()) {
            assets.retain(|a| a.get("redistributable").and_then(|r| r.as_bool()) == Some(true));
        }
        json["dependencies"] = serde_json::to_value(&deps)?;
    }
    if !include_wallpaper {
        json["wallpaper"] = serde_json::Value::Null;
        if let Some(assets) = json.get_mut("assets").and_then(|a| a.as_array_mut()) {
            assets.retain(|a| a.get("kind").and_then(|k| k.as_str()) != Some("wallpaper"));
        }
    }
    let text = serde_json::to_string_pretty(&json)? + "\n";
    files.push((
        theme::MANIFEST.into(),
        home_to_token(&text, &home).into_bytes(),
    ));
    Ok(files)
}

fn manifest_for(name: &str, files: &[(String, Vec<u8>)]) -> ExportManifest {
    ExportManifest {
        format: FORMAT.into(),
        theme: name.into(),
        exported: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        generator: format!("kolira {}", env!("CARGO_PKG_VERSION")),
        home_token: HOME_TOKEN.into(),
        files: files
            .iter()
            .map(|(k, b)| (k.clone(), sha256_hex(b)))
            .collect(),
    }
}

/// Export theme `name` as a `.zip` at `out`. Returns the number of files.
pub fn export_zip(name: &str, out: &Path, options: &ExportOptions) -> Result<usize> {
    validate_profile_name(name)?;
    let dir = profile_dir(name)?;
    if !theme::is_package(&dir) {
        bail!("{name} is not a theme package (run `kolira migrate {name}` first)");
    }
    let files = export_files(name, &dir, options)?;
    let manifest = manifest_for(name, &files);

    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let tmp = out.with_extension("zip.part");
    let result = (|| -> Result<()> {
        let mut zip = zip::ZipWriter::new(fs::File::create(&tmp)?);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);
        zip.start_file(EXPORT_MANIFEST, opts)?;
        zip.write_all((serde_json::to_string_pretty(&manifest)? + "\n").as_bytes())?;
        for (key, bytes) in &files {
            zip.start_file(key.as_str(), opts)?;
            zip.write_all(bytes)?;
        }
        zip.finish()?;
        Ok(())
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    fs::rename(&tmp, out)?;
    Ok(files.len())
}

/// Export theme `name` unpacked into directory `out` (themes in the
/// repository). `out` must not exist or be empty.
pub fn export_dir(name: &str, out: &Path, options: &ExportOptions) -> Result<usize> {
    validate_profile_name(name)?;
    let dir = profile_dir(name)?;
    if !theme::is_package(&dir) {
        bail!("{name} is not a theme package (run `kolira migrate {name}` first)");
    }
    if out.exists() && fs::read_dir(out)?.next().is_some() {
        bail!("{} is not empty", out.display());
    }
    let files = export_files(name, &dir, options)?;
    let manifest = manifest_for(name, &files);
    for (key, bytes) in &files {
        let dst = out.join(key);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dst, bytes)?;
    }
    fs::create_dir_all(out)?;
    fs::write(
        out.join(EXPORT_MANIFEST),
        serde_json::to_string_pretty(&manifest)? + "\n",
    )?;
    Ok(files.len())
}

// ─────────────────────────────────────────────────────────────────────────────
// Import
// ─────────────────────────────────────────────────────────────────────────────

/// A relative path made only of normal components (no `..`, root, prefix).
fn safe_rel(rel: &Path) -> Result<()> {
    if rel.as_os_str().is_empty() || rel.as_os_str().len() > 4096 {
        bail!("bad path in theme: {}", rel.display());
    }
    if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
        bail!("unsafe path in theme: {}", rel.display());
    }
    Ok(())
}

/// All files of an export (zip or directory) as `(path, bytes)`, with limits.
fn read_export(src: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut total: u64 = 0;
    let mut add = |key: String, bytes: Vec<u8>, total: &mut u64| -> Result<()> {
        safe_rel(Path::new(&key))?;
        *total += bytes.len() as u64;
        if files.len() >= MAX_FILES || *total > MAX_BYTES {
            bail!(
                "theme archive too large (limit {MAX_FILES} files / {} MiB)",
                MAX_BYTES >> 20
            );
        }
        files.push((key, bytes));
        Ok(())
    };

    if src.is_dir() {
        let mut rels = Vec::new();
        for entry in fs::read_dir(src)?.flatten() {
            let rel = PathBuf::from(entry.file_name());
            if fs::symlink_metadata(entry.path())?.file_type().is_symlink() {
                bail!("symlink in theme: {}", rel.display());
            }
            walk(src, &rel, &mut rels)?;
        }
        for rel in rels {
            let bytes = fs::read(src.join(&rel))?;
            add(rel.to_string_lossy().to_string(), bytes, &mut total)?;
        }
        return Ok(files);
    }

    let mut zip = zip::ZipArchive::new(
        fs::File::open(src).with_context(|| format!("open {}", src.display()))?,
    )
    .context("not a zip archive")?;
    if zip.len() > MAX_FILES + 1 {
        bail!("theme archive has too many entries");
    }
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        if entry.is_symlink() {
            bail!("symlink in theme archive: {}", entry.name());
        }
        let name = entry.name().to_string();
        let Some(rel) = entry.enclosed_name() else {
            bail!("unsafe path in theme archive: {name}");
        };
        let mut bytes = Vec::new();
        (&mut entry).take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        add(rel.to_string_lossy().to_string(), bytes, &mut total)?;
    }
    Ok(files)
}

/// What the kit's themes looked like when last installed here:
/// `<theme root>/bundled-themes.json`, name → (kit version, installed files).
fn bundled_record_path() -> PathBuf {
    crate::paths::theme_root().join("bundled-themes.json")
}

/// Hash of every file under `dir`: changes when anything in the theme changes.
fn tree_fingerprint(dir: &Path) -> String {
    let mut files: Vec<String> = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(folder) = stack.pop() {
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(bytes) = fs::read(&path) {
                let rel = path
                    .strip_prefix(dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                files.push(format!("{rel} {}", sha256_hex(&bytes)));
            }
        }
    }
    files.sort();
    sha256_hex(files.join("\n").as_bytes())
}

/// Does the installed theme hold exactly what importing `kit` would write:
/// every exported file (with `$HOME` expanded in text) and nothing else?
fn installed_matches_kit(kit: &Path, installed: &Path, manifest: &ExportManifest) -> bool {
    let home = home_dir().to_string_lossy().into_owned();
    let mut count = 0usize;
    let mut stack = vec![installed.to_path_buf()];
    while let Some(folder) = stack.pop() {
        let Ok(entries) = fs::read_dir(&folder) else {
            return false;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                count += 1;
            }
        }
    }
    count == manifest.files.len()
        && manifest.files.keys().all(|rel| {
            let (Ok(want), Ok(have)) = (fs::read(kit.join(rel)), fs::read(installed.join(rel)))
            else {
                return false;
            };
            if have == want {
                return true;
            }
            is_text(&want)
                && std::str::from_utf8(&want)
                    .is_ok_and(|text| token_to_home(text, &home).as_bytes() == have.as_slice())
        })
}

/// Bring the kit's ready themes up to date: add the new ones, replace the ones
/// the kit changed unless the user changed them here (the replaced one is backed
/// up). A theme with no record of how it was installed is left alone, it may hold
/// the user's edits. A kept theme is mentioned once per kit version. Returns one
/// line per theme that needs saying.
pub fn sync_bundled(themes: &Path) -> Result<Vec<String>> {
    let path = bundled_record_path();
    let mut record: BTreeMap<String, (String, String)> = fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let mut dirs: Vec<PathBuf> = fs::read_dir(themes)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join(EXPORT_MANIFEST).is_file())
        .collect();
    dirs.sort();
    let mut lines = Vec::new();
    for dir in dirs {
        let manifest: ExportManifest = match fs::read(dir.join(EXPORT_MANIFEST))
            .map_err(anyhow::Error::from)
            .and_then(|b| Ok(serde_json::from_slice(&b)?))
        {
            Ok(m) => m,
            Err(e) => {
                lines.push(format!("Could not read {}: {e:#}", dir.display()));
                continue;
            }
        };
        let name = manifest.theme.clone();
        let kit = sha256_hex(serde_json::to_string(&manifest.files)?.as_bytes());
        let existing = case_duplicate_of(&name).unwrap_or_else(|| name.clone());
        let installed = profile_dir(&existing)?;
        let force = if !installed.exists() {
            false
        } else {
            match record.get(&existing) {
                Some((old_kit, _)) if *old_kit == kit => continue,
                Some((_, files)) if *files == tree_fingerprint(&installed) => true,
                // Already this kit version (e.g. after a manual `import --force`
                // of it): nothing to keep or replace, just remember it.
                _ if installed_matches_kit(&dir, &installed, &manifest) => {
                    record.insert(existing.clone(), (kit, tree_fingerprint(&installed)));
                    continue;
                }
                // Said once per kit version; an empty fingerprint never matches,
                // so a theme of unknown origin is never replaced by itself.
                other => {
                    let files = other.map(|(_, f)| f.clone()).unwrap_or_default();
                    lines.push(format!("Kept theme {existing}: it may hold your changes (new version: kolira import {} --force)", dir.display()));
                    record.insert(existing.clone(), (kit, files));
                    continue;
                }
            }
        };
        match import(
            &dir,
            &ImportOptions {
                name: Some(existing.clone()),
                force,
            },
        ) {
            Ok(_) => {
                record.insert(
                    existing.clone(),
                    (kit, tree_fingerprint(&profile_dir(&existing)?)),
                );
                lines.push(format!(
                    "{} theme: {existing}",
                    if force { "Updated" } else { "Added" }
                ));
            }
            Err(e) => lines.push(format!("Could not add theme {existing}: {e:#}")),
        }
    }
    fs::write(&path, serde_json::to_vec_pretty(&record)?)?;
    Ok(lines)
}

/// Import a theme from `src` (`.zip` or unpacked directory).
pub fn import(src: &Path, options: &ImportOptions) -> Result<ImportReport> {
    let mut files = read_export(src)?;
    let at = files
        .iter()
        .position(|(k, _)| k == EXPORT_MANIFEST)
        .ok_or_else(|| anyhow!("{EXPORT_MANIFEST} missing — not a theme export"))?;
    let (_, manifest_bytes) = files.remove(at);
    let manifest: ExportManifest =
        serde_json::from_slice(&manifest_bytes).context("invalid export.json")?;
    if !matches!(manifest.format.as_str(), FORMAT | "theme-profile/export@1") {
        bail!("unsupported export format {:?}", manifest.format);
    }
    for (key, bytes) in &files {
        if crate::paths::is_agent_metadata(Path::new(key)) {
            bail!("Agent instructions/configuration are not theme content: {key}");
        }
        match manifest.files.get(key) {
            Some(sum) if *sum == sha256_hex(bytes) => {}
            Some(_) => bail!("checksum mismatch: {key}"),
            None => bail!("file not listed in export.json: {key}"),
        }
    }
    if files.len() != manifest.files.len() {
        bail!(
            "export is incomplete ({} of {} files)",
            files.len(),
            manifest.files.len()
        );
    }
    let theme_json = files
        .iter()
        .find(|(k, _)| k == theme::MANIFEST)
        .ok_or_else(|| anyhow!("theme.json missing"))?;
    let package: theme::Package =
        serde_json::from_slice(&theme_json.1).context("invalid theme.json")?;
    if !theme::supported_schema(&package.schema) {
        bail!("unsupported theme schema {:?}", package.schema);
    }

    theme::validate_assets(&package)?;

    let name = options.name.clone().unwrap_or_else(|| package.name.clone());
    validate_profile_name(&name)?;
    if let Some(other) = case_duplicate_of(&name) {
        bail!("theme \"{other}\" already exists — names may not differ only by case (use --name)");
    }
    let target = profile_dir(&name)?;
    if target.exists() && !options.force {
        bail!("theme \"{name}\" already exists (use --name=NEW or --force)");
    }

    let profiles = target
        .parent()
        .ok_or_else(|| anyhow!("bad profiles dir"))?
        .to_path_buf();
    fs::create_dir_all(&profiles)?;
    let tmp = unique_dir(&profiles, &format!(".import-{name}"))?;
    let home = home_dir().to_string_lossy().to_string();
    let written = (|| -> Result<()> {
        for (key, bytes) in &files {
            if crate::appearance::is_preserved(key) {
                continue;
            }
            let dst = tmp.join(key);
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            if is_text(bytes) {
                let text = token_to_home(std::str::from_utf8(bytes).unwrap_or_default(), &home);
                fs::write(&dst, text)?;
            } else {
                fs::write(&dst, bytes)?;
            }
        }
        theme::rename_manifest(&tmp, &name)
    })();
    if let Err(e) = written {
        let _ = fs::remove_dir_all(&tmp);
        return Err(e);
    }

    let mut replaced = None;
    if target.exists() {
        let backups = backups_dir();
        fs::create_dir_all(&backups)?;
        let old = unique_dir(&backups, &format!("pre-import-{name}"))?;
        fs::rename(&target, &old)?;
        replaced = Some(old.display().to_string());
    }
    fs::rename(&tmp, &target)?;
    let missing = theme::check(&name)?;
    let dependencies = theme::needed(&name)?;
    Ok(ImportReport {
        name,
        files: files.len(),
        replaced,
        missing,
        dependencies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_token_round_trip_respects_name_boundaries() {
        let text = "Image=file:///home/cbr/Pics/a.jpg\nother=/home/cbr18/x\nend=/home/cbr";
        let out = home_to_token(text, "/home/cbr");
        assert_eq!(
            out,
            "Image=file://$HOME/Pics/a.jpg\nother=/home/cbr18/x\nend=$HOME"
        );
        assert_eq!(
            token_to_home(&out, "/home/anna/"),
            "Image=file:///home/anna/Pics/a.jpg\nother=/home/cbr18/x\nend=/home/anna"
        );
        assert_eq!(token_to_home("$HOMEDIR stays", "/h"), "$HOMEDIR stays");
        // KDE's own expanding entries keep their literal $HOME.
        assert_eq!(
            token_to_home("A[$e]=file:$HOME/x\nB=file://$HOME/y\n", "/home/u"),
            "A[$e]=file:$HOME/x\nB=file:///home/u/y\n"
        );
    }

    #[test]
    fn export_keeps_snapshot_konsole_profile_and_license() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(
            dir.join("theme.json"),
            r#"{"schema":"kolira/theme@2","name":"test"}"#,
        )
        .unwrap();
        fs::create_dir_all(dir.join(".local/share/konsole")).unwrap();
        fs::write(
            dir.join(".local/share/konsole/Original.profile"),
            "[Appearance]\nColorScheme=Original\n",
        )
        .unwrap();
        fs::write(dir.join("LICENSE"), "MIT").unwrap();
        let files = theme_files(dir, true).unwrap();
        assert!(files.contains(&PathBuf::from(".local/share/konsole/Original.profile")));
        assert!(files.contains(&PathBuf::from("LICENSE")));
    }

    #[test]
    fn exports_carry_only_the_library_entries_the_theme_uses() {
        let assets = vec![
            PathBuf::from(".local/share/icons/Mine"),
            PathBuf::from(".local/share/color-schemes/Mine.colors"),
            PathBuf::from(".config/Kvantum/Orchis"),
        ];
        assert!(exported(
            Path::new(".local/share/color-schemes/Mine.colors"),
            &assets
        ));
        assert!(!exported(
            Path::new(".local/share/color-schemes/Sweet.colors"),
            &assets
        ));
        assert!(exported(
            Path::new(".config/Kvantum/Orchis/OrchisDark.kvconfig"),
            &assets
        ));
        assert!(!exported(
            Path::new(".config/Kvantum/Sandstone/Sandstone.svg"),
            &assets
        ));
        assert!(exported(
            Path::new(".config/Kvantum/kvantum.kvconfig"),
            &assets
        ));
        assert!(exported(Path::new(".config/kdeglobals"), &assets));
        assert!(exported(
            Path::new(".local/share/icons/Mine/scalable/folder.svg"),
            &assets
        ));
        assert!(!exported(
            Path::new(".local/share/icons/Other/index.theme"),
            &assets
        ));
    }

    #[test]
    fn unsafe_paths_are_rejected() {
        for bad in ["../x", "/etc/passwd", "a/../../b", ""] {
            assert!(safe_rel(Path::new(bad)).is_err(), "{bad}");
        }
        assert!(safe_rel(Path::new(".config/kdeglobals")).is_ok());
    }

    #[test]
    fn zip_slip_entry_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("evil.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("../../evil", opts).unwrap();
        zip.write_all(b"x").unwrap();
        zip.finish().unwrap();
        let err = read_export(&path).unwrap_err().to_string();
        assert!(err.contains("unsafe path"), "{err}");
    }
}
