//! tmux global appearance only: never source user commands or restart a server.
use crate::{four_apps, palette::Palette, paths};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
    process::Command,
};
pub const OPTIONS: &[(&str, bool)] = &[
    ("status-style", false),
    ("message-style", false),
    ("message-command-style", false),
    ("pane-border-style", true),
    ("pane-active-border-style", true),
    ("window-status-style", true),
    ("window-status-current-style", true),
    ("mode-style", true),
];
pub fn styles(p: &Palette) -> BTreeMap<String, String> {
    let selected = crate::palette::kde_roles(p).selection_text.hex();
    OPTIONS
        .iter()
        .map(|(key, _)| {
            let (fg, bg) = match *key {
                "pane-border-style" => (&p.base[3], None),
                "pane-active-border-style" => (&p.accent, None),
                "window-status-current-style" | "mode-style" => (&selected, Some(&p.accent)),
                _ => (&p.base[5], Some(&p.base[1])),
            };
            (
                key.to_string(),
                format!(
                    "fg={fg}{}",
                    bg.map(|b| format!(",bg={b}")).unwrap_or_default()
                ),
            )
        })
        .collect()
}
pub fn parse(text: &str) -> Result<BTreeMap<String, String>> {
    let values: BTreeMap<String, String> = serde_json::from_str(text)?;
    anyhow::ensure!(
        values.len() == OPTIONS.len(),
        "Incomplete tmux appearance data"
    );
    for (key, value) in &values {
        anyhow::ensure!(
            OPTIONS.iter().any(|(k, _)| k == key),
            "Unsupported tmux option"
        );
        for part in value.split(',') {
            let (name, color) = part.split_once('=').context("Invalid tmux style")?;
            anyhow::ensure!(
                matches!(name, "fg" | "bg"),
                "Only tmux foreground/background are allowed"
            );
            anyhow::ensure!(
                color.len() == 7 && color.starts_with('#'),
                "tmux colours must be #rrggbb"
            );
            crate::palette::Rgb::parse(color)?;
        }
    }
    Ok(values)
}
pub fn script(text: &str) -> Result<String> {
    let values = parse(text)?;
    Ok(OPTIONS
        .iter()
        .map(|(key, window)| {
            format!(
                "set-option -ag{} {key} \"{}\"\n",
                if *window { "w" } else { "" },
                values[*key]
            )
        })
        .collect())
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Server {
    pub socket: PathBuf,
    pub inode: u64,
    pub device: u64,
    pub pid: String,
    pub values: BTreeMap<String, String>,
}
pub type State = BTreeMap<String, Server>;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Change {
    pub before: Server,
    pub after: Server,
}
fn own_socket(path: &Path) -> bool {
    let uid = fs::metadata("/proc/self")
        .map(|m| m.uid())
        .unwrap_or(u32::MAX);
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_socket() && m.uid() == uid)
}
fn output(socket: &Path, args: &[&str]) -> Option<String> {
    let (s, b) = paths::output_bounded(
        Command::new("tmux")
            .arg("-N")
            .arg("-S")
            .arg(socket)
            .args(args),
    )?;
    s.success().then(|| {
        String::from_utf8_lossy(&b)
            .trim_end_matches('\n')
            .to_string()
    })
}
fn sockets() -> Vec<PathBuf> {
    let uid = fs::metadata("/proc/self")
        .map(|m| m.uid())
        .unwrap_or(u32::MAX);
    let base = std::env::var_os("TMUX_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let dir = base.join(format!("tmux-{uid}"));
    let mut out = vec![];
    if fs::symlink_metadata(&dir).is_ok_and(|m| {
        m.is_dir() && !m.file_type().is_symlink() && m.uid() == uid && m.mode() & 0o077 == 0
    }) {
        out.extend(
            fs::read_dir(&dir)
                .ok()
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| own_socket(p)),
        );
    }
    if let Ok(value) = std::env::var("TMUX") {
        if let Some(path) = value.rsplitn(3, ',').nth(2) {
            let p = PathBuf::from(path);
            if own_socket(&p) {
                out.push(p)
            }
        }
    }
    out.sort();
    out.dedup();
    out
}
pub fn capture() -> Result<State> {
    if crate::field::no_apply() || which::which("tmux").is_err() {
        return Ok(State::new());
    }
    let mut state = State::new();
    for socket in sockets() {
        let Some(pid) = output(&socket, &["display-message", "-p", "#{pid}"]) else {
            continue;
        };
        anyhow::ensure!(
            !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()),
            "Invalid tmux server identity"
        );
        let metadata = fs::symlink_metadata(&socket)?;
        let mut values = BTreeMap::new();
        for (key, window) in OPTIONS {
            values.insert(
                key.to_string(),
                output(
                    &socket,
                    &["show-options", if *window { "-gwv" } else { "-gv" }, key],
                )
                .with_context(|| format!("tmux cannot read {key}"))?,
            );
        }
        let server = Server {
            socket: socket.clone(),
            inode: metadata.ino(),
            device: metadata.dev(),
            pid,
            values,
        };
        state.insert(socket.to_string_lossy().into_owned(), server);
    }
    Ok(state)
}
fn same(a: &Server, b: &Server) -> bool {
    a.socket == b.socket && a.inode == b.inode && a.device == b.device && a.pid == b.pid
}
fn replace_style(server: &Server, key: &str, value: &str) -> Result<()> {
    anyhow::ensure!(own_socket(&server.socket), "tmux socket owner changed");
    let m = fs::symlink_metadata(&server.socket)?;
    anyhow::ensure!(
        m.ino() == server.inode
            && m.dev() == server.device
            && output(&server.socket, &["display-message", "-p", "#{pid}"]).as_deref()
                == Some(server.pid.as_str()),
        "tmux server changed during Apply"
    );
    let window = OPTIONS
        .iter()
        .find(|(k, _)| *k == key)
        .context("Invalid tmux option in journal")?
        .1;
    anyhow::ensure!(
        output(
            &server.socket,
            &["set-option", if window { "-gw" } else { "-g" }, key, value]
        )
        .is_some(),
        "Cannot update tmux {key}"
    );
    Ok(())
}
fn set(server: &Server, key: &str, value: &str) -> Result<()> {
    let window = OPTIONS
        .iter()
        .find(|(k, _)| *k == key)
        .context("Invalid tmux style")?
        .1;
    let current = output(
        &server.socket,
        &["show-options", if window { "-gwv" } else { "-gv" }, key],
    )
    .context("Cannot read current tmux style")?;
    replace_style(server, key, &merge_colours(&current, value)?)
}
pub fn restore(changes: &[Change]) -> Result<()> {
    if changes.is_empty() || crate::field::no_apply() {
        return Ok(());
    }
    let current = capture()?;
    for change in changes {
        let Some(now) = current
            .get(&change.before.socket.to_string_lossy().to_string())
            .filter(|s| same(s, &change.before))
        else {
            continue;
        };
        for (key, value) in &change.before.values {
            if now.values.get(key) == change.after.values.get(key) {
                if now.values.get(key) != Some(value) {
                    replace_style(now, key, value)?;
                }
                continue;
            }
            let original = colour_fields(value)?;
            let written = colour_fields(
                change
                    .after
                    .values
                    .get(key)
                    .context("Incomplete tmux journal")?,
            )?;
            let live = colour_fields(now.values.get(key).context("Missing tmux style")?)?;
            let mut delta = vec![];
            for field in ["fg", "bg"] {
                let old = original.get(field).map(String::as_str).unwrap_or("default");
                let applied = written.get(field).map(String::as_str).unwrap_or("default");
                let current = live.get(field).map(String::as_str).unwrap_or("default");
                if old != applied && current == applied {
                    delta.push(format!("{field}={old}"));
                }
            }
            if !delta.is_empty() {
                set(now, key, &delta.join(","))?;
            }
        }
    }
    Ok(())
}
pub fn changes(before: &State, after: &State) -> Vec<Change> {
    before
        .iter()
        .filter_map(|(key, old)| {
            after
                .get(key)
                .filter(|new| same(old, new))
                .map(|new| Change {
                    before: old.clone(),
                    after: new.clone(),
                })
        })
        .collect()
}
pub fn conflicts(before: &State, expected: &State) -> Result<Vec<String>> {
    if before.is_empty() {
        return Ok(vec![]);
    }
    let current = capture()?;
    let mut out = vec![];
    for (id, old) in before {
        if let (Some(now), Some(written)) =
            (current.get(id).filter(|s| same(s, old)), expected.get(id))
        {
            for (key, value) in &now.values {
                let current = colour_fields(value)?;
                let original = colour_fields(&old.values[key])?;
                let applied = colour_fields(&written.values[key])?;
                for field in ["fg", "bg"] {
                    let value = |m: &BTreeMap<String, String>| {
                        m.get(field).cloned().unwrap_or_else(|| "default".into())
                    };
                    if value(&original) != value(&applied)
                        && value(&current) != value(&original)
                        && value(&current) != value(&applied)
                    {
                        out.push(format!(
                            "tmux colour changed after checkpoint: {id} {key} {field}"
                        ));
                    }
                }
            }
        }
    }
    Ok(out)
}
pub fn apply_live() -> Result<()> {
    if crate::field::no_apply() {
        return Ok(());
    }
    let text = fs::read_to_string(paths::live_path(four_apps::TMUX_DATA))?;
    let values = parse(&text)?;
    let before = capture()?;
    if before.is_empty() {
        return Ok(());
    }
    let mut after = before.clone();
    for server in after.values_mut() {
        for (key, value) in &values {
            server
                .values
                .insert(key.clone(), merge_colours(&server.values[key], value)?);
        }
    }
    let changes = changes(&before, &after);
    // Persist the intended before/after before the first external write.
    crate::operation::note_tmux_before(&before)?;
    crate::undo::note_tmux(changes.clone())?;
    let result = (|| {
        for server in before.values() {
            for (key, value) in &values {
                set(server, key, value)?;
            }
        }
        Ok(())
    })();
    if result.is_err() {
        restore(&changes)?;
    } else {
        crate::undo::note_tmux(self::changes(&before, &capture()?))?;
    }
    result
}

