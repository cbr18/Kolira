//! Wallpaper Core Implementation

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const THUMB_WIDTH: &str = "420";
const THUMB_HEIGHT: &str = "260";
const THUMB_RADIUS: u32 = 18;
const THUMB_VERSION: &str = "rounded-v1";
pub(crate) const IMAGE_PLUGIN: &str = "org.kde.image";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct DesktopWallpaper {
    pub plugin: String,
    pub image: Option<String>,
}

pub(crate) type DesktopWallpapers = BTreeMap<String, DesktopWallpaper>;

impl DesktopWallpaper {
    fn image_path(&self) -> Option<String> {
        self.image.as_deref().map(image_path)
    }
}

pub(crate) fn image_path(value: &str) -> String {
    percent_decode(value.strip_prefix("file://").unwrap_or(value))
}

pub(crate) fn draft_image(path: &str) -> Result<String> {
    let path = Path::new(path);
    anyhow::ensure!(
        path.is_absolute() && path.is_file(),
        "Wallpaper file is unavailable"
    );
    let canonical = fs::canonicalize(path).context("Could not read wallpaper")?;
    // The file picker permits other formats supported by Plasma.
    anyhow::ensure!(canonical.is_file(), "Wallpaper file is unavailable");
    let path = canonical.to_str().context("Invalid wallpaper path")?;
    anyhow::ensure!(
        !path.chars().any(char::is_control),
        "Invalid wallpaper path"
    );
    Ok(path.to_owned())
}

pub(crate) fn image_url(path: &str) -> String {
    use std::fmt::Write;
    let mut url = String::from("file://");
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-_.~".contains(&byte) {
            url.push(char::from(byte));
        } else {
            write!(&mut url, "%{byte:02X}").expect("write into string");
        }
    }
    url
}

pub(crate) fn draft_layout_path() -> Result<PathBuf> {
    let path = crate::paths::live_path(APPLETSRC_REL);
    crate::app_sync::validate_live(&path)?;
    anyhow::ensure!(
        fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_file()),
        "Plasma desktop settings are unavailable"
    );
    Ok(path)
}

fn stored_wallpapers(path: &Path) -> Result<DesktopWallpapers> {
    Ok(crate::layout::desktop_wallpapers(&fs::read_to_string(
        path,
    )?))
}

fn plasma_script(script: &str) -> Result<Vec<u8>> {
    let (status, output) = crate::paths::output_bounded_for(
        Command::new("qdbus6")
            .args([
                "org.kde.plasmashell",
                "/PlasmaShell",
                "org.kde.PlasmaShell.evaluateScript",
                script,
            ])
            .stderr(std::process::Stdio::null()),
        std::time::Duration::from_secs(8),
    )
    .context("Plasma did not respond while changing wallpaper")?;
    anyhow::ensure!(
        status.success(),
        "Plasma could not read or change the wallpaper"
    );
    Ok(output)
}

pub(crate) fn observe_draft() -> Result<DesktopWallpapers> {
    let path = draft_layout_path()?;
    let wallpapers = if crate::field::no_apply() {
        stored_wallpapers(&path)?
    } else {
        let output = plasma_script(
            r#"
            const result = {};
            for (const desktop of desktops()) {
                const plugin = desktop.wallpaperPlugin || "org.kde.image";
                desktop.currentConfigGroup = ["Wallpaper", "org.kde.image", "General"];
                const image = plugin === "org.kde.image" ? desktop.readConfig("Image", "") : "";
                result[String(desktop.id)] = { plugin: plugin, image: image || null };
            }
            print(JSON.stringify(result));
        "#,
        )?;
        serde_json::from_slice(&output).context("Could not read the actual Plasma wallpaper")?
    };
    anyhow::ensure!(
        !wallpapers.is_empty(),
        "Plasma has no desktop available for wallpaper"
    );
    Ok(wallpapers)
}

pub(crate) fn matches_image(wallpapers: &DesktopWallpapers, path: &str) -> bool {
    !wallpapers.is_empty()
        && wallpapers.values().all(|wallpaper| {
            wallpaper.plugin == IMAGE_PLUGIN && wallpaper.image_path().as_deref() == Some(path)
        })
}

