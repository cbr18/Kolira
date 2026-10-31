//! Wallpaper Core Implementation

use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;
use anyhow::{Result, Context};

const THUMB_WIDTH: &str = "420";
const THUMB_HEIGHT: &str = "260";
const THUMB_RADIUS: u32 = 18;
const THUMB_VERSION: &str = "rounded-v1";

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
    let status = Command::new("magick")
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
        .args(["-alpha", "set", "-compose", "DstIn", "-composite", thumb.to_string_lossy().as_ref()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    status.map(|s| s.success()).unwrap_or(false)
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
    let index: usize = selection.parse().map_err(|_| anyhow::anyhow!("Invalid selection"))?;
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

/// Path of the wallpaper Plasma is showing right now (None — unreadable),
/// read from an explicit appletsrc path (the CLI honours `PLASMA_APPLETSRC`).
///
/// Plasma keeps the desktop wallpaper per containment in the appletsrc; it is
/// the authoritative answer to "what is on the desktop". The *last* `Image=`
/// inside a `[Containments][N][Wallpaper][org.kde.image][General]` group wins.
pub fn plasma_wallpaper_from(config: &Path) -> Option<String> {
    let text = fs::read_to_string(config).ok()?;

    let mut url: Option<String> = None;
    let mut in_image_group = false;
    for line in text.lines() {
        if line.starts_with('[') {
            let trimmed = line.trim();
            in_image_group = trimmed.starts_with("[Containments]")
                && trimmed.ends_with("[Wallpaper][org.kde.image][General]");
            continue;
        }
        if in_image_group {
            if let Some(value) = line.strip_prefix("Image=") {
                url = Some(value.trim().to_string());
            }
        }
    }

    let url = url?;
    let path = if let Some(rest) = url.strip_prefix("file://") {
        percent_decode(rest)
    } else {
        percent_decode(&url)
    };
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