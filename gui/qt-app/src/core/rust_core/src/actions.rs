//! What applying a theme leaves to do outside the config files — offered,
//! never done silently (programs may be in a call, a game, a download):
//!
//! * **restart** a running program that reads its look only at start:
//!   Vesktop / Discord with Vencord (a newly enabled theme, the translucent
//!   window), Zen / Firefox (`user.js`, `userChrome.css`);
//! * **steam-skin**: install Adwaita for Steam with the theme's colours
//!   (patches the Steam client; restart Steam to see it);
//! * **manual**: Telegram's one-time choice of the palette file (it cannot be
//!   set from outside; after that Telegram follows the file by itself).
//!
//! `for_theme` lists what applies to a theme here; `run` performs one action.
//! Each comes with a warning the GUI shows next to its checkbox.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Action {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub reason: String,
    pub warning: String,
}

/// A program that reads its look at start: (id, title, process names, launch
/// commands, theme marker relative to the theme dir, warning).
struct App {
    id: &'static str,
    title: &'static str,
    processes: &'static [&'static str],
    launch: &'static [&'static str],
    marker: &'static str,
    warning: &'static str,
}

const APPS: &[App] = &[
    App {
        id: "vesktop",
        title: "Vesktop",
        processes: &["vesktop"],
        launch: &["vesktop"],
        marker: ".config/vesktop/themes",
        warning: "a voice call or screen share in Vesktop ends",
    },
    App {
        id: "discord",
        title: "Discord (Vencord)",
        processes: &["Discord", "discord"],
        launch: &["discord", "Discord"],
        marker: ".config/Vencord/themes",
        warning: "a voice call or screen share in Discord ends",
    },
    App {
        id: "zen",
        title: "Zen Browser",
        processes: &["zen-bin", "zen"],
        launch: &["zen-browser", "zen"],
        marker: ".zen",
        warning: "open calls and downloads in Zen stop; tabs come back if session restore is on",
    },
    App {
        id: "firefox",
        title: "Firefox",
        processes: &["firefox"],
        launch: &["firefox"],
        marker: ".mozilla/firefox",
        warning: "open calls and downloads in Firefox stop; tabs come back if session restore is on",
    },
];

fn no_apply() -> bool {
    std::env::var("KTM_THEME_NO_APPLY").as_deref() == Ok("1")
}

/// Oldest running pid of any of `names` (the main process of the program).
fn running(names: &[&str]) -> Option<u32> {
    names.iter().find_map(|n| {
        let out = Command::new("pgrep").args(["-o", "-x", n]).output().ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok()
    })
}

/// Actions for theme directory `dir` on this machine.
pub fn for_theme(dir: &Path) -> Vec<Action> {
    for_theme_since(dir, None)
}

/// Did applying change any of the theme's files under `marker`? `before` is
/// the pre-apply backup (the live files as they were); without it, assume so.
fn changed(dir: &Path, marker: &str, before: Option<&Path>) -> bool {
    let Some(before) = before else { return true };
    let root = dir.join(marker);
    let mut stack = vec![root.clone()];
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&p) {
                stack.extend(entries.flatten().map(|e| e.path()));
            }
        } else if let Ok(rel) = p.strip_prefix(dir) {
            if std::fs::read(&p).ok() != std::fs::read(before.join(rel)).ok() {
                return true;
            }
        }
    }
    false
}