/// Split only top-level style members; formats may themselves contain commas.
fn style_members(style: &str) -> Result<Vec<String>> {
    let mut out = vec![];
    let mut stack = vec![];
    let mut start = 0;
    let mut quoted = None;
    let mut escape = false;
    for (i, c) in style
        .char_indices()
        .chain(std::iter::once((style.len(), ',')))
    {
        if escape {
            escape = false;
            continue;
        }
        if c == '\\' {
            escape = true;
            continue;
        }
        if let Some(q) = quoted {
            if c == q {
                quoted = None;
            }
            continue;
        }
        if matches!(c, '\'' | '"') {
            quoted = Some(c);
            continue;
        }
        if matches!(c, '{' | '(' | '[') {
            stack.push(c);
        }
        if matches!(c, '}' | ')' | ']') {
            let open = stack.pop().context("Unbalanced native tmux style")?;
            anyhow::ensure!(
                matches!((open, c), ('{', '}') | ('(', ')') | ('[', ']')),
                "Unbalanced native tmux style"
            );
        }
        if c == ',' && stack.is_empty() {
            let value = style[start..i].trim();
            if !value.is_empty() {
                out.push(value.to_string());
            }
            start = i + 1;
        }
    }
    anyhow::ensure!(
        stack.is_empty() && quoted.is_none(),
        "Unsupported native tmux style expression"
    );
    Ok(out)
}
fn colour_fields(style: &str) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for part in style_members(style)? {
        if part == "default" {
            out.clear();
        }
        if let Some((key, value)) = part.split_once('=') {
            if matches!(key, "fg" | "bg") {
                let v = if value.len() == 7 && value.starts_with('#') {
                    value.to_ascii_lowercase()
                } else {
                    value.to_string()
                };
                out.insert(key.into(), v);
            }
        }
    }
    Ok(out)
}
fn merge_colours(style: &str, delta: &str) -> Result<String> {
    let values = colour_fields(delta)?;
    let mut out = style_members(style)?;
    out.retain(|part| {
        !part
            .split_once('=')
            .is_some_and(|(key, _)| values.contains_key(key))
    });
    out.extend(
        values
            .into_iter()
            .map(|(key, value)| format!("{key}={value}")),
    );
    Ok(out.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn styles_keep_nested_formats_and_select_last_colour() {
        let v = colour_fields("fg=#{?active,red,blue},bold,bg=black,fg=#AABBCC").unwrap();
        assert_eq!(v["fg"], "#aabbcc");
        assert_eq!(v["bg"], "black");
        assert_eq!(
            colour_fields("fg=#{?active,red,blue},bold").unwrap()["fg"],
            "#{?active,red,blue}"
        );
        assert!(colour_fields("fg=#{invalid").is_err());
    }
}