pub(crate) fn same_wallpapers(left: &DesktopWallpapers, right: &DesktopWallpapers) -> bool {
    left.len() == right.len()
        && left.iter().all(|(id, wallpaper)| {
            right.get(id).is_some_and(|other| {
                wallpaper.plugin == other.plugin && wallpaper.image_path() == other.image_path()
            })
        })
}

pub(crate) fn describe_wallpapers(wallpapers: &DesktopWallpapers) -> String {
    wallpapers
        .values()
        .map(|wallpaper| {
            wallpaper
                .image_path()
                .and_then(|path| {
                    Path::new(&path)
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| wallpaper.plugin.clone())
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(" · ")
}

pub(crate) fn wait_for_wallpaper_config(path: &Path, expected: &DesktopWallpapers) -> Result<()> {
    use std::time::{Duration, Instant};
    // Plasma coalesces configuration sync requests for up to ten seconds.
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        if stored_wallpapers(path).is_ok_and(|stored| {
            expected.iter().all(|(id, wanted)| {
                stored.get(id).is_some_and(|actual| {
                    wanted.plugin == actual.plugin && wanted.image_path() == actual.image_path()
                })
            })
        }) {
            return Ok(());
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "Plasma did not save the wallpaper change"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn update_runtime(wallpapers: &DesktopWallpapers) -> Result<()> {
    let selection = serde_json::to_string(wallpapers)?;
    let script = format!(
        r#"
        const selection = {selection};
        const applied = [];
        for (const desktop of desktops()) {{
            const id = String(desktop.id);
            const next = selection[id];
            if (!next) continue;
            desktop.wallpaperPlugin = next.plugin;
            if (next.plugin === "org.kde.image") {{
                desktop.currentConfigGroup = ["Wallpaper", "org.kde.image", "General"];
                desktop.writeConfig("Image", next.image || "");
            }}
            applied.push(id);
        }}
        print(JSON.stringify(applied));
    "#
    );
    let ids: Vec<String> = serde_json::from_slice(&plasma_script(&script)?)
        .context("Could not verify Plasma wallpaper targets")?;
    anyhow::ensure!(
        wallpapers.keys().all(|id| ids.contains(id)),
        "Plasma desktops changed while applying wallpaper"
    );
    Ok(())
}

pub(crate) fn apply_draft_image(
    path: &Path,
    image: &str,
    desktops: &DesktopWallpapers,
) -> Result<()> {
    let url = image_url(image);
    if crate::field::no_apply() {
        let text = crate::layout::with_wallpaper_image(&fs::read_to_string(path)?, &url);
        crate::gui_api::write_bytes(path, text.as_bytes(), None)?;
    } else {
        let desired = desktops
            .keys()
            .map(|id| {
                (
                    id.clone(),
                    DesktopWallpaper {
                        plugin: IMAGE_PLUGIN.into(),
                        image: Some(url.clone()),
                    },
                )
            })
            .collect();
        update_runtime(&desired)?;
        wait_for_wallpaper_config(path, &desired)?;
    }
    Ok(())
}

pub(crate) fn refresh_after_restore() -> Result<()> {
    if crate::field::no_apply() {
        return Ok(());
    }
    let path = crate::paths::live_path(APPLETSRC_REL);
    if !path.is_file() {
        return Ok(());
    }
    let current = observe_draft()?;
    let mut restored = stored_wallpapers(&path)?;
    restored.retain(|id, _| current.contains_key(id));
    if !restored.is_empty() {
        update_runtime(&restored)?;
        let mut observed = observe_draft()?;
        observed.retain(|id, _| restored.contains_key(id));
        anyhow::ensure!(
            same_wallpapers(&restored, &observed),
            "Plasma did not restore the prior wallpaper"
        );
        wait_for_wallpaper_config(&path, &restored)?;
    }
    Ok(())
}

/// Deterministic 64-bit FNV-1a (cache keys don't leave the machine).
fn fnv1a64(input: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in input {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

fn thumb_path(thumb_dir: &Path, file: &str) -> PathBuf {
    let mut key = file.as_bytes().to_vec();
    key.push(0);
    key.extend_from_slice(THUMB_VERSION.as_bytes());
    let hash = fnv1a64(&key);
    thumb_dir.join(format!("{hash:016x}.png"))
}

fn file_newer_than(file: &str, thumb: &Path) -> bool {
    let file_mtime = fs::metadata(file).and_then(|m| m.modified()).ok();
    let thumb_mtime = fs::metadata(thumb).and_then(|m| m.modified()).ok();
    match (file_mtime, thumb_mtime) {
        (Some(f), Some(t)) => f > t,
        _ => true,
    }
}

/// Generate a rounded-corner thumbnail with ImageMagick (best effort).
fn make_thumb(file: &str, thumb: &Path) -> bool {
    let width: u32 = THUMB_WIDTH.parse().unwrap_or(0);
    let height: u32 = THUMB_HEIGHT.parse().unwrap_or(0);
    let status = crate::paths::status_bounded(
        Command::new("magick")
            .arg(file)
            .args(["-thumbnail", &format!("{THUMB_WIDTH}x{THUMB_HEIGHT}^")])
            .args(["-gravity", "center"])
            .args(["-extent", &format!("{THUMB_WIDTH}x{THUMB_HEIGHT}")])
            .arg("(")
            .args(["-size", &format!("{THUMB_WIDTH}x{THUMB_HEIGHT}"), "xc:none"])
            .args(["-fill", "white"])
            .arg("-draw")
            .arg(format!(
                "roundrectangle 0,0,{},{},{},{}",
                width.saturating_sub(1),
                height.saturating_sub(1),
                THUMB_RADIUS,
                THUMB_RADIUS
            ))
            .arg(")")
            .args([
                "-alpha",
                "set",
                "-compose",
                "DstIn",
                "-composite",
                thumb.to_string_lossy().as_ref(),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null()),
    );
    status.is_some_and(|s| s.success())
}

/// Apply one image through the same core for GUI and all wallpaper binaries.
pub fn apply_image(path: &str) -> Result<()> {
    if crate::field::no_apply() || std::env::var_os("WALLPAPER_DRY_RUN").is_some() {
        return Ok(());
    }
    let _lock = crate::operation::MutationGuard::acquire()?;
    if crate::paths::status_bounded(
        std::process::Command::new("plasma-apply-wallpaperimage").arg(path),
    )
    .is_some_and(|s| s.success())
    {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Failed to apply wallpaper (or command timed out): {path}"
        ))
    }
}

/// Outcome of the interactive picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickOutcome {
    /// wofi was cancelled (empty selection) — nothing to do.
    Cancelled,
    /// A wallpaper was chosen (its full-list index is already cached).
    Selected(String),
}

/// Full picker flow: thumbnails -> wofi grid -> cache the selection.
///
/// The `wallpaper-picker` binary wires the environment (directories, style)
/// and applies the chosen file, so there is exactly one implementation of the
/// picker logic.
pub fn pick_wallpaper_flow(
    dir: &Path,
    state: &Path,
    thumb_dir: &Path,
    style: &Path,
) -> Result<PickOutcome> {
    if crate::field::no_apply() {
        return Ok(PickOutcome::Cancelled);
    }
    let files = list_wallpapers_in(dir)?;
    if files.is_empty() {
        return Err(anyhow::anyhow!("No wallpapers found"));
    }

    fs::create_dir_all(thumb_dir)?;

    let mut entries = String::new();
    let mut displayed: Vec<String> = Vec::new();

    for file in &files {
        let name = Path::new(file)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| file.clone());
        let thumb = thumb_path(thumb_dir, file);
        if (!thumb.is_file() || file_newer_than(file, &thumb)) && !make_thumb(file, &thumb) {
            let _ = fs::remove_file(&thumb);
            continue;
        }
        displayed.push(file.clone());
        entries.push_str(&format!("img:{}:text:{}\n", thumb.display(), name));
    }

    if displayed.is_empty() {
        return Err(anyhow::anyhow!("No displayable wallpapers"));
    }

    let mut wofi = Command::new("wofi")
        .args([
            "--dmenu",
            "--allow-images",
            "--insensitive",
            "--width",
            "40%",
            "--height",
            "60%",
            "--columns",
            "4",
            "--define",
            "image_size=210",
            "--define",
            "content_halign=center",
            "--define",
            "dmenu-print_line_num=true",
            "--style",
            style.to_string_lossy().as_ref(),
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .context("could not start wofi")?;

    if let Some(mut stdin) = wofi.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(entries.as_bytes());
    }

    let output = wofi.wait_with_output()?;
    let selection = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if selection.is_empty() {
        return Ok(PickOutcome::Cancelled);
    }
    if !selection.bytes().all(|b| b.is_ascii_digit()) {
        // A typed custom entry is not a position.
        return Err(anyhow::anyhow!("Invalid selection: not a position"));
    }
    let index: usize = selection
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid selection"))?;
    let Some(selected) = displayed.get(index) else {
        return Err(anyhow::anyhow!("Selection out of range"));
    };

    // Cache the position in the FULL enumeration (the displayed list can be a
    // subset when a thumbnail failed to render).
    if let Some(full_index) = files.iter().position(|f| f == selected) {
        write_cache_index(state, full_index)?;
    }

    Ok(PickOutcome::Selected(selected.clone()))
}

/// Step the wallpaper index relative to the anchor (Plasma's current file,
/// else the cached index, else 0) and persist the new position.
pub fn step_index(
    files: &[String],
    state: &Path,
    plasma: Option<&Path>,
    delta: i64,
) -> Result<usize> {
    let len = files.len();
    if len == 0 {
        return Err(anyhow::anyhow!("No wallpapers found"));
    }
    let anchor = anchor_index(files, state, plasma);
    let index = ((anchor as i64 + delta).rem_euclid(len as i64)) as usize;
    write_cache_index(state, index)?;
    Ok(index)
}

/// Plasma's desktop layout file, relative to `$HOME` (profile snapshot layout).
pub const APPLETSRC_REL: &str = ".config/plasma-org.kde.plasma.desktop-appletsrc";

/// Wallpaper directory used by the picker, next/prev and the GUI.
///
/// Order: `$WALLPAPER_DIR` → GUI setting `wallpaper_directory` in
/// `app-preferences.json` → `$(xdg-user-dir PICTURES)/Wallpaper` →
/// `~/Pictures/Wallpaper`. Never a hard-coded localised folder name.
pub fn wallpaper_dir() -> PathBuf {
    wallpaper_dir_from(
        std::env::var_os("WALLPAPER_DIR"),
        &crate::paths::settings_file(),
        &crate::paths::config_home().join("user-dirs.dirs"),
        &crate::paths::home_dir(),
    )
}

/// GUI's explicit directory choice wins; otherwise use the shared resolver.
pub fn gui_wallpaper_dir() -> PathBuf {
    settings_wallpaper_dir(&crate::paths::settings_file(), &crate::paths::home_dir())
        .unwrap_or_else(wallpaper_dir)
}

/// Pure resolution behind [`wallpaper_dir`] (testable without touching `$HOME`).
pub fn wallpaper_dir_from(
    env: Option<std::ffi::OsString>,
    settings: &Path,
    user_dirs: &Path,
    home: &Path,
) -> PathBuf {
    if let Some(value) = env.filter(|v| !v.is_empty()) {
        return expand_home(&value.to_string_lossy(), home);
    }
    if let Some(dir) = settings_wallpaper_dir(settings, home) {
        return dir;
    }
    if let Some(pictures) = xdg_pictures_dir(user_dirs, home) {
        return pictures.join("Wallpaper");
    }
    home.join("Pictures").join("Wallpaper")
}

fn expand_home(value: &str, home: &Path) -> PathBuf {
    if value == "~" || value == "$HOME" {
        return home.to_path_buf();
    }
    for prefix in ["~/", "$HOME/"] {
        if let Some(rest) = value.strip_prefix(prefix) {
            return home.join(rest);
        }
    }
    PathBuf::from(value)
}

fn settings_wallpaper_dir(settings: &Path, home: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(settings).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let value = json.get("wallpaper_directory")?.as_str()?.trim();
    if value.is_empty() {
        None
    } else {
        Some(expand_home(value, home))
    }
}

/// `XDG_PICTURES_DIR` from `user-dirs.dirs` (None when unset or equal to `$HOME`,
/// which the XDG spec uses to mean "disabled").
fn xdg_pictures_dir(user_dirs: &Path, home: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(user_dirs).ok()?;
    for line in text.lines() {
        let Some(value) = line.trim().strip_prefix("XDG_PICTURES_DIR=") else {
            continue;
        };
        let value = value.trim().trim_matches('"');
        let path = expand_home(value, home);
        if path.as_os_str().is_empty() || path == home {
            return None;
        }
        return Some(path);
    }
    None
}

/// Path of the wallpaper Plasma is showing right now (None — unreadable or not
/// an `org.kde.image` wallpaper), read from an explicit appletsrc path.
///
/// One answer for every caller (next/prev, GUI, profile save/apply): the
/// `Image=` of the desktop containment on the *primary* screen (`lastScreen=0`,
/// else the lowest screen number); among several containments for that screen
/// (activities) the last one in the file wins. Plugins other than
/// `org.kde.image` (slideshow, POTD) have no image group and are ignored.
pub fn plasma_wallpaper_from(config: &Path) -> Option<String> {
    let text = fs::read_to_string(config).ok()?;

    #[derive(Default)]
    struct Containment {
        screen: Option<i64>,
        image: Option<String>,
        image_seq: usize,
    }
    let mut containments: std::collections::BTreeMap<String, Containment> = Default::default();
    let mut current: Option<(String, bool)> = None; // (containment id, in image group)
    let mut seq = 0usize; // position of each Image= line: later lines win ties

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            current = None;
            let Some(rest) = trimmed.strip_prefix("[Containments][") else {
                continue;
            };
            let Some(end) = rest.find(']') else { continue };
            let id = rest[..end].to_string();
            let tail = &rest[end + 1..];
            if tail.is_empty() {
                current = Some((id, false));
            } else if tail == "[Wallpaper][org.kde.image][General]" {
                current = Some((id, true));
            }
            continue;
        }
        let Some((id, in_image)) = current.as_ref() else {
            continue;
        };
        let entry = containments.entry(id.clone()).or_default();
        if *in_image {
            if let Some(value) = trimmed.strip_prefix("Image=") {
                seq += 1;
                entry.image = Some(value.trim().to_string());
                entry.image_seq = seq;
            }
        } else if let Some(value) = trimmed.strip_prefix("lastScreen=") {
            entry.screen = value.trim().parse::<i64>().ok().filter(|n| *n >= 0);
        }
    }

    let url = containments
        .values()
        .filter(|c| c.image.as_deref().is_some_and(|i| !i.is_empty()))
        .min_by(|a, b| {
            let sa = a.screen.unwrap_or(i64::MAX);
            let sb = b.screen.unwrap_or(i64::MAX);
            sa.cmp(&sb).then(b.image_seq.cmp(&a.image_seq))
        })?
        .image
        .clone()?;

    let path = image_path(&url);
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

/// Decode %XX escapes (Plasma stores file URLs percent-encoded)
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(value) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(value);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// List wallpapers in an explicit directory (CLI: honours `WALLPAPER_DIR`).
pub fn list_wallpapers_in(dir: &Path) -> Result<Vec<String>> {
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut wallpapers = Vec::new();
    let extensions = ["jpg", "jpeg", "png", "webp", "bmp"];

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext = ext.to_string_lossy().to_lowercase();
                if extensions.contains(&ext.as_str()) {
                    wallpapers.push(path.to_string_lossy().to_string());
                }
            }
        }
    }

    wallpapers.sort();
    Ok(wallpapers)
}

/// Starting index for next/prev: the wallpaper Plasma actually shows, else the
/// cached index, else 0. Mirrors the historical bash scripts exactly.
pub fn anchor_index(files: &[String], state: &Path, plasma: Option<&Path>) -> usize {
    let mut index: Option<usize> = None;
    if let Some(config) = plasma {
        if let Some(current) = plasma_wallpaper_from(config) {
            index = files.iter().position(|f| f == &current);
        }
    }
    if index.is_none() {
        index = fs::read_to_string(state)
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok());
    }
    index.unwrap_or(0)
}

