//! Profile Core — native save/apply/diff/delete in Rust.
//!
//! 0.5.0 moved every mutation out of the bash script into this module. The
//! bash `theme-profile` no longer exists; the CLI (`src/bin/theme-profile`)
//! and the FFI library (`kde_theme_core.so`) both use these functions.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use serde::{Serialize, Deserialize};

use crate::appearance;
use crate::config_parser::{ini_group_key, ini_group_lines};
use crate::paths::{
    active_file, backups_dir, config_home, live_path, profile_dir as safe_profile_dir,
    profiles_dir as theme_profiles_dir, validate_profile_name,
};
use crate::tracked_apps::component_config_paths;
use crate::types::ProfileInfo;
use crate::wallpaper::{plasma_wallpaper_from, wallpaper_dir, APPLETSRC_REL};

/// Version of the `manifest.json` theme-file format written by this tool.
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

/// The formalised "theme text file" — a profile's metadata.
///
/// Written next to the legacy `manifest.txt` (kept for older tools). File
/// snapshots remain the source of truth; this JSON only describes them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThemeManifest {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub tool_version: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub wallpaper: String,
    #[serde(default)]
    pub accent_from_wallpaper: bool,
    #[serde(default)]
    pub components: Vec<ManifestComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestComponent {
    pub id: String,
    #[serde(default)]
    pub paths: Vec<String>,
}

fn default_schema_version() -> u32 {
    MANIFEST_SCHEMA_VERSION
}

/// Build a `ThemeManifest` from the resolved tracked paths.
fn build_manifest(profile: &str, rel_paths: &[String]) -> ThemeManifest {
    let known = component_config_paths();
    let components: Vec<ManifestComponent> = known
        .into_iter()
        .filter_map(|(id, cpaths)| {
            let matched: Vec<String> = cpaths
                .iter()
                .filter(|p| rel_paths.contains(p))
                .cloned()
                .collect();
            if matched.is_empty() {
                None
            } else {
                Some(ManifestComponent {
                    id,
                    paths: matched,
                })
            }
        })
        .collect();

    let accent_fw = ini_group_key(
        &config_home().join("kdeglobals"),
        "General",
        "accentColorFromWallpaper",
    )
    .as_deref()
    == Some("true");

    ThemeManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        created_at: chrono::Local::now()
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        name: profile.to_string(),
        description: String::new(),
        wallpaper: String::new(),
        accent_from_wallpaper: accent_fw,
        components,
    }
}

/// Write `manifest.json` next to the profile snapshot.
fn write_json_manifest(target: &Path, manifest: &ThemeManifest) -> Result<()> {
    let json = serde_json::to_string_pretty(manifest)?;
    fs::write(target.join("manifest.json"), json + "\n")?;
    Ok(())
}

/// Read the `manifest.json` of a profile and return it (a fallback default when
/// the file is missing, so old profiles stay readable).
fn read_manifest(profile_dir: &Path) -> ThemeManifest {
    let path = profile_dir.join("manifest.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_else(|| ThemeManifest {
            name: profile_dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            ..Default::default()
        })
}

/// Read the description from the manifest json (empty string for old profiles).
pub fn get_profile_description(profile_name: &str) -> Result<String> {
    let dir = safe_profile_dir(profile_name)?;
    if crate::theme::is_package(&dir) {
        return Ok(crate::theme::load(&dir)?.description);
    }
    Ok(read_manifest(&dir).description)
}

/// Update a profile's description (rewrites `manifest.json` in place — no
/// other files in the snapshot are touched).
pub fn set_profile_description(profile_name: &str, description: &str) -> Result<()> {
    let dir = safe_profile_dir(profile_name)?;
    if !dir.is_dir() {
        return Err(anyhow!("Profile not found: {profile_name}"));
    }
    if crate::theme::is_package(&dir) {
        return crate::theme::set_description(profile_name, description);
    }
    let mut manifest = read_manifest(&dir);
    manifest.description = description.to_string();
    write_json_manifest(&dir, &manifest)
}

/// List all profiles
pub fn list_profiles() -> Result<Vec<ProfileInfo>> {
    let profiles_dir = theme_profiles_dir();
    if !profiles_dir.exists() {
        return Ok(vec![]);
    }

    let active = get_active_profile().unwrap_or_default();
    let mut profiles = Vec::new();

    for entry in fs::read_dir(&profiles_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();

        if entry.file_type()?.is_dir() {
            if validate_profile_name(&name).is_err() {
                continue;
            }
            let profile_path = entry.path();
            let is_active = name == active;
            let components = get_profile_components(&name)?;
            let created_at = get_profile_created_at(&profile_path)?;

            profiles.push(ProfileInfo {
                name,
                is_active,
                components,
                created_at,
                description: read_manifest(&profile_path).description,
                wallpaper_path: String::new(),
            });
        }
    }

    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(profiles)
}

/// Get active profile name
pub fn get_active_profile() -> Result<String> {
    let active_file = active_file();
    if active_file.exists() {
        let content = fs::read_to_string(&active_file)?;
        let name = content.trim();
        if validate_profile_name(name).is_ok() {
            Ok(name.to_string())
        } else {
            Ok(String::new())
        }
    } else {
        Ok(String::new())
    }
}

/// Get components in a profile
///
/// A profile directory mirrors the real layout (`<profile>/.config/kwinrc`, …),
/// so components are detected through the same tracked-app registry the CLI
/// core uses — joining the bare component id to the profile root would look for
/// `<profile>/kwin` and find nothing.
pub fn get_profile_components(profile_name: &str) -> Result<Vec<String>> {
    let profile_dir = safe_profile_dir(profile_name)?;
    if !profile_dir.exists() {
        return Ok(vec![]);
    }

    let mut components = Vec::new();
    for (id, config_paths) in component_config_paths() {
        if config_paths
            .iter()
            .any(|rel_path| profile_dir.join(rel_path).exists())
        {
            components.push(id);
        }
    }
    Ok(components)
}

