//! XDG-aware paths and profile-name validation.
//!
//! Themes keep the `.config/...` layout inside, but the live files they mirror
//! must follow `XDG_CONFIG_HOME`/`XDG_DATA_HOME`. Data lives in
//! `$XDG_CONFIG_HOME/kolira`; the old `kde-theme-profile` / `theme-profile` folders are moved
//! there on start by Rust; Python only resolves the canonical path.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

pub fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub fn config_home() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| home_dir().join(".config"))
}

pub fn cache_home() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(|| home_dir().join(".cache"))
}

pub fn data_home() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| home_dir().join(".local/share"))
}

/// Canonical data directory and historical names accepted during migration.
const DATA_DIR: &str = "kolira";
const OLD_DATA_DIRS: &[&str] = &["kde-theme-profile", "theme-profile", "kolira"];

pub fn theme_root() -> PathBuf {
    let preferred = config_home().join(DATA_DIR);
    let in_dot_config = home_dir().join(".config").join(DATA_DIR);
    // Unit tests read this path with the real HOME: they must not move real data.
    if !cfg!(test) {
        let old_roots: Vec<PathBuf> = OLD_DATA_DIRS
            .iter()
            .flat_map(|name| {
                [
                    config_home().join(name),
                    home_dir().join(".config").join(name),
                ]
            })
            .collect();
        migrate_old_names(&old_roots, &preferred);
    }
    if preferred.exists() || !in_dot_config.exists() {
        preferred
    } else {
        in_dot_config
    }
}

/// Move data from the old folder names: `kde-theme-profile/` / `theme-profile/` → `kolira/`,
/// `profiles/` → `themes/`, and inside each theme its own `.config/kolira`.
/// Runs on every start and does nothing once done. It merges, because something
/// may have created the new folder first (the installer's backups). Nothing is
/// overwritten: an entry that exists in both places stays in the old folder.
fn migrate_old_names(old_roots: &[PathBuf], new_root: &Path) {
    for old in old_roots {
        if !old.is_dir() || old == new_root {
            continue;
        }
        move_children(old, new_root);
        let _ = std::fs::remove_dir(old);
    }
    let (old_themes, themes) = (new_root.join("profiles"), new_root.join("themes"));
    if old_themes.is_dir() {
        move_children(&old_themes, &themes);
        let _ = std::fs::remove_dir(&old_themes);
    }
    let Ok(entries) = std::fs::read_dir(&themes) else {
        return;
    };
    for entry in entries.flatten() {
        let target = entry.path().join(".config").join(DATA_DIR);
        for name in OLD_DATA_DIRS {
            let inner = entry.path().join(".config").join(name);
            if inner != target && inner.is_dir() {
                move_children(&inner, &target);
                let _ = std::fs::remove_dir(&inner);
            }
        }
    }
}

fn move_children(from: &Path, to: &Path) {
    let Ok(entries) = std::fs::read_dir(from) else {
        return;
    };
    let _ = std::fs::create_dir_all(to);
    for entry in entries.flatten() {
        let target = to.join(entry.file_name());
        if !target.exists() {
            let _ = std::fs::rename(entry.path(), &target);
        } else if entry.path().is_dir() && target.is_dir() && entry.file_name() == "backups" {
            move_children(&entry.path(), &target);
            let _ = std::fs::remove_dir(entry.path());
        }
    }
}

/// Resolve persisted backup references after moving a historical data root.
/// Only a missing old backup path with an existing migrated counterpart is mapped.
pub fn migrated_backup_path(path: &Path) -> PathBuf {
    if path.exists() {
        return path.to_path_buf();
    }
    for name in ["kde-theme-profile", "theme-profile"] {
        for base in [config_home(), home_dir().join(".config")] {
            if let Ok(suffix) = path.strip_prefix(base.join(name).join("backups")) {
                let migrated = theme_root().join("backups").join(suffix);
                if migrated.exists() {
                    return migrated;
                }
            }
        }
    }
    path.to_path_buf()
}

pub fn profiles_dir() -> PathBuf {
    theme_root().join("themes")
}

pub fn backups_dir() -> PathBuf {
    theme_root().join("backups")
}

