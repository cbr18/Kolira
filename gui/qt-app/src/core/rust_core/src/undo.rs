//! Apply journal: switching themes leaves nothing of the previous theme behind.
//!
//! A theme only says what it contains. Files it does not contain would stay as
//! the previous theme left them: a window rule, the
//! settings of an effect, a pref in the browser's `prefs.js`. So each apply
//! records what it did on top of the pre-theme state, and the next apply first
//! undoes exactly that. The chain A → B → C → A ends where A began.
//!
//! Changes are reversed against exact before/after states. Mixed configs restore
//! only values still matching what Apply wrote; later user edits are retained.
//!
//! Lives in `<theme root>/apply-journal/`: `journal.json` + `originals/`. It is
//! local state, never part of a theme or an export.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::appearance;
use crate::change_plan::{Entry, Plan};
use crate::paths::{live_path, theme_root};

/// Firefox/Zen defaults of the look prefs themes set. A pref in `user.js` stays
/// in `prefs.js` after the line is gone; with no earlier value to go back to,
/// the default clears it. Others (a Zen mod's own prefs) do nothing without it.
const PREF_DEFAULTS: &[(&str, &str)] = &[
    ("ui.accentcolor", "\"\""),
    ("ui.accentcolortext", "\"\""),
    ("zen.theme.accent-color", "\"AccentColor\""),
    ("browser.tabs.allow_transparent_browser", "false"),
    ("zen.widget.linux.transparency", "false"),
    (
        "toolkit.legacyUserProfileCustomizations.stylesheets",
        "false",
    ),
    ("layout.css.prefers-color-scheme.content-override", "2"),
    ("zen.view.window.scheme", "2"),
    ("browser.theme.toolbar-theme", "2"),
    ("browser.theme.content-theme", "2"),
    ("extensions.activeThemeID", "\"default-theme@mozilla.org\""),
    ("ui.systemUsesDarkTheme", "\"\""),
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
    /// Every actual filesystem mutation, including mixed configs and removals.
    #[serde(default)]
    changes: BTreeMap<String, Change>,
    /// GSettings values before any theme.
    #[serde(default)]
    pub(crate) gsettings: BTreeMap<String, String>,
    #[serde(default)]
    gsettings_after: BTreeMap<String, String>,
    #[serde(default)]
    tmux_changes: Vec<crate::tmux_runtime::Change>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Change {
    before: Saved,
    after: Saved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Saved {
    Missing,
    File { blob: String, mode: u32 },
    Directory { mode: u32 },
    Symlink { target: PathBuf },
}
impl Saved {
    fn store(entry: &Entry) -> Result<Self> {
        Ok(match entry {
            Entry::Missing => Self::Missing,
            Entry::Directory { mode } => Self::Directory { mode: *mode },
            Entry::Symlink { target } => Self::Symlink {
                target: target.clone(),
            },
            Entry::File { bytes, mode } => {
                let blob = sha(bytes);
                let folder = dir().join("originals");
                fs::create_dir_all(&folder)?;
                fs::set_permissions(&folder, std::os::unix::fs::PermissionsExt::from_mode(0o700))?;
                crate::gui_api::write_bytes(&folder.join(&blob), bytes, Some(0o600))?;
                Self::File { blob, mode: *mode }
            }
        })
    }
    fn read(&self, plan: &mut Plan) -> Result<Entry> {
        Ok(match self {
            Self::Missing => Entry::Missing,
            Self::Directory { mode } => Entry::Directory { mode: *mode },
            Self::Symlink { target } => Entry::Symlink {
                target: target.clone(),
            },
            Self::File { blob, mode } => {
                anyhow::ensure!(
                    blob.len() == 64 && blob.bytes().all(|b| b.is_ascii_hexdigit()),
                    "Invalid journal blob"
                );
                let bytes = plan
                    .read(&dir().join("originals").join(blob))?
                    .context("Missing apply journal original")?;
                anyhow::ensure!(sha(&bytes) == *blob, "Corrupt apply journal original");
                Entry::File { bytes, mode: *mode }
            }
        })
    }
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
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn load() -> Journal {
    fs::read(dir().join("journal.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save(journal: &Journal) -> Result<()> {
    let d = dir();
    fs::create_dir_all(&d)?;
    fs::set_permissions(&d, std::os::unix::fs::PermissionsExt::from_mode(0o700))?;
    crate::gui_api::write_bytes(
        &d.join("journal.json"),
        &serde_json::to_vec_pretty(journal)?,
        Some(0o600),
    )
}

/// Capture actual entries, without following symlinks or discarding mixed
/// config bytes. Errors abort Apply while its recovery copy is still intact.
fn entries(rels: &[String]) -> Result<BTreeMap<String, Entry>> {
    fn walk(rel: String, out: &mut BTreeMap<String, Entry>) -> Result<()> {
        if appearance::is_retired_app_path(&rel) || Path::new(&rel).starts_with(".config/fastfetch")
        {
            return Ok(());
        }
        let path = live_path(&rel);
        let entry = crate::change_plan::entry(&path)?;
        let directory = matches!(entry, Entry::Directory { .. });
        out.insert(rel.clone(), entry);
        if directory {
            for item in fs::read_dir(path)? {
                walk(
                    format!("{rel}/{}", item?.file_name().to_string_lossy()),
                    out,
                )?;
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    for rel in rels {
        walk(rel.clone(), &mut out)?;
    }
    Ok(out)
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
    let at = sections
        .iter()
        .position(|s| s.header.is_some() && s.group() == group);
    match (at, lines) {
        (Some(i), Some(l)) => sections[i].lines = l.to_vec(),
        (Some(i), None) => {
            sections.remove(i);
        }
        (None, Some(l)) => sections.push(appearance::Section {
            header: Some(format!("[{group}]")),
            lines: l.to_vec(),
        }),
        (None, None) => {}
    }
    if file == ".config/kwinrulesrc" {
        let ids: Vec<String> = sections
            .iter()
            .filter(|s| s.header.is_some() && s.group() != "General")
            .map(|s| s.group().to_string())
            .collect();
        if let Some(general) = sections
            .iter_mut()
            .find(|s| s.header.is_some() && s.group() == "General")
        {
            let list = crate::layout::rule_list_lines(&ids, &general.lines);
            general
                .lines
                .retain(|l| !matches!(appearance::base_key(l), Some("count" | "rules" | "Order")));
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
        let Some(rest) = line.trim().strip_prefix("user_pref(\"") else {
            continue;
        };
        let Some((key, value)) = rest.split_once("\",") else {
            continue;
        };
        let Some(value) = value.trim().strip_suffix(");") else {
            continue;
        };
        out.insert(key.to_string(), value.trim().to_string());
    }
    out
}

fn prefs_js_for(user_js: &str) -> Option<String> {
    let is_browser = crate::browser_appearance::is_profile_file(user_js);
    user_js
        .strip_suffix("/user.js")
        .filter(|_| is_browser)
        .map(|d| format!("{d}/prefs.js"))
}

pub(crate) fn gsettings_get(key: &str) -> Option<String> {
    if crate::field::no_apply() || cfg!(test) {
        return None;
    }
    let (status, out) = crate::paths::output_bounded(
        std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", key])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null()),
    )?;
    if !status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out)
            .trim()
            .trim_matches('\'')
            .to_string(),
    )
    .filter(|v| !v.is_empty())
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
    revert_preserving(&[])
}

pub(crate) fn revert_preserving(preserved: &[PathBuf]) -> Result<Journal> {
    let mut plan = crate::change_plan::Plan::default();
    let mut journal = plan_revert_preserving(&mut plan, preserved)?;
    plan.commit()?;
    crate::tmux_runtime::restore(&journal.tmux_changes)?;
    journal.tmux_changes.clear();
    let _ = fs::remove_dir_all(dir().join("originals"));
    for (key, value) in &journal.gsettings {
        // Older Apply versions journalled this independent user preference.
        // Themes no longer manage it; replaying that legacy entry would still
        // overwrite a font the user chose after applying the old theme.
        if key == "monospace-font-name" {
            continue;
        }
        crate::field::gsettings_set(key, value);
    }
    save(&journal)?;
    Ok(journal)
}

pub(crate) fn plan_revert_preserving(plan: &mut Plan, preserved: &[PathBuf]) -> Result<Journal> {
    let mut dependencies = Vec::new();
    for (hosts, include) in [
        (
            crate::extras::SHELL_HOSTS.to_vec(),
            crate::extras::OMP_THEME,
        ),
        (
            vec![crate::terminal_apps::WOFI_HOST],
            crate::terminal_apps::WOFI_COLORS,
        ),
        (
            crate::four_apps::TMUX_HOSTS.to_vec(),
            crate::four_apps::TMUX_SCRIPT,
        ),
        (vec![appearance::KITTY_HOST], appearance::KITTY_THEME),
        (
            vec![appearance::ALACRITTY_HOST],
            appearance::ALACRITTY_THEME,
        ),
        (
            vec![crate::extras::GHOSTTY_HOST],
            crate::extras::GHOSTTY_THEME,
        ),
    ] {
        dependencies.push((hosts, include, plan.get(&live_path(include))?));
    }
    let kept = preserved
        .iter()
        .map(|path| Ok((path.clone(), plan.get(path)?)))
        .collect::<Result<Vec<_>>>()?;
    let journal = plan_revert_inner(plan)?;
    for (hosts, include, original) in dependencies {
        let path = live_path(include);
        if matches!(plan.get(&path)?, Entry::Missing) && !matches!(original, Entry::Missing) {
            let name = Path::new(include)
                .file_name()
                .context("include name")?
                .to_string_lossy();
            let mut referenced = false;
            for host in hosts {
                if plan
                    .text(&live_path(host))?
                    .is_some_and(|text| text.contains(name.as_ref()))
                {
                    referenced = true;
                    break;
                }
            }
            if referenced {
                plan.ensure_parent(&path)?;
                plan.put(&path, original, "kept_referenced_include")?;
            }
        }
    }
    for (path, value) in kept {
        if !matches!(value, Entry::Missing) {
            plan.ensure_parent(&path)?;
            plan.put(&path, value, "keep_wallpaper")?;
        }
    }
    Ok(journal)
}

fn plan_revert_inner(plan: &mut Plan) -> Result<Journal> {
    let mut j: Journal = plan
        .text(&dir().join("journal.json"))?
        .map(|text| serde_json::from_str(&text).context("Invalid apply journal"))
        .transpose()?
        .unwrap_or_default();
    let mut changes: Vec<_> = std::mem::take(&mut j.changes).into_iter().collect();
    changes.sort_by_key(|(rel, _)| std::cmp::Reverse(Path::new(rel).components().count()));
    for (rel, change) in changes {
        anyhow::ensure!(
            Path::new(&rel).components().all(|c| matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::RootDir
            )),
            "Invalid journal path"
        );
        if appearance::is_retired_app_path(&rel) || Path::new(&rel).starts_with(".config/fastfetch")
        {
            continue;
        }
        let path = live_path(&rel);
        let current = plan.get(&path)?;
        let before = change.before.read(plan)?;
        let after = change.after.read(plan)?;
        let restore = if current == after {
            Some(before)
        } else if let (
            Entry::File { bytes: now, mode },
            Entry::File {
                bytes: written,
                mode: written_mode,
            },
        ) = (&current, &after)
        {
            let original = match &before {
                Entry::File { bytes, .. } => bytes.as_slice(),
                Entry::Missing => &[],
                _ => continue,
            };
            if now == written {
                if matches!(before, Entry::Missing) {
                    continue;
                }
                let mut value = before;
                if let Entry::File { mode: old_mode, .. } = &mut value {
                    *old_mode = *mode;
                }
                Some(value)
            } else if let (Ok(original), Ok(written), Ok(now)) = (
                std::str::from_utf8(original),
                std::str::from_utf8(written),
                std::str::from_utf8(now),
            ) {
                let text = crate::appearance::restore_changed(&rel, original, written, now)?;
                let restored_mode = match &before {
                    Entry::File { mode: old_mode, .. } if mode == written_mode => *old_mode,
                    _ => *mode,
                };
                Some(Entry::File {
                    bytes: text.into_bytes(),
                    mode: restored_mode,
                })
            } else {
                None
            }
        } else {
            None
        };
        if let Some(value) = restore {
            if matches!(value, Entry::Missing) && matches!(current, Entry::Directory { .. }) {
                let mut empty = true;
                for child in plan.children(&path)? {
                    if !matches!(plan.get(&child)?, Entry::Missing) {
                        empty = false;
                        break;
                    }
                }
                if !empty {
                    continue;
                }
            }
            plan.ensure_parent(&path)?;
            plan.put(&path, value, "previous_theme")?;
        }
    }
    for (rel, hash) in std::mem::take(&mut j.created) {
        // Shell init lines keep pointing to this shared prompt file when the
        // next theme has no prompt. Keep it, matching profile::plan_reset.
        if appearance::is_retired_app_path(&rel)
            || Path::new(&rel).starts_with(".config/fastfetch")
            || rel == crate::extras::OMP_THEME
        {
            continue;
        }
        let path = live_path(&rel);
        if plan.read(&path)?.is_some_and(|b| sha(&b) == hash) {
            plan.remove(&path, "previous_theme")?;
            plan.remove_empty_parents(&path)?;
        }
    }
    let originals = dir().join("originals");
    for (rel, (hash, name)) in std::mem::take(&mut j.replaced) {
        if appearance::is_retired_app_path(&rel) || Path::new(&rel).starts_with(".config/fastfetch")
        {
            continue;
        }
        let path = live_path(&rel);
        let original = originals.join(&name);
        if let (Some(now), Some(bytes)) = (plan.read(&path)?, plan.read(&original)?) {
            if sha(&now) == hash {
                plan.write(&path, bytes, "previous_theme")?;
            }
        }
    }
    for block in std::mem::take(&mut j.blocks) {
        let path = live_path(&block.file);
        let Some(text) = plan.text(&path)? else {
            continue;
        };
        if blocks_of(&text).get(&block.group) == Some(&block.written) {
            plan.write_text(
                &path,
                with_block(&block.file, &text, &block.group, block.original.as_deref()),
                "previous_theme",
            )?;
        }
    }
    j.gsettings.retain(|key, _| {
        j.gsettings_after
            .get(key)
            .is_none_or(|written| gsettings_get(key).as_ref() == Some(written))
    });
    j.gsettings_after.clear();
    Ok(j)
}

/// State after reversing the previous theme, before applying the new one.
pub struct Baseline {
    rels: Vec<String>,
    files: BTreeMap<String, Entry>,
}

pub fn baseline(rels: &[String]) -> Result<Baseline> {
    let mut roots = rels.to_vec();
    let boundaries = [
        crate::paths::home_dir(),
        crate::paths::config_home(),
        crate::paths::data_home(),
        crate::paths::cache_home(),
    ];
    for relative in rels {
        let path = live_path(relative);
        let mut parent = path.parent();
        while let Some(path) = parent {
            if boundaries.iter().any(|boundary| path == boundary) || path == Path::new("/") {
                break;
            }
            if !matches!(crate::change_plan::entry(path)?, Entry::Missing) {
                break;
            }
            let relative = if let Ok(p) = path.strip_prefix(crate::paths::config_home()) {
                format!(".config/{}", p.display())
            } else if let Ok(p) = path.strip_prefix(crate::paths::data_home()) {
                format!(".local/share/{}", p.display())
            } else if let Ok(p) = path.strip_prefix(crate::paths::home_dir()) {
                p.to_string_lossy().to_string()
            } else {
                path.to_string_lossy().to_string()
            };
            if !roots.contains(&relative) {
                roots.push(relative);
            }
            parent = path.parent();
        }
    }
    Ok(Baseline {
        files: entries(&roots)?,
        rels: roots,
    })
}

/// Record every actual change, not just whole-file appearance resources. Blobs
/// preserve the exact originals and the values we wrote for three-way recovery.
pub fn record(mut j: Journal, before: &Baseline) -> Result<()> {
    j.tmux_changes = load().tmux_changes;
    j.gsettings = load().gsettings;
    record_prefs(&mut j, &before.rels)?;
    let now = entries(&before.rels)?;
    let paths: std::collections::BTreeSet<_> = before.files.keys().chain(now.keys()).collect();
    for rel in paths {
        let old = before.files.get(rel).unwrap_or(&Entry::Missing);
        let written = now.get(rel).unwrap_or(&Entry::Missing);
        if old != written {
            j.changes.insert(
                rel.clone(),
                Change {
                    before: Saved::store(old)?,
                    after: Saved::store(written)?,
                },
            );
        }
    }
    j.gsettings_after = j
        .gsettings
        .keys()
        .filter_map(|key| gsettings_get(key).map(|value| (key.clone(), value)))
        .collect();
    save(&j)
}

fn record_prefs(j: &mut Journal, rels: &[String]) -> Result<()> {
    let mut plan = crate::change_plan::Plan::default();
    plan_record_prefs(j, rels, &mut plan)?;
    plan.commit()
}

pub(crate) fn plan_record_prefs(
    j: &mut Journal,
    rels: &[String],
    plan: &mut crate::change_plan::Plan,
) -> Result<()> {
    for user_js in rels.iter().filter(|r| r.ends_with("/user.js")) {
        let Some(prefs_rel) = prefs_js_for(user_js) else {
            continue;
        };
        let path = live_path(user_js);
        let theme = plan.text(&path)?.map(|t| prefs_of(&t)).unwrap_or_default();
        let stored = prefs_of(&plan.text(&live_path(&prefs_rel))?.unwrap_or_default());
        let entry = j.prefs.entry(prefs_rel.clone()).or_default();
        for key in theme
            .keys()
            .filter(|key| crate::browser_appearance::PREFS.contains(&key.as_str()))
        {
            entry
                .entry(key.clone())
                .or_insert_with(|| stored.get(key).cloned());
        }
        // Prefs of an earlier theme: back to their value on the next start.
        let mut restore = String::new();
        let mut settled = Vec::new();
        for (key, original) in entry.iter() {
            if theme.contains_key(key) {
                continue;
            }
            let wanted = original.clone().or_else(|| {
                PREF_DEFAULTS
                    .iter()
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| (*v).to_string())
            });
            let back = match (stored.get(key), original) {
                (None, None) => true,
                (Some(v), Some(o)) => v == o,
                _ => false,
            };
            match wanted {
                Some(value) if !back => {
                    restore.push_str(&format!("user_pref(\"{key}\", {value});\n"))
                }
                _ => settled.push(key.clone()),
            }
        }
        for key in settled {
            entry.remove(&key);
        }
        if !restore.is_empty() && path.parent().is_some_and(Path::exists) {
            let old = plan.text(&path)?.unwrap_or_default();
            let sep = if old.is_empty() || old.ends_with('\n') {
                ""
            } else {
                "\n"
            };
            plan.write_text(
                &path,
                format!(
                    "{old}{sep}// kolira: back to the value before the previous theme\n{restore}"
                ),
                "browser_reset",
            )?;
            // Written for the browser's next start: not a theme's file to remove.
            j.created.remove(user_js);
            j.replaced.remove(user_js);
        }
    }
    Ok(())
}

/// Remember a GSettings value the first time a theme changes it.
pub fn note_gsettings(key: &str) {
    let Some(value) = gsettings_get(key) else {
        return;
    };
    let mut j = load();
    if !j.gsettings.contains_key(key) {
        j.gsettings.insert(key.to_string(), value);
        let _ = save(&j);
    }
}

/// Previous-theme files may be changed even by a partial profile application.
pub(crate) fn affected_paths() -> Vec<String> {
    let journal = load();
    let mut paths: Vec<_> = journal
        .created
        .keys()
        .chain(journal.replaced.keys())
        .cloned()
        .collect();
    paths.extend(journal.changes.keys().cloned());
    paths.extend(journal.blocks.into_iter().map(|b| b.file));
    paths.extend(journal.prefs.keys().filter_map(|p| {
        p.strip_suffix("prefs.js")
            .map(|root| format!("{root}user.js"))
    }));
    paths
}

pub(crate) fn note_tmux(changes: Vec<crate::tmux_runtime::Change>) -> Result<()> {
    let mut j = load();
    j.tmux_changes = changes;
    save(&j)
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
        assert!(
            t.contains("Order=a\n") && !t.contains("[b]") && t.contains("[a]"),
            "{t}"
        );
        let l = vec!["x=1".to_string()];
        let t = with_block(".config/kwinrc", "[Plugins]\nfoo=1\n", "Effect-x", Some(&l));
        assert!(t.contains("[Effect-x]\nx=1") && t.contains("foo=1"), "{t}");
    }

    #[test]
    fn only_browsers_have_prefs() {
        assert_eq!(
            prefs_js_for(".zen/x y/user.js").as_deref(),
            Some(".zen/x y/prefs.js")
        );
        assert_eq!(prefs_js_for(".config/app/user.js"), None);
    }
}