/// Get profile creation timestamp (manifest.json preferred, then manifest.txt)
fn get_profile_created_at(profile_path: &Path) -> Result<String> {
    let manifest = read_manifest(profile_path);
    if !manifest.created_at.is_empty() {
        return Ok(manifest.created_at);
    }
    let legacy = profile_path.join("manifest.txt");
    if legacy.exists() {
        let content = fs::read_to_string(&legacy)?;
        for line in content.lines() {
            if let Some(stamp) = line.strip_prefix("timestamp=") {
                return Ok(stamp.to_string());
            }
            if line.starts_with('[') {
                break;
            }
        }
    }
    Ok(String::new())
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Strict component selection for the FFI/GUI path (mirrors the CLI core):
/// `[]` means "nothing selected", never "all"; unknown ids are errors.
fn resolve_components(components: Option<Vec<String>>) -> Result<Vec<String>> {
    let known = component_config_paths();
    let ids: Vec<String> = match components {
        None => known.iter().map(|(id, _)| id.clone()).collect(),
        Some(list) => {
            if list.is_empty() {
                return Err(anyhow!("Select at least one component"));
            }
            let mut cleaned: Vec<String> = Vec::new();
            for component in list {
                if component.is_empty()
                    || component.contains(',')
                    || component.contains('=')
                    || component.contains('\n')
                {
                    return Err(anyhow!("Invalid component id: {component:?}"));
                }
                if !known.iter().any(|(id, _)| id == &component) {
                    return Err(anyhow!("Unknown component: {component}"));
                }
                if !cleaned.contains(&component) {
                    cleaned.push(component);
                }
            }
            if cleaned.is_empty() {
                return Err(anyhow!("Select at least one component"));
            }
            cleaned
        }
    };

    let mut paths: Vec<String> = Vec::new();
    for (id, config_paths) in &known {
        if ids.contains(id) {
            for rel in config_paths {
                if !paths.contains(rel) {
                    paths.push(rel.clone());
                }
            }
        }
    }
    Ok(paths)
}

fn no_apply() -> bool {
    std::env::var("KTM_THEME_NO_APPLY").as_deref() == Ok("1")
}

/// Run an external command ignoring the result (all hooks are best-effort).
fn run_external(cmd: &str, args: &[&str]) {
    if no_apply() {
        return;
    }
    let _ = Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// Run an external command and return its trimmed stdout (empty on failure).
fn run_external_capture(cmd: &str, args: &[&str]) -> String {
    if no_apply() {
        return String::new();
    }
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

pub(crate) fn unique_dir(root: &Path, prefix: &str) -> Result<PathBuf> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    for attempt in 0..100 {
        let candidate = root.join(format!("{prefix}-{}-{nanos}-{attempt}", std::process::id()));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(anyhow!("Could not create a unique directory under {}", root.display()))
}

fn remove_path(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() || meta.is_file() {
        fs::remove_file(path)?;
    } else {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

/// `cp -a` semantics: symlinks stay symlinks, directories recurse, permissions
/// and modes are preserved.
pub(crate) fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(src)?;

    if meta.file_type().is_symlink() {
        let target = fs::read_link(src)?;
        std::os::unix::fs::symlink(target, dst)?;
        return Ok(());
    }

    if meta.is_dir() {
        fs::create_dir_all(dst)?;
        fs::set_permissions(dst, fs::Permissions::from_mode(meta.permissions().mode()))?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            copy_tree(&entry.path(), &dst.join(entry.file_name()))?;
        }
        return Ok(());
    }

    fs::copy(src, dst)?;
    fs::set_permissions(dst, fs::Permissions::from_mode(meta.permissions().mode()))?;
    Ok(())
}

/// Copy the tracked live files into a target directory.
/// `[ -e "$src" ] || continue` — broken symlinks and missing files are skipped.
fn copy_tracked_into(target: &Path, rel_paths: &[String]) -> Result<()> {
    fs::create_dir_all(target)?;
    for rel in rel_paths {
        let src = live_path(rel);
        if !src.exists() {
            continue;
        }
        let dst = target.join(rel);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        if fs::symlink_metadata(&dst).is_ok() {
            remove_path(&dst)?;
        }
        copy_tree(&src, &dst).with_context(|| format!("Failed to copy {} -> {}", src.display(), dst.display()))?;
    }
    Ok(())
}

/// Save variant of [`copy_tracked_into`]: every file goes through
/// [`appearance::for_theme`] — only the look is stored (personal keys such as
/// screen scaling, key bindings or an SSH host never enter a theme).
fn copy_tracked_appearance(target: &Path, rel_paths: &[String]) -> Result<()> {
    fs::create_dir_all(target)?;
    for rel in rel_paths {
        let src = live_path(rel);
        if !src.exists() {
            continue;
        }
        let dst = target.join(rel);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        if fs::symlink_metadata(&dst).is_ok() {
            remove_path(&dst)?;
        }
        let text = (src.is_file()).then(|| fs::read_to_string(&src).ok()).flatten();
        if rel == crate::extras::OMP_THEME {
            continue; // written below from the prompt the shell really uses
        }
        match text {
            Some(text) if !matches!(appearance::policy(rel), appearance::Policy::Whole) => {
                if let Some(look) = appearance::for_theme(rel, &text) {
                    fs::write(&dst, look)?;
                }
            }
            _ => copy_tree(&src, &dst).with_context(|| format!("Failed to copy {} -> {}", src.display(), dst.display()))?,
        }
    }
    Ok(())
}

/// Theme files that come from elsewhere on save (no live change): the prompt
/// theme the shell's oh-my-posh line points to, and ghostty's look when the
/// user's config still holds it.
fn capture_derived(target: &Path, rel_paths: &[String]) -> Result<()> {
    if rel_paths.iter().any(|r| r == crate::extras::OMP_THEME) {
        let src = crate::extras::omp_current().unwrap_or_else(|| live_path(crate::extras::OMP_THEME));
        if src.is_file() {
            let dst = target.join(crate::extras::OMP_THEME);
            if let Some(p) = dst.parent() {
                fs::create_dir_all(p)?;
            }
            fs::copy(&src, &dst)?;
        }
    }
    if rel_paths.iter().any(|r| r == crate::extras::GHOSTTY_HOST) && !live_path(crate::extras::GHOSTTY_THEME).exists() {
        if let Ok(host) = fs::read_to_string(live_path(crate::extras::GHOSTTY_HOST)) {
            let (look, _) = crate::extras::ghostty_split(&host);
            if !look.trim().is_empty() {
                write_text(&target.join(crate::extras::GHOSTTY_THEME), &look)?;
            }
        }
    }
    Ok(())
}

/// One-time move of the look out of the user's kitty.conf / alacritty.toml
/// into the include files (the way ricers keep themes apart from config).
/// The host is backed up to `backups/pre-split-<app>-…` first.
pub(crate) fn split_live_hosts(rel_paths: &[String]) -> Result<()> {
    for (host, include) in [
        (appearance::KITTY_HOST, appearance::KITTY_THEME),
        (appearance::ALACRITTY_HOST, appearance::ALACRITTY_THEME),
    ] {
        if !rel_paths.iter().any(|r| r == host) {
            continue;
        }
        let host_path = live_path(host);
        let Ok(text) = fs::read_to_string(&host_path) else { continue };
        let Some((_, look)) = appearance::include_from_host(host, &text)? else { continue };
        let backups = backups_dir();
        fs::create_dir_all(&backups)?;
        let app = if host == appearance::KITTY_HOST { "kitty" } else { "alacritty" };
        let backup = unique_dir(&backups, &format!("pre-split-{app}"))?;
        copy_tracked_into(&backup, &[host.to_string(), include.to_string()])?;
        let include_path = live_path(include);
        let existing = fs::read_to_string(&include_path).unwrap_or_default();
        // Lines from the host come last: they are what the user saw.
        let merged = if host == appearance::KITTY_HOST {
            format!("{}{}{}", existing, if existing.is_empty() || existing.ends_with('\n') { "" } else { "\n" }, look)
        } else {
            merge_toml(&existing, &look)?
        };
        write_text(&include_path, &merged)?;
        write_text(&host_path, &appearance::applied(host, "", Some(&text))?)?;
    }
    Ok(())
}

/// `base` with every value of `over` set on top (tables merged key by key).
fn merge_toml(base: &str, over: &str) -> Result<String> {
    let mut doc: toml_edit::DocumentMut = base.parse().unwrap_or_default();
    let over: toml_edit::DocumentMut = over.parse()?;
    fn merge(into: &mut toml_edit::Table, from: &toml_edit::Table) {
        for (k, v) in from.iter() {
            match (into.get_mut(k).and_then(toml_edit::Item::as_table_mut), v.as_table()) {
                (Some(dst), Some(src)) => merge(dst, src),
                _ => {
                    into.insert(k, v.clone());
                }
            }
        }
    }
    merge(doc.as_table_mut(), over.as_table());
    Ok(doc.to_string())
}

fn write_text(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text).with_context(|| format!("write {}", path.display()))
}

pub(crate) fn write_active(name: &str) -> Result<()> {
    let path = active_file();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true).mode(0o600);
    let mut file = opts.open(&path)?;
    writeln!(file, "{name}")?;
    Ok(())
}

fn write_manifest(target: &Path, profile: &str, rel_paths: &[String]) -> Result<()> {
    // Legacy format: timestamp + tracked paths (kept for older tools that read
    // manifest.txt; the new format is manifest.json).
    let mut out = String::from("Generated by theme-profile\n");
    out.push_str(&format!(
        "timestamp={}\n",
        chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
    ));
    out.push_str(&format!("user={}\n", std::env::var("USER").unwrap_or_default()));
    out.push('\n');
    out.push_str("[tracked]\n");
    for rel in rel_paths {
        out.push_str(rel);
        out.push('\n');
    }
    fs::write(target.join("manifest.txt"), out)?;

    // Formal "theme text file" (schema v1).
    write_json_manifest(target, &build_manifest(profile, rel_paths))
}

/// Best-effort KDE-effects snapshot (metadata only — never blocks a save/apply).
fn capture_kde_effects_state(target: &Path) -> Result<()> {
    let meta = target.join("metadata/kde");
    fs::create_dir_all(&meta)?;

    let mut notification = run_external_capture(
        "qdbus6",
        &[
            "org.freedesktop.Notifications",
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications.GetServerInformation",
        ],
    );
    if notification.is_empty() {
        notification = run_external_capture(
            "busctl",
            &[
                "--user", "call", "org.freedesktop.Notifications", "/org/freedesktop/Notifications",
                "org.freedesktop.Notifications", "GetServerInformation",
            ],
        );
    }

    let mut support = run_external_capture("qdbus6", &["org.kde.KWin", "/KWin", "supportInformation"]);
    if support.is_empty() {
        support = run_external_capture(
            "busctl",
            &["--user", "call", "org.kde.KWin", "/KWin", "org.kde.KWin", "supportInformation"],
        );
    }

    let mut active_effects =
        run_external_capture("qdbus6", &["org.kde.KWin", "/Effects", "activeEffects"]);
    if active_effects.is_empty() {
        active_effects = run_external_capture(
            "busctl",
            &["--user", "call", "org.kde.KWin", "/Effects", "org.kde.kwin.Effects", "activeEffects"],
        );
    }

    let mut loaded_effects =
        run_external_capture("qdbus6", &["org.kde.KWin", "/Effects", "loadedEffects"]);
    if loaded_effects.is_empty() {
        loaded_effects = run_external_capture(
            "busctl",
            &["--user", "call", "org.kde.KWin", "/Effects", "org.kde.kwin.Effects", "loadedEffects"],
        );
    }

    let kwinrc = config_home().join("kwinrc");
    let mut summary = String::from("Generated by theme-profile\n");
    summary.push_str(&format!(
        "timestamp={}\n",
        chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
    ));
    summary.push_str("\n[kwinrc: Plugins]\n");
    for key in [
        "glassEnabled",
        "blurEnabled",
        "better_blur_dxEnabled",
        "contrastEnabled",
        "translucencyEnabled",
    ] {
        summary.push_str(&format!("{key}={}\n", ini_group_key(&kwinrc, "Plugins", key).unwrap_or_default()));
    }
    summary.push_str("\n[notification-server]\n");
    summary.push_str(&notification);
    summary.push_str("\n\n[kwin-dbus-active-effects]\n");
    summary.push_str(&active_effects);
    summary.push_str("\n\n[kwin-dbus-loaded-effects]\n");
    summary.push_str(&loaded_effects);
    summary.push('\n');
    fs::write(meta.join("summary.txt"), summary)?;

    fs::write(meta.join("kwin-support-information.txt"), support)?;
    fs::write(
        meta.join("kwin-plugins.txt"),
        run_external_capture("kreadconfig6", &["--file", "kwinrc", "--group", "Plugins"]),
    )?;

    let mut glass = String::new();
    for group in ["Effect-blur", "Effect-blurplus", "Effect-better-blur-dx", "Effect-translucency"] {
        glass.push_str(&format!("[{group}]\n"));
        for line in ini_group_lines(&kwinrc, group) {
            glass.push_str(&line);
            glass.push('\n');
        }
        glass.push('\n');
    }
    fs::write(meta.join("glass-and-blur-config.txt"), glass)?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Apply
// ─────────────────────────────────────────────────────────────────────────────

fn stop_plasma_shell_for_restore() {
    run_external("systemctl", &["--user", "reset-failed", "plasma-plasmashell.service"]);
    run_external("systemctl", &["--user", "stop", "plasma-plasmashell.service"]);
}

fn plasma_shell_active() -> bool {
    if no_apply() {
        return true;
    }
    Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", "plasma-plasmashell.service"])
        .status()
        .map(|s| s.success())
        .unwrap_or(true)
}

fn start_plasma_shell() {
    run_external("systemctl", &["--user", "reset-failed", "plasma-plasmashell.service"]);
    run_external("systemctl", &["--user", "start", "plasma-plasmashell.service"]);
    if !no_apply() {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    if !plasma_shell_active() {
        run_external("systemctl", &["--user", "reset-failed", "plasma-plasmashell.service"]);
        run_external("systemctl", &["--user", "start", "plasma-plasmashell.service"]);
    }
}

fn sync_kwin_effect(source: &Path, effect: &str) {
    let key = format!("{effect}Enabled");
    let enabled = ini_group_key(&source.join(".config/kwinrc"), "Plugins", &key).unwrap_or_default();
    match enabled.as_str() {
        "true" => run_external("qdbus6", &["org.kde.KWin", "/Effects", "loadEffect", effect]),
        "false" => run_external("qdbus6", &["org.kde.KWin", "/Effects", "unloadEffect", effect]),
        _ => {}
    }
}

fn apply_kitty_live(source: &Path) {
    let config = source.join(".config/kitty/kitty.conf");
    if !config.is_file() || which::which("kitty").is_err() {
        return;
    }
    let config_str = config.to_string_lossy().to_string();
    // kitty appends `-<pid>` to the listen_on path: reload every instance.
    for socket in crate::palette::kitty_sockets() {
        reload_kitty(&socket, &config, &config_str);
    }
}

fn reload_kitty(socket: &str, config: &Path, config_str: &str) {
    run_external("kitty", &["@", "--to", socket, "load-config", config_str]);

    let opacity = std::fs::read_to_string(config)
        .ok()
        .and_then(|content| {
            content.lines().find_map(|line| {
                let mut parts = line.split_whitespace();
                if parts.next() == Some("background_opacity") {
                    parts.next().map(|v| v.to_string())
                } else {
                    None
                }
            })
        });
    if let Some(opacity) = opacity {
        run_external("kitty", &["@", "--to", socket, "set-background-opacity", "--all", &opacity]);
    }

    run_external("kitty", &["@", "--to", socket, "set-colors", "--all", "--configured", config_str]);
}

/// Order of qdbus commands matches the historical bash script.
/// Characters that would make a reload command a shell-injection vector.
/// The command is never run through a shell; these are rejected up front so a
/// typo'd pipe or substitution fails loudly instead of half-running.
const RELOAD_COMMAND_FORBIDDEN: &[char] =
    &['|', '&', ';', '<', '>', '$', '`', '"', '\'', '\n', '\r'];

/// Validate a user-defined reload command: one binary resolvable via PATH plus
/// its arguments, no shell operators. Returns the argv for `Command`.
pub fn validate_reload_command(command: &str) -> Result<Vec<String>> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Reload command is empty"));
    }
    if let Some(bad) = trimmed.chars().find(|c| RELOAD_COMMAND_FORBIDDEN.contains(c)) {
        return Err(anyhow!(
            "Reload command contains a forbidden shell character: {bad:?} \
             (commands run without a shell)"
        ));
    }
    let argv: Vec<String> = trimmed.split_whitespace().map(str::to_owned).collect();
    let binary = &argv[0];
    if which::which(binary).is_err() {
        return Err(anyhow!("Reload command binary not found in PATH: {binary}"));
    }
    Ok(argv)
}

/// Run the reload commands of the applied *custom* components (best-effort).
///
/// Fires after the files are copied and the built-in live hooks ran; an error
/// is a warning only — the apply itself has already succeeded. Skipped
/// entirely under `KTM_THEME_NO_APPLY=1`.
pub(crate) fn run_custom_reload_hooks(rel_paths: &[String]) {
    if no_apply() {
        return;
    }
    let Ok(custom) = crate::custom_components::load_custom_components() else {
        return;
    };
    for component in custom {
        let Some(command) = component.reload_command.as_deref() else {
            continue;
        };
        if command.trim().is_empty() {
            continue;
        }
        let applies = component.config_paths.iter().any(|p| rel_paths.contains(p));
        if !applies {
            continue;
        }
        match validate_reload_command(command) {
            Ok(argv) => {
                let mut iter = argv.into_iter();
                let binary = iter.next().unwrap_or_default();
                let status = Command::new(&binary)
                    .args(iter.collect::<Vec<_>>())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
                match status {
                    Ok(s) if s.success() => {}
                    Ok(s) => {
                        eprintln!("Warning: reload command for '{}' exited with {s}", component.id)
                    }
                    Err(e) => eprintln!(
                        "Warning: could not run reload command for '{}': {e}",
                        component.id
                    ),
                }
            }
            Err(e) => eprintln!("Warning: invalid reload command for '{}': {e}", component.id),
        }
    }
}

fn rgb_string_to_hex(value: &str) -> Option<String> {
    let parts: Vec<&str> = value.split(',').collect();
    if parts.len() != 3 {
        return None;
    }
    let rgb: Option<Vec<u8>> = parts
        .iter()
        .map(|part| part.trim().parse::<u8>().ok())
        .collect();
    let rgb = rgb?;
    Some(format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]))
}

