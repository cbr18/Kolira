//! The user's automation choices — per user, not per theme: whether applying
//! a theme may download from stores, install system packages (in a terminal,
//! the user types the password), restart programs. Each is `ask` (offer, the
//! default), `always` (do it, report it) or `never` (skip, do not offer).
//!
//! Stored in `app-preferences.json` as `"automation": {key: policy}` plus
//! `"automation_configured": true` once the user chose (first run of the app,
//! or the installer's `theme-profile automation ask`).

use std::io::{BufRead, Write};

use anyhow::{bail, Context, Result};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    Ask,
    Always,
    Never,
}

impl Policy {
    pub fn parse(s: &str) -> Option<Policy> {
        match s.trim() {
            "ask" => Some(Policy::Ask),
            "always" => Some(Policy::Always),
            "never" => Some(Policy::Never),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Policy::Ask => "ask",
            Policy::Always => "always",
            Policy::Never => "never",
        }
    }
}

/// (key, what it does, what to know before choosing `always`).
pub const KEYS: &[(&str, &str, &str)] = &[
    ("store", "Download missing themes, icons, widgets and editor themes from their official stores", "downloads from the KDE Store, VS Code Marketplace / Open VSX, Zed"),
    ("packages", "Install missing system packages (Kvantum, KWin effects, Klassy…) in a terminal", "opens a terminal with yay/pacman; you type your password there"),
    ("restart_vesktop", "Restart Vesktop when a theme changed its look", "a voice call or screen share in Vesktop ends"),
    ("restart_discord", "Restart Discord (Vencord) when a theme changed its look", "a voice call or screen share in Discord ends"),
    ("restart_zen", "Restart Zen Browser when a theme changed its look", "calls and downloads in Zen stop; tabs return with session restore"),
    ("restart_firefox", "Restart Firefox when a theme changed its look", "calls and downloads in Firefox stop; tabs return with session restore"),
];

fn read_settings() -> Map<String, Value> {
    std::fs::read_to_string(crate::paths::settings_file())
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

/// The user's policy for `key` (`ask` when unset or unknown).
pub fn policy(key: &str) -> Policy {
    read_settings()
        .get("automation")
        .and_then(|a| a.get(key))
        .and_then(Value::as_str)
        .and_then(Policy::parse)
        .unwrap_or(Policy::Ask)
}

/// Has the user been asked yet?
pub fn configured() -> bool {
    read_settings().get("automation_configured").and_then(Value::as_bool).unwrap_or(false)
}

/// Store `choices` (other preferences are kept) and mark automation configured.
pub fn set(choices: &[(&str, Policy)]) -> Result<()> {
    for (key, _) in choices {
        if !KEYS.iter().any(|(k, _, _)| k == key) {
            bail!("unknown automation key {key:?} (known: {})", KEYS.iter().map(|k| k.0).collect::<Vec<_>>().join(", "));
        }
    }
    let mut settings = read_settings();
    let mut auto = settings.get("automation").and_then(Value::as_object).cloned().unwrap_or_default();
    for (key, p) in choices {
        auto.insert((*key).to_string(), Value::from(p.as_str()));
    }
    settings.insert("automation".into(), Value::Object(auto));
    settings.insert("automation_configured".into(), Value::Bool(true));
    let path = crate::paths::settings_file();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(&Value::Object(settings))? + "\n";
    let tmp = path.with_extension("tmp-automation");
    std::fs::write(&tmp, text)?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, &path).context("cannot save app-preferences.json")
}

/// Ask every question on a terminal (installer); Enter keeps the current one.
pub fn ask_interactively(input: &mut dyn BufRead, out: &mut dyn Write) -> Result<()> {
    writeln!(out, "What may KDE Theme Manager do by itself when you apply a theme?")?;
    writeln!(out, "  ask = offer it each time, always = just do it, never = skip it\n")?;
    let mut choices = Vec::new();
    for (key, what, note) in KEYS {
        let current = policy(key);
        loop {
            write!(out, "{what}\n  ({note})\n  [ask/always/never, Enter = {}]: ", current.as_str())?;
            out.flush()?;
            let mut line = String::new();
            if input.read_line(&mut line)? == 0 {
                choices.push((*key, current));
                break;
            }
            let line = line.trim();
            if line.is_empty() {
                choices.push((*key, current));
                break;
            }
            match Policy::parse(line) {
                Some(p) => {
                    choices.push((*key, p));
                    break;
                }
                None => writeln!(out, "  please type ask, always or never")?,
            }
        }
    }
    set(&choices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policies_parse() {
        assert_eq!(Policy::parse("always"), Some(Policy::Always));
        assert_eq!(Policy::parse(" never\n"), Some(Policy::Never));
        assert_eq!(Policy::parse("yes"), None);
        for p in [Policy::Ask, Policy::Always, Policy::Never] {
            assert_eq!(Policy::parse(p.as_str()), Some(p));
        }
    }
}