/// Write the cached index (`mkdir -p` + `echo > state`).
pub fn write_cache_index(state: &Path, index: usize) -> Result<()> {
    if let Some(parent) = state.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(state, format!("{index}\n"))?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn wallpaper_dir_resolution_order() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let settings = home.join("settings.json");
        let user_dirs = home.join("user-dirs.dirs");

        // Nothing configured → ~/Pictures/Wallpaper, not a localised name.
        assert_eq!(
            wallpaper_dir_from(None, &settings, &user_dirs, home),
            home.join("Pictures/Wallpaper")
        );
        // XDG_PICTURES_DIR (localised) → <pictures>/Wallpaper.
        write(&user_dirs, "XDG_PICTURES_DIR=\"$HOME/Изображения\"\n");
        assert_eq!(
            wallpaper_dir_from(None, &settings, &user_dirs, home),
            home.join("Изображения/Wallpaper")
        );
        // GUI setting beats XDG; `~` expands.
        write(&settings, r#"{"wallpaper_directory": "~/Walls"}"#);
        assert_eq!(
            wallpaper_dir_from(None, &settings, &user_dirs, home),
            home.join("Walls")
        );
        // An empty setting means "auto".
        write(&settings, r#"{"wallpaper_directory": ""}"#);
        assert_eq!(
            wallpaper_dir_from(None, &settings, &user_dirs, home),
            home.join("Изображения/Wallpaper")
        );
        // $WALLPAPER_DIR beats everything.
        assert_eq!(
            wallpaper_dir_from(Some("/srv/w".into()), &settings, &user_dirs, home),
            PathBuf::from("/srv/w")
        );
        // XDG_PICTURES_DIR == $HOME means "disabled" per the spec.
        write(&user_dirs, "XDG_PICTURES_DIR=\"$HOME/\"\n");
        fs::remove_file(&settings).unwrap();
        assert_eq!(
            wallpaper_dir_from(None, &settings, &user_dirs, home),
            home.join("Pictures/Wallpaper")
        );
    }

    #[test]
    fn current_wallpaper_is_the_primary_screen() {
        let tmp = tempfile::tempdir().unwrap();
        let rc = tmp.path().join("appletsrc");
        write(
            &rc,
            "[Containments][1]\nlastScreen=1\nwallpaperplugin=org.kde.image\n\n\
             [Containments][1][Wallpaper][org.kde.image][General]\nImage=file:///w/second.jpg\n\n\
             [Containments][2]\nlastScreen=0\n\n\
             [Containments][2][Wallpaper][org.kde.image][General]\nImage=file:///w/pri%20mary.jpg\n\n\
             [Containments][3]\nlastScreen=-1\n\n\
             [Containments][3][Wallpaper][org.kde.image][General]\nImage=file:///w/orphan.jpg\n\n\
             [Containments][4]\nlastScreen=0\nwallpaperplugin=org.kde.slideshow\n\n\
             [Containments][4][Wallpaper][org.kde.slideshow][General]\nSlidePaths=/w\n",
        );
        // Screen 0 wins over file order; the slideshow has no Image.
        assert_eq!(
            plasma_wallpaper_from(&rc).as_deref(),
            Some("/w/pri mary.jpg")
        );

        // Two activities on screen 0: the later Image= wins.
        let mut text = fs::read_to_string(&rc).unwrap();
        text.push_str(
            "\n[Containments][9]\nlastScreen=0\n\n\
             [Containments][9][Wallpaper][org.kde.image][General]\nImage=/w/activity.png\n",
        );
        write(&rc, &text);
        assert_eq!(
            plasma_wallpaper_from(&rc).as_deref(),
            Some("/w/activity.png")
        );
    }

    #[test]
    fn no_image_wallpaper_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        let rc = tmp.path().join("appletsrc");
        write(
            &rc,
            "[Containments][1]\nlastScreen=0\nwallpaperplugin=org.kde.potd\n",
        );
        assert_eq!(plasma_wallpaper_from(&rc), None);
    }
}