/// Restoring kdeglobals is not enough: Qt/Plasma keeps the previous palette in
/// memory, so the scheme is re-applied explicitly.
///
/// Accent: an explicit `AccentColor` is passed along. When the profile takes
/// the accent from the wallpaper we do *not* compute it ourselves — Plasma
/// (plasmashell's accent service) reads `accentColorFromWallpaper` on start-up
/// and recomputes the accent from the restored wallpaper with its own
/// algorithm; `apply` restarts plasmashell right after these hooks. (Verified
/// 2026-11-04; this replaced an ImageMagick average-colour approximation.)
fn reapply_color_scheme(source: &Path) {
    let kdeglobals = source.join(".config/kdeglobals");
    let Some(scheme) = ini_group_key(&kdeglobals, "General", "ColorScheme") else {
        return;
    };
    if scheme.is_empty() {
        return;
    }

    let from_wallpaper = ini_group_key(&kdeglobals, "General", "accentColorFromWallpaper")
        .as_deref()
        == Some("true");
    let explicit = ini_group_key(&kdeglobals, "General", "AccentColor")
        .and_then(|value| rgb_string_to_hex(&value));

    // The kded accent service (plasma_accentcolor_service) and plasmashell
    // learn about kdeglobals changes from KConfig D-Bus notifications, not from
    // the file restore_from_profile just copied — after paper → glass the
    // service still believed "accent from wallpaper = off" and dropped the
    // wallpaper accent. Re-write the flag with --notify; flip first, because
    // writing an unchanged value emits nothing.
    let (flag, flipped) = if from_wallpaper { ("true", "false") } else { ("false", "true") };
    for value in [flipped, flag] {
        run_external(
            "kwriteconfig6",
            &["--notify", "--file", "kdeglobals", "--group", "General", "--key", "accentColorFromWallpaper", value],
        );
    }

    // Scheme first, accent in a separate call: given `--accent-color`,
    // plasma-apply-colorscheme sets only the accent and silently ignores the
    // scheme argument (verified 2026-11-04, Plasma 6).
    run_external("plasma-apply-colorscheme", &[&scheme]);
    if let Some(accent) = explicit.filter(|_| !from_wallpaper) {
        run_external("plasma-apply-colorscheme", &["--accent-color", &accent]);
    }

    // The copied kdeglobals already names this scheme, so the call above is a
    // no-op ("already in use") and running apps keep the previous profile's
    // palette (paper's olive after switching to glass). Broadcast the palette
    // change ourselves — KGlobalSettings::PaletteChanged (0), the signal
    // plasma-apply-colorscheme emits when it does apply.
    run_external(
        "dbus-send",
        &["--session", "--type=signal", "/KGlobalSettings", "org.kde.KGlobalSettings.notifyChange", "int32:0", "int32:0"],
    );
}

