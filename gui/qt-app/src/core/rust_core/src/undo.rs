//! Apply journal: switching themes leaves nothing of the previous theme behind.
//!
//! A theme only says what it contains. Files it does not contain would stay as
//! the previous theme left them: `fastfetch/config.jsonc`, a window rule, the
//! settings of an effect, a pref in the browser's `prefs.js`. So each apply
//! records what it did on top of the pre-theme state, and the next apply first
//! undoes exactly that. The chain A → B → C → A ends where A began.
//!
//! Only what the theme left untouched is undone: a file or block the user has
//! changed since (its content is no longer what the theme wrote) stays theirs.
//!
//! Lives in `<theme root>/apply-journal/`: `journal.json` + `originals/`. It is
//! local state, never part of a theme or an export.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::appearance::{self, Policy};
use crate::paths::{home_dir, live_path, theme_root};

/// Files larger than this are not kept as originals (no config is).
const MAX_ORIGINAL: u64 = 16 << 20;

/// Firefox/Zen defaults of the look prefs themes set. A pref in `user.js` stays
/// in `prefs.js` after the line is gone; with no earlier value to go back to,
/// the default clears it. Others (a Zen mod's own prefs) do nothing without it.
const PREF_DEFAULTS: &[(&str, &str)] = &[
    ("browser.tabs.allow_transparent_browser", "false"),
    ("zen.widget.linux.transparency", "false"),
    ("toolkit.legacyUserProfileCustomizations.stylesheets", "false"),
    ("layout.css.prefers-color-scheme.content-override", "2"),
    ("zen.view.window.scheme", "2"),
    ("browser.theme.toolbar-theme", "2"),
    ("browser.theme.content-theme", "2"),
    ("extensions.activeThemeID", "\"default-theme@mozilla.org\""),
    ("ui.systemUsesDarkTheme", "0"),
    ("browser.uidensity", "0"),
    ("browser.compactmode.show", "false"),
    ("svg.context-properties.content.enabled", "false"),
];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Journal {
    /// Files that did not exist before the theme: `$HOME`-relative → sha256.
    #[serde(default)]
    created: BTreeMap<String, String>,
    /// Whole files the theme replaced: rel → (sha256 written, original's name).
    #[serde(default)]
    replaced: BTreeMap<String, (String, String)>,
    /// INI groups the theme wrote (settings of an effect, a window rule).
    #[serde(default)]
    blocks: Vec<Block>,
    /// Browser prefs a theme set: `prefs.js` rel → pref → value before any
    /// theme (`None`: it was not set). Kept until `prefs.js` is back.
    #[serde(default)]
    prefs: BTreeMap<String, BTreeMap<String, Option<String>>>,
    /// GSettings values before any theme.
    #[serde(default)]
    gsettings: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Block {
    file: String,
    group: String,
    original: Option<Vec<String>>,
    written: Vec<String>,
}

fn dir() -> PathBuf {
    theme_root().join("apply-journal")
}

fn sha(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

pub fn load() -> Journal {
    fs::read(dir().join("journal.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save(journal: &Journal) -> Result<()> {
    let d = dir();
    fs::create_dir_all(&d)?;
    let tmp = d.join("journal.json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(journal)?)?;
    fs::rename(&tmp, d.join("journal.json")).context("cannot write the apply journal")
}

/// Every regular file under the tracked `rels`, as `$HOME`-relative paths.
fn files_under(rels: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack: Vec<String> = rels.to_vec();
    while let Some(rel) = stack.pop() {
        let path = live_path(&rel);
        let Ok(meta) = fs::symlink_metadata(&path) else { continue };
        if meta.is_dir() {
            if let Ok(entries) = fs::read_dir(&path) {
                for e in entries.flatten() {
                    stack.push(format!("{rel}/{}", e.file_name().to_string_lossy()));
                }
            }
        } else if meta.is_file() {
            out.push(rel);
        }
    }
    out.sort();
    out.dedup();
    out
}

fn is_journal_block_file(rel: &str) -> bool {
    matches!(rel, ".config/kwinrc" | ".config/kwinrulesrc")
}

fn is_tracked_block(file: &str, group: &str) -> bool {
    if file == ".config/kwinrc" {
        group.starts_with("Effect-") || group == "Round-Corners"
    } else {
        group != "General"
    }
}

fn blocks_of(text: &str) -> BTreeMap<String, Vec<String>> {
    appearance::parse_ini(text)
        .into_iter()
        .filter(|s| s.header.is_some())
        .map(|s| {
            let mut lines = s.lines.clone();
            while lines.last().is_some_and(|l| l.trim().is_empty()) {
                lines.pop();
            }
            (s.group().to_string(), lines)
        })
        .collect()
}

/// `text` with INI group `group` replaced (`None`: removed); the rule list of a
/// kwinrulesrc follows.
fn with_block(file: &str, text: &str, group: &str, lines: Option<&[String]>) -> String {
    let mut sections = appearance::parse_ini(text);
    let at = sections.iter().position(|s| s.header.is_some() && s.group() == group);
    match (at, lines) {
        (Some(i), Some(l)) => sections[i].lines = l.to_vec(),
        (Some(i), None) => {
            sections.remove(i);
        }
        (None, Some(l)) => sections.push(appearance::Section { header: Some(format!("[{group}]")), lines: l.to_vec() }),
        (None, None) => {}
    }
    if file == ".config/kwinrulesrc" {
        let ids: Vec<String> = sections.iter().filter(|s| s.header.is_some() && s.group() != "General").map(|s| s.group().to_string()).collect();
        if let Some(general) = sections.iter_mut().find(|s| s.header.is_some() && s.group() == "General") {
            let list = crate::layout::rule_list_lines(&ids, &general.lines);
            general.lines.retain(|l| !matches!(appearance::base_key(l), Some("count" | "rules" | "Order")));
            for (i, line) in list.into_iter().enumerate() {
                general.lines.insert(i, line);
            }
        }
    }
    appearance::render_ini(&sections)
}

fn prefs_of(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("user_pref(\"") else { continue };
        let Some((key, value)) = rest.split_once("\",") else { continue };
        let Some(value) = value.trim().strip_suffix(");") else { continue };
        out.insert(key.to_string(), value.trim().to_string());
    }
    out
}

fn prefs_js_for(user_js: &str) -> Option<String> {
    let is_browser = user_js.starts_with(".zen/") || user_js.starts_with(".mozilla/firefox/");
    user_js.strip_suffix("/user.js").filter(|_| is_browser).map(|d| format!("{d}/prefs.js"))
}

fn remove_empty_parents(path: &Path) {
    let home = home_dir();
    let keep = [home.clone(), home.join(".config"), home.join(".local"), home.join(".local/share")];
    let mut cur = path.parent();
    while let Some(d) = cur {
        if keep.iter().any(|k| k == d) || !d.starts_with(&home) || fs::remove_dir(d).is_err() {
            return;
        }
        cur = d.parent();
    }
}

fn gsettings_get(key: &str) -> Option<String> {
    if crate::field::no_apply() || cfg!(test) {
        return None;
    }
    let (status, out) = crate::paths::output_bounded(std::process::Command::new("gsettings").args(["get", "org.gnome.desktop.interface", key]).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()))?;
    if !status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out).trim().trim_matches('\'').to_string()).filter(|v| !v.is_empty())
}

/// The live GSettings GTK theme (what Wayland GTK apps really use).
pub fn live_gtk_theme() -> Option<String> {
    gsettings_get("gtk-theme")
}

/// Live GSettings colour scheme.
pub fn live_color_scheme() -> Option<String> {
    gsettings_get("color-scheme")
}

/// Undo what the previous apply left. Called with Plasma stopped, before the
/// new theme is restored. Prefs stay journalled: `prefs.js` only changes when
/// the browser next runs.
pub fn revert() -> Result<Journal> {
    let mut j = load();
    for (rel, hash) in std::mem::take(&mut j.created) {
        let path = live_path(&rel);
        if fs::read(&path).is_ok_and(|b| sha(&b) == hash) {
            fs::remove_file(&path)?;
            remove_empty_parents(&path);
        }
    }
    let originals = dir().join("originals");
    for (rel, (hash, name)) in std::mem::take(&mut j.replaced) {
        let path = live_path(&rel);
        let original = originals.join(&name);
        if let (Ok(now), Ok(bytes)) = (fs::read(&path), fs::read(&original)) {
            if sha(&now) == hash {
                fs::write(&path, bytes)?;
            }
        }
    }
    let _ = fs::remove_dir_all(&originals);
    for block in std::mem::take(&mut j.blocks) {
        let path = live_path(&block.file);
        let Ok(text) = fs::read_to_string(&path) else { continue };
        if blocks_of(&text).get(&block.group) == Some(&block.written) {
            fs::write(&path, with_block(&block.file, &text, &block.group, block.original.as_deref()))?;
        }
    }
    for (key, value) in &j.gsettings {
        crate::field::gsettings_set(key, value);
    }
    save(&j)?;
    Ok(j)
}

/// State before the theme is restored.
pub struct Baseline {
    rels: Vec<String>,
    files: BTreeMap<String, Option<Vec<u8>>>,
}

/// Note the current files of the tracked `rels`.
pub fn baseline(rels: &[String]) -> Baseline {
    let mut files = BTreeMap::new();
    for rel in files_under(rels) {
        let path = live_path(&rel);
        let keep = (matches!(appearance::policy(&rel), Policy::Whole) || is_journal_block_file(&rel))
            && fs::metadata(&path).is_ok_and(|m| m.len() <= MAX_ORIGINAL);
        files.insert(rel, keep.then(|| fs::read(&path).ok()).flatten());
    }
    Baseline { rels: rels.to_vec(), files }
}

/// Record what the apply did relative to `before`, and write the prefs that a
/// theme no longer sets back into the browser's `user.js`.
pub fn record(mut j: Journal, before: &Baseline) -> Result<()> {
    // GSettings originals were noted on disk during the apply.
    j.gsettings = load().gsettings;
    let originals = dir().join("originals");
    let now = files_under(&before.rels);
    for rel in &now {
        let Ok(bytes) = fs::read(live_path(rel)) else { continue };
        match before.files.get(rel) {
            None => {
                j.created.insert(rel.clone(), sha(&bytes));
            }
            Some(Some(old)) if old != &bytes && matches!(appearance::policy(rel), Policy::Whole) => {
                let name = sha(rel.as_bytes());
                fs::create_dir_all(&originals)?;
                fs::write(originals.join(&name), old)?;
                j.replaced.insert(rel.clone(), (sha(&bytes), name));
            }
            _ => {}
        }
    }
    for file in [".config/kwinrc", ".config/kwinrulesrc"] {
        let Ok(text) = fs::read_to_string(live_path(file)) else { continue };
        let old = before.files.get(file).cloned().flatten().map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default();
        let (old, new) = (blocks_of(&old), blocks_of(&text));
        for (group, written) in new.iter().filter(|(g, _)| is_tracked_block(file, g)) {
            if old.get(group) != Some(written) {
                j.blocks.push(Block { file: file.into(), group: group.clone(), original: old.get(group).cloned(), written: written.clone() });
            }
        }
    }
    record_prefs(&mut j, &before.rels)?;
    save(&j)
}

fn record_prefs(j: &mut Journal, rels: &[String]) -> Result<()> {
    for user_js in rels.iter().filter(|r| r.ends_with("/user.js")) {
        let Some(prefs_rel) = prefs_js_for(user_js) else { continue };
        let path = live_path(user_js);
        let theme = fs::read_to_string(&path).map(|t| prefs_of(&t)).unwrap_or_default();
        let stored = prefs_of(&fs::read_to_string(live_path(&prefs_rel)).unwrap_or_default());
        let entry = j.prefs.entry(prefs_rel.clone()).or_default();
        for key in theme.keys() {
            entry.entry(key.clone()).or_insert_with(|| stored.get(key).cloned());
        }
        // Prefs of an earlier theme: back to their value on the next start.
        let mut restore = String::new();
        let mut settled = Vec::new();
        for (key, original) in entry.iter() {
            if theme.contains_key(key) {
                continue;
            }
            let wanted = original.clone().or_else(|| PREF_DEFAULTS.iter().find(|(k, _)| k == key).map(|(_, v)| (*v).to_string()));
            let back = match (stored.get(key), original) {
                (None, None) => true,
                (Some(v), Some(o)) => v == o,
                _ => false,
            };
            match wanted {
                Some(value) if !back => restore.push_str(&format!("user_pref(\"{key}\", {value});\n")),
                _ => settled.push(key.clone()),
            }
        }
        for key in settled {
            entry.remove(&key);
        }
        if !restore.is_empty() && path.parent().is_some_and(Path::exists) {
            let old = fs::read_to_string(&path).unwrap_or_default();
            let sep = if old.is_empty() || old.ends_with('\n') { "" } else { "\n" };
            fs::write(&path, format!("{old}{sep}// theme-profile: back to the value before the previous theme\n{restore}"))?;
            // Written for the browser's next start: not a theme's file to remove.
            j.created.remove(user_js);
            j.replaced.remove(user_js);
        }
    }
    Ok(())
}

/// Remember a GSettings value the first time a theme changes it.
pub fn note_gsettings(key: &str) {
    let Some(value) = gsettings_get(key) else { return };
    let mut j = load();
    if !j.gsettings.contains_key(key) {
        j.gsettings.insert(key.to_string(), value);
        let _ = save(&j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefs_parse() {
        let p = prefs_of("user_pref(\"a.b\", true);\n// no\nuser_pref(\"c\", \"x, y\");\n");
        assert_eq!(p.get("a.b").map(String::as_str), Some("true"));
        assert_eq!(p.get("c").map(String::as_str), Some("\"x, y\""));
    }

    #[test]
    fn rule_blocks_keep_the_rule_list() {
        let text = "[General]\nOrder=a,b\n\n[a]\nDescription=mine\n\n[b]\nDescription=theme\n";
        let t = with_block(".config/kwinrulesrc", text, "b", None);
        assert!(t.contains("Order=a\n") && !t.contains("[b]") && t.contains("[a]"), "{t}");
        let l = vec!["x=1".to_string()];
        let t = with_block(".config/kwinrc", "[Plugins]\nfoo=1\n", "Effect-x", Some(&l));
        assert!(t.contains("[Effect-x]\nx=1") && t.contains("foo=1"), "{t}");
    }

    #[test]
    fn only_browsers_have_prefs() {
        assert_eq!(prefs_js_for(".zen/x y/user.js").as_deref(), Some(".zen/x y/prefs.js"));
        assert_eq!(prefs_js_for(".config/app/user.js"), None);
    }
}
