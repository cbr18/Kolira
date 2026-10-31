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

use crate::config_parser::{ini_group_key, ini_group_lines};
use crate::paths::{
    active_file, backups_dir, config_home, live_path, profile_dir as safe_profile_dir,
    profiles_dir as theme_profiles_dir, validate_profile_name,
};
use crate::tracked_apps::component_config_paths;
use crate::types::ProfileInfo;

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
    Ok(read_manifest(&dir).description)
}

/// Update a profile's description (rewrites `manifest.json` in place — no
/// other files in the snapshot are touched).
pub fn set_profile_description(profile_name: &str, description: &str) -> Result<()> {
    let dir = safe_profile_dir(profile_name)?;
    if !dir.is_dir() {
        return Err(anyhow!("Profile not found: {profile_name}"));
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

fn unique_dir(root: &Path, prefix: &str) -> Result<PathBuf> {
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
fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
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

fn write_active(name: &str) -> Result<()> {
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
    let socket = "unix:/tmp/kitty-theme-profile";
    let config_str = config.to_string_lossy().to_string();

    run_external("kitty", &["@", "--to", socket, "load-config", &config_str]);

    let opacity = std::fs::read_to_string(&config)
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

    run_external("kitty", &["@", "--to", socket, "set-colors", "--all", "--configured", &config_str]);
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
fn run_custom_reload_hooks(rel_paths: &[String]) {
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

/// Order of qdbus commands matches the historical bash script.
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

fn restore_from_profile(profile: &str, rel_paths: &[String]) -> Result<()> {
    let source = safe_profile_dir(profile)?;
    if !source.is_dir() {
        return Err(anyhow!("Profile not found: {profile}"));
    }

    for rel in rel_paths {
        let src = source.join(rel);
        if !src.exists() {
            continue;
        }
        let dst = live_path(rel);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        copy_tree(&src, &dst).with_context(|| {
            format!("Failed to restore {} -> {}", src.display(), dst.display())
        })?;
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

    match restore_from_profile(profile, rel_paths) {
        Ok(()) => {
            apply_live_hooks(&dir);
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
// Save
// ─────────────────────────────────────────────────────────────────────────────

/// Save the current live state as a profile (atomic swap, pre-save backup).
pub fn save_profile_impl(profile: &str, components: Option<Vec<String>>) -> Result<()> {
    validate_profile_name(profile)?;
    let rel_paths = resolve_components(components)?;
    if rel_paths.is_empty() {
        return Err(anyhow!("No valid components selected"));
    }
    save_with_paths(profile, &rel_paths)
}

/// CLI variant: bash-compatible component selection (warn + skip unknown).
pub fn save_profile_cli(profile: &str, components: Option<Vec<String>>) -> Result<()> {
    validate_profile_name(profile)?;
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
    let built = copy_tracked_into(&tmp, rel_paths)
        .and(capture_kde_effects_state(&tmp))
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
    use super::validate_reload_command;

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