fn reapply_widget_style(source: &Path) {
    let kdeglobals = source.join(".config/kdeglobals");
    let Some(widget_style) = ini_group_key(&kdeglobals, "KDE", "widgetStyle") else {
        return;
    };
    if widget_style.is_empty() {
        return;
    }

    run_external(
        "kwriteconfig6",
        &["--file", "kdeglobals", "--group", "KDE", "--key", "widgetStyle", &widget_style],
    );

    if widget_style != "kvantum" {
        return;
    }
    let Some(theme) = ini_group_key(&source.join(".config/Kvantum/kvantum.kvconfig"), "General", "theme") else {
        return;
    };
    if theme.is_empty() {
        return;
    }
    run_external("kvantummanager", &["--set", &theme]);
}

fn apply_live_hooks(source: &Path) {
    // Read from the profile, never via `kreadconfig6` on the live config: that
    // used to pass the profile path as an extra positional argument, making
    // kreadconfig6 print --help into kdeglobals as the LookAndFeelPackage name.
    let look_and_feel = ini_group_key(&source.join(".config/kdeglobals"), "KDE", "LookAndFeelPackage")
        .unwrap_or_default();
    if !look_and_feel.is_empty() {
        run_external(
            "kwriteconfig6",
            &["--file", "kdeglobals", "--group", "KDE", "--key", "LookAndFeelPackage", &look_and_feel],
        );
    }

    reapply_color_scheme(source);
    reapply_widget_style(source);

    run_external("kbuildsycoca6", &[]);
    run_external("systemctl", &["--user", "stop", "swaync.service"]);
    run_external("qdbus6", &["org.kde.KWin", "/KWin", "reconfigure"]);

    for effect in ["glass", "blur", "contrast", "better_blur_dx"] {
        sync_kwin_effect(source, effect);
    }
    for effect in ["glass", "blur", "contrast", "better_blur_dx"] {
        run_external("qdbus6", &["org.kde.KWin", "/Effects", "reconfigureEffect", effect]);
    }

    apply_kitty_live(source);
    start_plasma_shell();
}

/// Directories whose profile folder has a random per-install name.
const PROFILE_ROOTS: &[&str] = &[".zen", ".mozilla/firefox"];