pub fn active_file() -> PathBuf {
    theme_root().join("active-profile")
}

pub fn settings_file() -> PathBuf {
    theme_root().join("app-preferences.json")
}

/// Resolve a profile-relative live path through the XDG base directories.
pub fn live_path(relative: &str) -> PathBuf {
    let path = Path::new(relative);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    let parts: Vec<&std::ffi::OsStr> = path.iter().collect();
    let push_rest = |base: PathBuf, skip: usize| {
        let mut out = base;
        for part in &parts[skip..] {
            out.push(part);
        }
        out
    };
    if parts.first().map(|p| *p == ".config").unwrap_or(false) {
        return push_rest(config_home(), 1);
    }
    if parts.len() >= 2 && parts[0] == ".local" && parts[1] == "share" {
        return push_rest(data_home(), 2);
    }
    if parts.first().map(|p| *p == ".cache").unwrap_or(false) {
        return push_rest(cache_home(), 1);
    }
    home_dir().join(path)
}

/// A profile name is a single path component: no separators, no leading dot.
pub fn is_valid_profile_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 {
        return false;
    }
    if !bytes[0].is_ascii_alphanumeric() {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || *b == b'.' || *b == b'_' || *b == b'-')
}

pub fn validate_profile_name(name: &str) -> Result<()> {
    if is_valid_profile_name(name) {
        Ok(())
    } else {
        Err(anyhow!(
            "Invalid profile name: must match [A-Za-z0-9][A-Za-z0-9._-]{{0,63}}"
        ))
    }
}

/// Agent control files are workspace metadata, not theme assets. An imported
/// package must not become a source of trusted client instructions or hooks.
pub fn is_agent_metadata(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy().to_ascii_lowercase();
        [
            ".git",
            ".codex",
            ".claude",
            ".agents",
            "agents.md",
            "agents.override.md",
            "claude.md",
            "claude.local.md",
            "agents.md.kit-new",
            "agents.override.md.kit-new",
            "claude.md.kit-new",
            "claude.local.md.kit-new",
        ]
        .contains(&name.as_str())
    })
}

pub fn profile_dir(name: &str) -> Result<PathBuf> {
    validate_profile_name(name)?;
    // Name syntax stays identical to Python's validator. These root paths
    // belong to workspace metadata, never to an import/save destination.
    if is_agent_metadata(Path::new(name)) {
        return Err(anyhow!(
            "Theme destination is reserved for workspace instructions: {name}"
        ));
    }
    Ok(profiles_dir().join(name))
}

/// One line in `<theme root>/apply.log`, on disk at once: if the machine freezes
/// during an apply, the last line says which step it was. Best-effort.
pub fn log_step(message: &str) {
    use std::io::Write;
    let path = theme_root().join("apply.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 256 * 1024) {
        let _ = std::fs::rename(&path, theme_root().join("apply.log.old"));
    }
    let _ = std::fs::create_dir_all(theme_root());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(f, "{now} {message}");
        let _ = f.sync_data();
    }
}