/// [`for_theme`] right after an apply: restarts only for programs whose
/// files the apply changed (`before` = the pre-apply backup).
pub fn for_theme_since(dir: &Path, before: Option<&Path>) -> Vec<Action> {
    let mut out = Vec::new();
    for app in APPS {
        if dir.join(app.marker).exists() && running(app.processes).is_some() && changed(dir, app.marker, before) {
            out.push(Action {
                id: app.id.into(),
                kind: "restart".into(),
                title: format!("Restart {}", app.title),
                reason: format!("{} reads the theme's look only when it starts", app.title),
                warning: app.warning.into(),
            });
        }
    }
    if dir.join(crate::steam::CSS_FILE).is_file() && crate::steam::steam_installed() {
        out.push(Action {
            id: "steam-skin".into(),
            kind: "steam-skin".into(),
            title: "Install the Steam skin (Adwaita for Steam)".into(),
            reason: "the theme carries colours for Steam; the skin patches the Steam client's files".into(),
            warning: "downloads and runs the Adwaita for Steam installer; restart Steam afterwards (close games first)".into(),
        });
    }
    if dir.join(crate::telegram::PALETTE_FILE).is_file() && crate::telegram::installed() && !crate::telegram::follows_file() {
        out.push(Action {
            id: "telegram".into(),
            kind: "manual".into(),
            title: "Telegram: choose the theme file once".into(),
            reason: format!(
                "Telegram → Settings → Chat Settings → Choose from file → ~/{}; afterwards Telegram follows it by itself",
                crate::telegram::PALETTE_FILE
            ),
            warning: String::new(),
        });
    }
    out
}

/// Ask `pid` to quit (SIGTERM — a normal quit: browsers save the session)
/// and wait up to `timeout` for every process named `names` to end.
fn quit(pid: u32, names: &[&str], timeout: Duration) -> Result<()> {
    let _ = Command::new("kill").args(["-TERM", &pid.to_string()]).status();
    let start = Instant::now();
    while running(names).is_some() {
        if start.elapsed() > timeout {
            bail!("it did not quit within {} s — close it yourself", timeout.as_secs());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Ok(())
}

fn launch(commands: &[&str]) -> Result<String> {
    use std::os::unix::process::CommandExt;
    for cmd in commands {
        if which::which(cmd).is_err() {
            continue;
        }
        Command::new(cmd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()?;
        return Ok((*cmd).to_string());
    }
    bail!("none of {} is installed", commands.join(", "))
}

/// Perform action `id` (from `for_theme`).
pub fn run(id: &str) -> Result<String> {
    if let Some(app) = APPS.iter().find(|a| a.id == id) {
        let Some(pid) = running(app.processes) else {
            return Ok(format!("{} is not running", app.title));
        };
        if no_apply() {
            return Ok(format!("would restart {} (KTM_THEME_NO_APPLY)", app.title));
        }
        quit(pid, app.processes, Duration::from_secs(20))?;
        let bin = launch(app.launch)?;
        return Ok(format!("{} restarted ({bin})", app.title));
    }
    match id {
        "steam-skin" => {
            let css = crate::paths::home_dir().join(crate::steam::CSS_FILE);
            crate::steam::install(&css, false)
        }
        "telegram" => Err(anyhow!("Telegram's theme is chosen inside Telegram: Settings → Chat Settings → Choose from file")),
        _ => bail!("unknown action {id:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_for_an_empty_theme() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(for_theme(tmp.path()).iter().all(|a| a.kind != "steam-skin" && a.kind != "manual"));
    }

    #[test]
    fn restart_only_when_the_apply_changed_files() {
        let tmp = tempfile::tempdir().unwrap();
        let (theme, backup) = (tmp.path().join("theme"), tmp.path().join("backup"));
        for d in [&theme, &backup] {
            std::fs::create_dir_all(d.join(".config/vesktop/themes")).unwrap();
        }
        std::fs::write(theme.join(".config/vesktop/themes/t.css"), "a").unwrap();
        std::fs::write(backup.join(".config/vesktop/themes/t.css"), "a").unwrap();
        assert!(!changed(&theme, ".config/vesktop/themes", Some(&backup)));
        std::fs::write(backup.join(".config/vesktop/themes/t.css"), "b").unwrap();
        assert!(changed(&theme, ".config/vesktop/themes", Some(&backup)));
        assert!(changed(&theme, ".config/vesktop/themes", None));
    }

    #[test]
    fn unknown_action_is_an_error() {
        assert!(run("rm -rf").is_err());
    }
}