/// The theme's copy of tracked file `rel` (a path on *this* machine). A
/// browser profile folder is named randomly per install (`gp0y8wrv.Default
/// (release)`), so a theme made elsewhere stores the same file under its
/// author's folder: fall back to the theme's profile folder holding it.
pub(crate) fn theme_file(theme: &Path, rel: &str) -> PathBuf {
    let direct = theme.join(rel);
    if direct.exists() {
        return direct;
    }
    // Konsole: the receiver's default profile ← the theme's profile.
    if rel.starts_with(".local/share/konsole/") && rel.ends_with(".profile") {
        if let Ok(entries) = fs::read_dir(theme.join(".local/share/konsole")) {
            let mut profiles: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "profile")).collect();
            profiles.sort();
            if let Some(p) = profiles.into_iter().next() {
                return p;
            }
        }
    }
    // Vencord and Vesktop share the themes/settings format.
    for (from, to) in [(".config/Vencord/", ".config/vesktop/"), (".config/vesktop/", ".config/Vencord/")] {
        if let Some(inner) = rel.strip_prefix(from) {
            let other = theme.join(to).join(inner);
            if other.exists() {
                return other;
            }
        }
    }
    for root in PROFILE_ROOTS {
        let Some(rest) = rel.strip_prefix(&format!("{root}/")) else { continue };
        let Some((_, inner)) = rest.split_once('/') else { continue };
        let Ok(entries) = fs::read_dir(theme.join(root)) else { continue };
        let mut dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.join(inner).exists()).collect();
        dirs.sort();
        if let Some(dir) = dirs.into_iter().next() {
            return dir.join(inner);
        }
    }
    direct
}

/// Desktop-icon state in plasma appletsrc: icon positions per resolution and
/// which screen each desktop item sits on. Plasma rewrites these itself.
fn is_desktop_icon_key(group: &str, key: &str) -> bool {
    group == "[ScreenMapping]"
        || (group.ends_with("[General]") && matches!(key, "positions" | "changedPositions" | "lastResolution"))
}

/// `restored` (a theme's appletsrc) with every desktop-icon key taken from
/// `live` instead: switching themes must not move the user's desktop icons.
pub(crate) fn keep_desktop_icons(restored: &str, live: &str) -> String {
    let mut keep: Vec<(String, Vec<String>)> = Vec::new();
    let mut group = String::new();
    for line in live.lines() {
        if line.starts_with('[') {
            group = line.to_string();
            continue;
        }
        let Some((key, _)) = line.split_once('=') else { continue };
        if is_desktop_icon_key(&group, key.trim()) {
            match keep.iter_mut().find(|(g, _)| *g == group) {
                Some((_, lines)) => lines.push(line.to_string()),
                None => keep.push((group.clone(), vec![line.to_string()])),
            }
        }
    }

    let mut out: Vec<String> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let mut group = String::new();
    let flush = |out: &mut Vec<String>, group: &str, seen: &mut Vec<String>| {
        if let Some((_, lines)) = keep.iter().find(|(g, _)| g == group) {
            // Before the blank separator line, if the group ends with one.
            let at = if out.last().is_some_and(|l| l.is_empty()) { out.len() - 1 } else { out.len() };
            for (i, l) in lines.iter().enumerate() {
                out.insert(at + i, l.clone());
            }
            seen.push(group.to_string());
        }
    };
    for line in restored.lines() {
        if line.starts_with('[') {
            flush(&mut out, &group, &mut seen);
            group = line.to_string();
            out.push(line.to_string());
            continue;
        }
        if line.split_once('=').is_some_and(|(k, _)| is_desktop_icon_key(&group, k.trim())) {
            continue;
        }
        out.push(line.to_string());
    }
    flush(&mut out, &group, &mut seen);
    for (g, lines) in &keep {
        if !seen.contains(g) {
            if out.last().is_some_and(|l| !l.is_empty()) {
                out.push(String::new());
            }
            out.push(g.clone());
            out.extend(lines.iter().cloned());
        }
    }
    let mut text = out.join("\n");
    text.push('\n');
    text
}

fn restore_from_profile(profile: &str, rel_paths: &[String]) -> Result<()> {
    let source = safe_profile_dir(profile)?;
    if !source.is_dir() {
        return Err(anyhow!("Profile not found: {profile}"));
    }

    for rel in rel_paths {
        let src = theme_file(&source, rel);
        let dst = live_path(rel);
        match appearance::policy(rel) {
            appearance::Policy::OmpHost => {
                // Only point existing oh-my-posh lines at the theme's prompt.
                if source.join(crate::extras::OMP_THEME).is_file() && dst.is_file() {
                    let live = fs::read_to_string(&dst)?;
                    let updated = appearance::applied(rel, "", Some(&live))?;
                    if updated != live {
                        write_text(&dst, &updated)?;
                    }
                }
            }
            appearance::Policy::GhosttyHost => {
                if source.join(crate::extras::GHOSTTY_THEME).is_file() && (dst.is_file() || which::which("ghostty").is_ok()) {
                    let live = fs::read_to_string(&dst).ok();
                    write_text(&dst, &appearance::applied(rel, "", live.as_deref())?)?;
                }
            }
            appearance::Policy::NvimHost => {
                // Load the theme's Neovim file when the theme has one and
                // Neovim is set up here (init.vim users: left alone).
                let has_theme = source.join(crate::nvim::THEME).is_file();
                let nvim_dir = live_path(".config/nvim");
                if has_theme && !nvim_dir.join("init.vim").exists() && (dst.is_file() || which::which("nvim").is_ok()) {
                    let live = fs::read_to_string(&dst).ok();
                    write_text(&dst, &appearance::applied(rel, "", live.as_deref())?)?;
                }
            }
            appearance::Policy::KittyHost | appearance::Policy::AlacrittyHost => {
                let include = if rel == appearance::KITTY_HOST { appearance::KITTY_THEME } else { appearance::ALACRITTY_THEME };
                let has_include = source.join(include).is_file();
                // Old themes keep the look inside the host config: derive the
                // include file from it (a new theme's include is restored as a
                // whole file by its own entry).
                if !has_include {
                    if let Ok(theme_host) = fs::read_to_string(&src) {
                        if let Some((inc, look)) = appearance::include_from_host(rel, &theme_host)? {
                            write_text(&live_path(inc), &look)?;
                        }
                    }
                }
                let app = if rel == appearance::KITTY_HOST { "kitty" } else { "alacritty" };
                if (has_include || src.is_file()) && (dst.is_file() || which::which(app).is_ok()) {
                    let live = fs::read_to_string(&dst).ok();
                    write_text(&dst, &appearance::applied(rel, "", live.as_deref())?)?;
                }
            }
            policy => {
                if !src.exists() {
                    continue;
                }
                if let Some(parent) = dst.parent() {
                    fs::create_dir_all(parent)?;
                }
                let theme_text = (!matches!(policy, appearance::Policy::Whole) && src.is_file())
                    .then(|| fs::read_to_string(&src).ok())
                    .flatten();
                match theme_text {
                    Some(theme_text) => {
                        let live = fs::read_to_string(&dst).ok();
                        write_text(&dst, &appearance::applied(rel, &theme_text, live.as_deref())?)?;
                        if rel.starts_with(".local/share/konsole/") && rel.ends_with(".profile") {
                            crate::konsole::finish_profile(rel)?;
                        }
                    }
                    None => copy_tree(&src, &dst).with_context(|| {
                        format!("Failed to restore {} -> {}", src.display(), dst.display())
                    })?,
                }
            }
        }
    }
    write_active(profile)?;
    Ok(())
}

/// Apply a profile natively. Returns the pre-apply backup directory.
///
/// Profile validation happens *before* Plasma is stopped — a missing profile
/// must not touch the running session.
pub fn apply_profile_impl(profile: &str, components: Option<Vec<String>>) -> Result<PathBuf> {
    validate_profile_name(profile)?;
    let rel_paths = resolve_components(components)?;
    if rel_paths.is_empty() {
        return Err(anyhow!("No valid components selected"));
    }
    apply_with_paths(profile, &rel_paths)
}