/// How long an external tool may take (`KTM_COMMAND_TIMEOUT`, seconds).
fn command_timeout() -> std::time::Duration {
    let secs = std::env::var("KTM_COMMAND_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    std::time::Duration::from_secs(secs)
}

/// Run `cmd` and wait, but not forever: a tool that hangs is killed. An apply
/// stops plasmashell first, so one stuck command (`kvantummanager --set` waits
/// on a dialog when the theme is missing) would leave the user without a desktop.
pub fn status_bounded(cmd: &mut std::process::Command) -> Option<std::process::ExitStatus> {
    output_bounded(cmd.stdout(std::process::Stdio::null())).map(|(status, _)| status)
}

/// [`status_bounded`] that also returns what the command printed.
pub fn output_bounded(
    cmd: &mut std::process::Command,
) -> Option<(std::process::ExitStatus, Vec<u8>)> {
    output_bounded_for(cmd, command_timeout())
}

/// [`output_bounded`] with its own limit (a store's installer, a terminal the
/// user is typing in).
pub fn output_bounded_for(
    cmd: &mut std::process::Command,
    limit: std::time::Duration,
) -> Option<(std::process::ExitStatus, Vec<u8>)> {
    use std::{
        io::Read,
        os::unix::process::CommandExt,
        process::Stdio,
        time::{Duration, Instant},
    };
    let own_group = std::env::var_os("KTM_DEP_OPERATION").is_none();
    if own_group {
        cmd.process_group(0);
    }
    let mut child = cmd.stdout(Stdio::piped()).spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut out = Vec::new();
        // Diagnostics/probes are bounded in memory as well as time.
        let _ = stdout.by_ref().take(16 << 20).read_to_end(&mut out);
        let _ = tx.send(out);
    });
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if own_group {
                    terminate_group(child.id());
                }
                let remaining = deadline
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_secs(1));
                return rx.recv_timeout(remaining).ok().map(|out| (status, out));
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            _ => {
                if own_group {
                    terminate_group(child.id());
                }
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

fn terminate_group(pid: u32) {
    let _ = std::process::Command::new("kill")
        .args(["-KILL", "--", &format!("-{pid}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[cfg(test)]
mod process_tests {
    use super::*;
    #[test]
    fn a_descendant_holding_stdout_does_not_block_a_finished_parent() {
        let start = std::time::Instant::now();
        let result = output_bounded_for(
            std::process::Command::new("sh").args(["-c", "sleep 20 & printf done"]),
            std::time::Duration::from_secs(2),
        );
        assert_eq!(result.unwrap().1, b"done");
        assert!(start.elapsed() < std::time::Duration::from_secs(3));
    }
    #[test]
    fn hanging_commands_have_a_deadline() {
        let start = std::time::Instant::now();
        assert!(output_bounded_for(
            std::process::Command::new("sleep").arg("20"),
            std::time::Duration::from_millis(100)
        )
        .is_none());
        assert!(start.elapsed() < std::time::Duration::from_secs(2));
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;

    #[test]
    fn old_folder_names_move_and_nothing_is_overwritten() {
        let tmp = tempfile::tempdir().unwrap();
        let (old, new) = (tmp.path().join("theme-profile"), tmp.path().join("kolira"));
        std::fs::create_dir_all(old.join("profiles/mine/.config/theme-profile")).unwrap();
        std::fs::write(
            old.join("profiles/mine/.config/theme-profile/palette.json"),
            "p",
        )
        .unwrap();
        std::fs::create_dir_all(old.join("backups/one")).unwrap();
        std::fs::write(old.join("active-profile"), "mine").unwrap();
        std::fs::write(old.join("app-preferences.json"), "old").unwrap();
        // The installer got there first.
        std::fs::create_dir_all(new.join("backups/deploy")).unwrap();
        std::fs::write(new.join("app-preferences.json"), "new").unwrap();
        migrate_old_names(std::slice::from_ref(&old), &new);
        assert!(new
            .join("themes/mine/.config/kolira/palette.json")
            .is_file());
        assert!(new.join("backups/one").is_dir() && new.join("backups/deploy").is_dir());
        assert_eq!(
            std::fs::read_to_string(new.join("active-profile")).unwrap(),
            "mine"
        );
        assert_eq!(
            std::fs::read_to_string(new.join("app-preferences.json")).unwrap(),
            "new"
        );
        assert_eq!(
            std::fs::read_to_string(old.join("app-preferences.json")).unwrap(),
            "old"
        );
        assert!(!old.join("profiles").exists() && !new.join("profiles").exists());
        // Done: a second run changes nothing.
        migrate_old_names(std::slice::from_ref(&old), &new);
        assert!(new.join("themes/mine").is_dir());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_normal_names() {
        for name in ["glass", "paper", "my.theme_2", "A1"] {
            assert!(is_valid_profile_name(name), "{name}");
        }
    }

    #[test]
    fn rejects_traversal_and_separators() {
        for name in ["", ".", "..", "../x", "a/b", ".hidden", "a b", "a\\b"] {
            assert!(!is_valid_profile_name(name), "{name}");
        }
    }

    #[test]
    fn profile_dir_stays_under_profiles() {
        let dir = profile_dir("glass").unwrap();
        assert_eq!(dir.parent(), Some(profiles_dir().as_path()));
        assert!(profile_dir("../etc").is_err());
    }
}