/// CLI variant: unknown components warn (like the historical bash script)
/// instead of failing; `--components=all` and a plain call select everything.
pub fn apply_profile_cli(profile: &str, components: Option<Vec<String>>) -> Result<PathBuf> {
    validate_profile_name(profile)?;
    let rel_paths = resolve_cli_paths(components)?;
    if rel_paths.is_empty() {
        return Err(anyhow!("No valid components selected"));
    }
    apply_with_paths(profile, &rel_paths)
}

/// Apply every tracked file plus `extras` — the theme's own assets (its
/// colour scheme, Kvantum family, Look & Feel, …) whose paths the receiver's
/// live state does not name.
pub fn apply_profile_with(profile: &str, extras: &[String]) -> Result<PathBuf> {
    validate_profile_name(profile)?;
    let mut rel_paths = resolve_cli_paths(None)?;
    for e in extras {
        if !e.contains("..") && !e.starts_with('/') && !rel_paths.contains(e) {
            rel_paths.push(e.clone());
        }
    }
    apply_with_paths(profile, &rel_paths)
}

/// Warn about unknown components, keep the known ones (bash CLI behaviour).
pub fn resolve_cli_paths(components: Option<Vec<String>>) -> Result<Vec<String>> {
    let known = component_config_paths();
    let mut out: Vec<String> = Vec::new();
    match components {
        None => {
            for (_, paths) in &known {
                for p in paths {
                    if !out.contains(p) {
                        out.push(p.clone());
                    }
                }
            }
        }
        Some(list) => {
            for c in list {
                if let Some((_, paths)) = known.iter().find(|(id, _)| id == &c) {
                    for p in paths {
                        if !out.contains(p) {
                            out.push(p.clone());
                        }
                    }
                } else {
                    eprintln!("Warning: unknown component '{c}'");
                }
            }
        }
    }
    Ok(out)
}

fn apply_with_paths(profile: &str, rel_paths: &[String]) -> Result<PathBuf> {
    let dir = safe_profile_dir(profile)?;
    if !dir.is_dir() {
        return Err(anyhow!("Profile not found: {profile}"));
    }

    let backups = backups_dir();
    fs::create_dir_all(&backups)?;
    let backup = unique_dir(&backups, &format!("pre-apply-{profile}"))?;
    if copy_tracked_into(&backup, rel_paths)
        .and(capture_kde_effects_state(&backup))
        .and(write_manifest(&backup, profile, rel_paths))
        .is_err()
    {
        let _ = fs::remove_dir_all(&backup);
        return Err(anyhow!("Failed to create pre-apply backup"));
    }

    if !no_apply() {
        stop_plasma_shell_for_restore();
    }

    // Desktop icons are not part of the look: keep where they are now. Read
    // after plasmashell stopped — it writes its latest icon state on quit.
    let appletsrc = live_path(APPLETSRC_REL);
    let live_appletsrc = fs::read_to_string(&appletsrc).ok();
    match restore_from_profile(profile, rel_paths) {
        Ok(()) => {
            if let (Some(live), Ok(restored)) = (live_appletsrc.as_deref(), fs::read_to_string(&appletsrc)) {
                let _ = fs::write(&appletsrc, keep_desktop_icons(&restored, live));
            }
            // Resolve (and if needed restore) the profile wallpaper while
            // plasmashell is stopped, so the restored appletsrc finds its file.
            let wallpaper = if tracks_desktop_layout(rel_paths) {
                profile_wallpaper(&dir)
            } else {
                None
            };
            if let Err(e) = restore_active_kvantum_theme(&dir, rel_paths) {
                eprintln!("Warning: could not restore the Kvantum theme: {e}");
            }
            apply_live_hooks(&dir);
            if let Some(path) = wallpaper {
                let shown = plasma_wallpaper_from(&live_path(APPLETSRC_REL));
                if shown.as_deref() != Some(path.to_string_lossy().as_ref()) {
                    apply_wallpaper_live(&path);
                }
            }
            run_custom_reload_hooks(rel_paths);
            Ok(backup)
        }
        Err(e) => {
            if !no_apply() {
                start_plasma_shell();
            }
            Err(e)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Wallpaper inside profiles
// ─────────────────────────────────────────────────────────────────────────────

/// `<profile>/wallpaper/` holds a copy of the desktop wallpaper plus `source`
/// (its original absolute path), so a profile survives a moved/deleted image.
const PROFILE_WALLPAPER_DIR: &str = "wallpaper";
const PROFILE_WALLPAPER_SOURCE: &str = "source";

/// Wallpaper belongs to the desktop layout component (plasma appletsrc).
fn tracks_desktop_layout(rel_paths: &[String]) -> bool {
    rel_paths.iter().any(|p| p == APPLETSRC_REL)
}

/// Copy the wallpaper Plasma shows into the snapshot being saved.
fn capture_wallpaper(target: &Path, rel_paths: &[String]) -> Result<()> {
    if !tracks_desktop_layout(rel_paths) {
        return Ok(());
    }
    let Some(current) = plasma_wallpaper_from(&live_path(APPLETSRC_REL)) else {
        return Ok(());
    };
    let source = PathBuf::from(&current);
    let Some(name) = source.file_name() else {
        return Ok(());
    };
    if !source.is_file() {
        return Ok(());
    }
    let dir = target.join(PROFILE_WALLPAPER_DIR);
    fs::create_dir_all(&dir)?;
    fs::copy(&source, dir.join(name))
        .with_context(|| format!("Failed to copy wallpaper {}", source.display()))?;
    fs::write(dir.join(PROFILE_WALLPAPER_SOURCE), format!("{current}\n"))?;
    Ok(())
}

/// The wallpaper file a profile wants, restoring the saved copy when the
/// original is gone (back to its old path, else into [`wallpaper_dir`]; an
/// existing file is never overwritten). Falls back to `theme-bundle.json`'s
/// `wallpaper` path for profiles saved before copies existed.
pub fn profile_wallpaper(profile_dir: &Path) -> Option<PathBuf> {
    profile_wallpaper_in(profile_dir, &wallpaper_dir())
}

/// [`profile_wallpaper`] with an explicit wallpaper folder (tests).
fn profile_wallpaper_in(profile_dir: &Path, walls: &Path) -> Option<PathBuf> {
    let dir = profile_dir.join(PROFILE_WALLPAPER_DIR);
    let original = fs::read_to_string(dir.join(PROFILE_WALLPAPER_SOURCE))
        .ok()
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| !p.as_os_str().is_empty());
    let copy = fs::read_dir(&dir).ok().and_then(|entries| {
        entries
            .flatten()
            .map(|e| e.path())
            .find(|p| p.is_file() && p.file_name().is_some_and(|n| n != PROFILE_WALLPAPER_SOURCE))
    });

    if let Some(original) = original.as_ref().filter(|p| p.is_file()) {
        return Some(original.clone());
    }
    if let Some(copy) = copy {
        let name = copy.file_name()?.to_owned();
        // The original path only when its folder exists here: a theme from
        // another machine names e.g. ~/Изображения/Wallpaper — don't create
        // a localised folder, use this machine's wallpaper folder instead.
        let original = original.filter(|p| p.is_file() || p.parent().is_some_and(Path::is_dir));
        for target in [original, Some(walls.join(&name))].into_iter().flatten() {
            if target.is_file() {
                return Some(target);
            }
            let parent_ok = target.parent().is_some_and(|p| fs::create_dir_all(p).is_ok());
            if parent_ok && fs::copy(&copy, &target).is_ok() {
                return Some(target);
            }
        }
        return Some(copy);
    }
    bundle_wallpaper(profile_dir)
}

fn bundle_wallpaper(profile_dir: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(profile_dir.join("theme-bundle.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let path = PathBuf::from(json.get("wallpaper")?.as_str()?);
    path.is_file().then_some(path)
}

/// `plasma-apply-wallpaperimage` needs plasmashell's D-Bus API, which can lag
/// a moment behind a restart — retry briefly instead of failing silently.
pub(crate) fn apply_wallpaper_live(path: &Path) {
    if no_apply() {
        return;
    }
    for attempt in 0..5 {
        let applied = Command::new("plasma-apply-wallpaperimage")
            .arg(path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if applied {
            return;
        }
        if attempt < 4 {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    eprintln!("Warning: could not apply wallpaper {}", path.display());
}

// ─────────────────────────────────────────────────────────────────────────────
// Active Kvantum theme inside profiles
// ─────────────────────────────────────────────────────────────────────────────

const KVANTUM_CONFIG_REL: &str = ".config/Kvantum/kvantum.kvconfig";

/// User Kvantum theme named by a `kvantum.kvconfig` (None for system themes,
/// which are not copied, and for unsafe names).
fn kvantum_theme_rel(kvantum_config: &Path) -> Option<String> {
    let theme = ini_group_key(kvantum_config, "General", "theme")?;
    let theme = theme.trim();
    if theme.is_empty() || theme.contains('/') || theme.starts_with('.') {
        return None;
    }
    Some(format!(".config/Kvantum/{theme}"))
}

/// Copy the *active* user Kvantum theme directory into the snapshot, so
/// profiles using a generated (`ThemeProfile*`) or downloaded theme are
/// self-contained. Tied to the kvantum component.
fn capture_active_kvantum_theme(target: &Path, rel_paths: &[String]) -> Result<()> {
    if !rel_paths.iter().any(|p| p == KVANTUM_CONFIG_REL) {
        return Ok(());
    }
    let Some(rel) = kvantum_theme_rel(&live_path(KVANTUM_CONFIG_REL)) else {
        return Ok(());
    };
    let source = live_path(&rel);
    let dest = target.join(&rel);
    if !source.is_dir() || dest.exists() {
        return Ok(()); // system theme, or already tracked explicitly
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    copy_tree(&source, &dest).with_context(|| format!("Failed to copy Kvantum theme {}", source.display()))
}

/// Put the profile's Kvantum theme directory back (merge, like any restore).
fn restore_active_kvantum_theme(profile_dir: &Path, rel_paths: &[String]) -> Result<()> {
    if !rel_paths.iter().any(|p| p == KVANTUM_CONFIG_REL) {
        return Ok(());
    }
    let Some(rel) = kvantum_theme_rel(&profile_dir.join(KVANTUM_CONFIG_REL)) else {
        return Ok(());
    };
    let saved = profile_dir.join(&rel);
    if !saved.is_dir() || rel_paths.contains(&rel) {
        return Ok(());
    }
    let live = live_path(&rel);
    if let Some(parent) = live.parent() {
        fs::create_dir_all(parent)?;
    }
    copy_tree(&saved, &live)
}

// ─────────────────────────────────────────────────────────────────────────────
// Save
// ─────────────────────────────────────────────────────────────────────────────

/// Save the current live state as a profile (atomic swap, pre-save backup).
pub fn save_profile_impl(profile: &str, components: Option<Vec<String>>) -> Result<()> {
    validate_profile_name(profile)?;
    reject_case_duplicate(profile)?;
    let rel_paths = resolve_components(components)?;
    if rel_paths.is_empty() {
        return Err(anyhow!("No valid components selected"));
    }
    save_with_paths(profile, &rel_paths)
}

/// CLI variant: bash-compatible component selection (warn + skip unknown).
pub fn save_profile_cli(profile: &str, components: Option<Vec<String>>) -> Result<()> {
    validate_profile_name(profile)?;
    reject_case_duplicate(profile)?;
    let rel_paths = resolve_cli_paths(components)?;
    if rel_paths.is_empty() {
        return Err(anyhow!("No valid components selected"));
    }
    save_with_paths(profile, &rel_paths)
}

fn save_with_paths(profile: &str, rel_paths: &[String]) -> Result<()> {
    let profiles = theme_profiles_dir();
    fs::create_dir_all(&profiles)?;
    let tmp = unique_dir(&profiles, &format!(".save-{profile}"))?;
    let built = split_live_hosts(rel_paths)
        .and(copy_tracked_appearance(&tmp, rel_paths))
        .and(capture_derived(&tmp, rel_paths))
        .and(capture_wallpaper(&tmp, rel_paths))
        .and(capture_active_kvantum_theme(&tmp, rel_paths))
        // No metadata/kde diagnostics in a theme: they describe this machine
        // (pre-apply backups still keep them for recovery).
        .and(write_manifest(&tmp, profile, rel_paths));

    if let Err(e) = built {
        let _ = fs::remove_dir_all(&tmp);
        return Err(e.context(format!("Failed to save profile: {profile}")));
    }

    let target = profiles.join(profile);
    let backup: Option<PathBuf> = if fs::symlink_metadata(&target).is_ok() {
        let backups = backups_dir();
        fs::create_dir_all(&backups)?;
        let dir = unique_dir(&backups, &format!("pre-save-{profile}"))?;
        fs::rename(&target, &dir)?;
        Some(dir)
    } else {
        None
    };

    if let Err(e) = fs::rename(&tmp, &target) {
        let _ = fs::remove_dir_all(&tmp);
        if let Some(b) = backup {
            let _ = fs::rename(b, &target);
        }
        return Err(anyhow!("Failed to activate the new profile snapshot: {profile}: {e}"));
    }

    write_active(profile)?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Delete / diff
// ─────────────────────────────────────────────────────────────────────────────

/// Delete a profile and clear the active marker when it pointed at it.
pub fn delete_profile(profile_name: &str) -> Result<()> {
    let profile_dir = safe_profile_dir(profile_name)?;
    if !profile_dir.exists() && !profile_dir.is_symlink() {
        return Err(anyhow!("Profile not found: {profile_name}"));
    }
    if profile_dir.is_symlink() || !profile_dir.is_dir() {
        fs::remove_file(&profile_dir)?;
    } else {
        fs::remove_dir_all(&profile_dir)?;
    }
    let marker = active_file();
    if let Ok(active) = fs::read_to_string(&marker) {
        if active.trim() == profile_name {
            let _ = fs::remove_file(&marker);
        }
    }
    Ok(())
}

/// Clone a profile snapshot into a new profile (`theme-profile clone A B`).
///
/// Files are copied with `cp -a` semantics; `manifest.json` is rewritten with
/// the new name. The snapshot content, description and the original
/// `created_at` are kept — the clone IS the same theme.
/// Existing profile whose name equals `name` ignoring case but not exactly
/// (`paper` vs `Paper`): such pairs look identical in the UI and the app
/// theme follows names case-insensitively.
pub fn case_duplicate_of(name: &str) -> Option<String> {
    let entries = fs::read_dir(theme_profiles_dir()).ok()?;
    entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .find(|existing| existing != name && existing.eq_ignore_ascii_case(name))
}

pub(crate) fn reject_case_duplicate(name: &str) -> Result<()> {
    match case_duplicate_of(name) {
        Some(existing) => Err(anyhow!(
            "Profile \"{existing}\" already exists — names may not differ only by case \
             (save as \"{existing}\" to overwrite it, or choose another name)"
        )),
        None => Ok(()),
    }
}

pub fn clone_profile(source: &str, target: &str) -> Result<()> {
    validate_profile_name(source)?;
    validate_profile_name(target)?;
    let src = safe_profile_dir(source)?;
    if !src.is_dir() {
        return Err(anyhow!("Profile not found: {source}"));
    }
    let dst = safe_profile_dir(target)?;
    if dst.symlink_metadata().is_ok() {
        return Err(anyhow!("Profile already exists: {target}"));
    }
    reject_case_duplicate(target)?;
    copy_tree(&src, &dst)?;
    if crate::theme::is_package(&dst) {
        return crate::theme::rename_manifest(&dst, target);
    }
    let mut manifest = read_manifest(&dst);
    manifest.name = target.to_string();
    write_json_manifest(&dst, &manifest)
}

/// Diff two profiles. Uses the system `diff` (coreutils) for a regular
/// recursive unified diff — the same output both cores already agree on.
/// Exit status 1 (differences found) is a normal result, not an error.
pub fn diff_profiles(profile_a: &str, profile_b: &str) -> Result<String> {
    validate_profile_name(profile_a)?;
    validate_profile_name(profile_b)?;
    let dir_a = safe_profile_dir(profile_a)?;
    let dir_b = safe_profile_dir(profile_b)?;
    if !dir_a.is_dir() {
        return Err(anyhow!("Profile not found: {profile_a}"));
    }
    if !dir_b.is_dir() {
        return Err(anyhow!("Profile not found: {profile_b}"));
    }

    let output = Command::new("diff")
        .args(["-ru", "--"])
        .arg(&dir_a)
        .arg(&dir_b)
        .output()
        .context("Failed to run diff")?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    match output.status.code() {
        Some(0) | Some(1) => Ok(stdout),
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(anyhow!(if stderr.is_empty() { stdout } else { stderr }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{keep_desktop_icons, theme_file, validate_reload_command};

    #[test]
    fn browser_profiles_map_across_machines() {
        let t = tempfile::tempdir().unwrap();
        let theme = t.path();
        std::fs::create_dir_all(theme.join(".zen/abc.Default (release)/chrome")).unwrap();
        std::fs::write(theme.join(".zen/abc.Default (release)/user.js"), "x").unwrap();
        // The receiver's profile folder is named differently.
        assert_eq!(theme_file(theme, ".zen/xyz.Default (release)/user.js"), theme.join(".zen/abc.Default (release)/user.js"));
        assert_eq!(theme_file(theme, ".zen/xyz.Default (release)/chrome/none.css"), theme.join(".zen/xyz.Default (release)/chrome/none.css"));
        assert_eq!(theme_file(theme, ".config/kdeglobals"), theme.join(".config/kdeglobals"));
    }

    #[test]
    fn theme_apply_keeps_live_desktop_icons() {
        let theme = "[Containments][1][General]\npositions=OLD\nlastResolution=1x1\nsortMode=-1\n\n[Containments][2]\nplugin=org.kde.panel\nthickness=40\n\n[ScreenMapping]\nscreenMapping=OLD\n";
        let live = "[Containments][1][General]\npositions=NEW\nlastResolution=2x2\nsortMode=5\n\n[Containments][2]\nthickness=99\n\n[ScreenMapping]\nitemsOnDisabledScreens=NEW\nscreenMapping=NEW\n";
        let out = keep_desktop_icons(theme, live);
        // Icons from the live file …
        assert!(out.contains("positions=NEW") && out.contains("lastResolution=2x2"));
        assert!(out.contains("screenMapping=NEW") && out.contains("itemsOnDisabledScreens=NEW"));
        assert!(!out.contains("OLD"));
        // … everything else (panel, sorting) from the theme.
        assert!(out.contains("thickness=40") && out.contains("sortMode=-1"));
        assert!(!out.contains("thickness=99") && !out.contains("sortMode=5"));
        // A live icon group the theme lacks is added.
        let out = keep_desktop_icons("[Containments][2]\nthickness=40\n", live);
        assert!(out.contains("[Containments][1][General]\npositions=NEW"));
    }

    #[test]
    fn reload_command_accepts_binary_with_args() {
        // `sh` exists on any Unix CI image; validation never runs the command.
        let argv = validate_reload_command("sh -c echo").expect("sh must resolve via PATH");
        assert_eq!(argv, vec!["sh", "-c", "echo"]);
    }

    #[test]
    fn reload_command_rejects_shell_metacharacters() {
        for bad in [
            "echo a|b",
            "echo a&b",
            "echo a;b",
            "echo $(id)",
            "echo `id`",
            "echo \"x\"",
            "echo 'x'",
            "echo a>b",
            "echo a<b",
        ] {
            assert!(
                validate_reload_command(bad).is_err(),
                "{bad:?} must be rejected"
            );
        }
    }

    #[test]
    fn reload_command_rejects_empty_and_unknown_binary() {
        assert!(validate_reload_command("").is_err());
        assert!(validate_reload_command("   ").is_err());
        assert!(
            validate_reload_command("definitely-not-a-real-binary-ktm --x").is_err()
        );
    }
}

#[cfg(test)]
mod wallpaper_tests {
    use super::*;

    #[test]
    fn profile_wallpaper_prefers_original_then_restores_copy() {
        let tmp = tempfile::tempdir().unwrap();
        let profile = tmp.path().join("profile");
        let saved = profile.join(PROFILE_WALLPAPER_DIR);
        fs::create_dir_all(&saved).unwrap();
        let original = tmp.path().join("walls/forest.jpg");
        fs::create_dir_all(original.parent().unwrap()).unwrap();
        fs::write(saved.join("forest.jpg"), b"jpeg-bytes").unwrap();
        fs::write(saved.join(PROFILE_WALLPAPER_SOURCE), format!("{}\n", original.display())).unwrap();

        // Original missing (its folder exists) → the copy is put back where it was.
        assert_eq!(profile_wallpaper(&profile), Some(original.clone()));
        assert_eq!(fs::read(&original).unwrap(), b"jpeg-bytes");

        // Original present (even if it changed) → used as is, never overwritten.
        fs::write(&original, b"edited").unwrap();
        assert_eq!(profile_wallpaper(&profile), Some(original.clone()));
        assert_eq!(fs::read(&original).unwrap(), b"edited");
    }

    #[test]
    fn profile_wallpaper_never_creates_a_foreign_folder() {
        // A theme from another machine names e.g. ~/Изображения/Wallpaper:
        // the copy goes to this machine's wallpaper folder instead.
        let tmp = tempfile::tempdir().unwrap();
        let profile = tmp.path().join("profile");
        let saved = profile.join(PROFILE_WALLPAPER_DIR);
        fs::create_dir_all(&saved).unwrap();
        let foreign = tmp.path().join("no-such-folder/Wallpaper/sea.jpg");
        fs::write(saved.join("sea.jpg"), b"jpeg").unwrap();
        fs::write(saved.join(PROFILE_WALLPAPER_SOURCE), format!("{}\n", foreign.display())).unwrap();
        let walls = tmp.path().join("my-walls");
        assert_eq!(profile_wallpaper_in(&profile, &walls), Some(walls.join("sea.jpg")));
        assert!(!tmp.path().join("no-such-folder").exists());
    }

    #[test]
    fn profile_wallpaper_falls_back_to_bundle_path() {
        let tmp = tempfile::tempdir().unwrap();
        let profile = tmp.path().join("old-profile");
        fs::create_dir_all(&profile).unwrap();
        let image = tmp.path().join("bundle.png");
        fs::write(&image, b"png").unwrap();
        fs::write(
            profile.join("theme-bundle.json"),
            format!("{{\"wallpaper\": \"{}\"}}", image.display()),
        )
        .unwrap();
        assert_eq!(profile_wallpaper(&profile), Some(image.clone()));
        fs::remove_file(&image).unwrap();
        assert_eq!(profile_wallpaper(&profile), None);
    }
}
