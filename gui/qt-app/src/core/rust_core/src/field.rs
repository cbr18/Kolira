//! Theme field engine: read and write single theme values (keys) in config
//! files, then run the live command / reload hook — the Rust twin of the
//! Python inspector core (`theme_core.py`).
//!
//! The registry is `gui/qt-app/src/core/theme_fields.json`, compiled in here
//! and loaded by Python at runtime, so both cores touch exactly the same keys.
//! Formats: KDE/GTK INI, kitty `key value`, Alacritty TOML, JSON/JSONC (patched
//! in place by value position, never re-serialised), Zen `user.js`, and the
//! active Kvantum theme's kvconfig.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use serde_json::Value;

use crate::paths::{home_dir, live_path};

#[derive(Debug, Clone, Deserialize)]
pub struct FieldDef {
    pub id: String,
    pub section: String,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub source: String,
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub path: Vec<String>,
    #[serde(default)]
    pub choices: String,
    #[serde(default = "default_apply")]
    pub apply: String,
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub hook: String,
    #[serde(default = "yes")]
    pub profile_tracked: bool,
}

fn default_apply() -> String {
    "config".into()
}
fn yes() -> bool {
    true
}

#[derive(Deserialize)]
struct Registry {
    fields: Vec<FieldDef>,
}

const REGISTRY_JSON: &str = include_str!("../../theme_fields.json");

/// Files written through `kwriteconfig6` (KConfig escaping), as in Python.
const KDE_FILES: &[&str] = &[
    ".config/kdeglobals",
    ".config/kwinrc",
    ".config/kwinrulesrc",
    ".config/plasmarc",
    ".config/plasmashellrc",
    ".config/kcminputrc",
    ".config/klassy/klassyrc",
    ".config/klassy/windecopresetsrc",
];

/// Fields applied first, in this order: Look & Feel can reset colours and
/// styles, and the accent command needs the scheme already in place.
const APPLY_FIRST: &[&str] = &[
    "look_and_feel",
    "plasma_theme",
    "color_scheme",
    "accent_from_wallpaper",
    "accent_color",
    "widget_style",
    "kvantum_theme",
];

pub fn registry() -> &'static [FieldDef] {
    static FIELDS: OnceLock<Vec<FieldDef>> = OnceLock::new();
    FIELDS.get_or_init(|| {
        serde_json::from_str::<Registry>(REGISTRY_JSON)
            .expect("theme_fields.json is valid (checked by tests)")
            .fields
    })
}

pub fn field(id: &str) -> Option<&'static FieldDef> {
    registry().iter().find(|f| f.id == id)
}

fn no_apply() -> bool {
    std::env::var("KTM_THEME_NO_APPLY").as_deref() == Ok("1")
}

// ─────────────────────────────────────────────────────────────────────────────
// Where a field lives
// ─────────────────────────────────────────────────────────────────────────────

/// Root a field is read from: the live system or a profile directory that
/// mirrors `$HOME` (the historical snapshot layout).
#[derive(Debug, Clone)]
pub enum Base {
    Live,
    Snapshot(PathBuf),
}

impl Base {
    pub fn resolve(&self, relative: &str) -> PathBuf {
        match self {
            Base::Live => live_path(relative),
            Base::Snapshot(root) => root.join(relative),
        }
    }
}

fn read_text(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|b| String::from_utf8_lossy(&b).into_owned())
}

fn kvantum_file(theme: &str, base: &Base) -> Option<PathBuf> {
    if theme.is_empty() || theme.contains('/') {
        return None;
    }
    let mut roots = vec![base.resolve(".config/Kvantum")];
    if matches!(base, Base::Live) {
        roots.push(PathBuf::from("/usr/share/Kvantum"));
    }
    for root in roots {
        let direct = root.join(format!("{theme}.kvconfig"));
        if direct.is_file() {
            return Some(direct);
        }
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let candidate = entry.path().join(format!("{theme}.kvconfig"));
                if entry.path().is_dir() && candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

pub(crate) fn file_for(f: &FieldDef, base: &Base) -> Option<PathBuf> {
    if f.source == "kvantum" {
        let theme = read_raw(field("kvantum_theme")?, base)?;
        return kvantum_file(value_str(&theme).as_str(), base);
    }
    if f.file.is_empty() {
        return None;
    }
    let path = base.resolve(&f.file);
    if f.source == "userjs" && !path.is_file() {
        // Zen profile ids differ between installs: first profile with a user.js.
        let zen = base.resolve(".zen");
        let mut found: Vec<PathBuf> = std::fs::read_dir(&zen)
            .ok()?
            .flatten()
            .map(|e| e.path().join("user.js"))
            .filter(|p| p.is_file())
            .collect();
        found.sort();
        return found.into_iter().next().or(Some(path));
    }
    Some(path)
}

// ─────────────────────────────────────────────────────────────────────────────
// Format helpers (mirror theme_core.py)
// ─────────────────────────────────────────────────────────────────────────────

/// Last value of `key` in `[group]` (lenient KConfig/GTK INI).
pub fn ini_get(text: &str, group: &str, key: &str) -> Option<String> {
    let mut current = String::new();
    let mut found = None;
    for line in text.lines() {
        let s = line.trim();
        if s.is_empty() || s.starts_with('#') || s.starts_with(';') {
            continue;
        }
        if s.starts_with('[') && s.ends_with(']') {
            current = s[1..s.len() - 1].to_string();
            continue;
        }
        if current == group {
            if let Some((k, v)) = s.split_once('=') {
                if k.trim() == key {
                    found = Some(v.trim().to_string());
                }
            }
        }
    }
    found
}

/// Line-preserving INI update; creates the group/key when missing.
pub fn set_ini_text(text: &str, group: &str, key: &str, value: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let mut current = String::new();
    let mut header: Option<usize> = None;
    let mut group_end = lines.len();
    let mut key_line: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        let s = line.trim();
        if s.starts_with('[') && s.ends_with(']') {
            let name = &s[1..s.len() - 1];
            if current == group && header.is_some() && group_end == lines.len() {
                group_end = i;
            }
            if name == group {
                header = Some(i);
            }
            current = name.to_string();
            continue;
        }
        if current == group {
            if let Some((k, _)) = s.split_once('=') {
                if k.trim() == key {
                    key_line = Some(i);
                }
            }
        }
    }
    if let Some(i) = key_line {
        let indent: String = lines[i].chars().take_while(|c| c.is_whitespace()).collect();
        lines[i] = format!("{indent}{key}={value}");
    } else if header.is_some() {
        lines.insert(group_end, format!("{key}={value}"));
    } else if !group.is_empty() {
        if lines.last().is_some_and(|l| !l.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push(format!("[{group}]"));
        lines.push(format!("{key}={value}"));
    } else {
        lines.push(format!("{key}={value}"));
    }
    lines.join("\n") + "\n"
}

pub fn kv_get(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let s = line.trim();
        if s.is_empty() || s.starts_with('#') {
            continue;
        }
        let mut parts = s.splitn(2, char::is_whitespace);
        if parts.next() == Some(key) {
            return Some(parts.next().unwrap_or("").trim().to_string());
        }
    }
    None
}

pub fn kv_set(text: &str, key: &str, value: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    for line in lines.iter_mut() {
        let s = line.trim();
        if !s.is_empty() && !s.starts_with('#') && s.split_whitespace().next() == Some(key) {
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            *line = format!("{indent}{key} {value}");
            return lines.join("\n") + "\n";
        }
    }
    lines.push(format!("{key} {value}"));
    lines.join("\n") + "\n"
}

/// Byte spans of every scalar in a JSON/JSONC document, keyed by path
/// (array items by index). Comments and trailing commas are tolerated.
pub fn json_positions(text: &str) -> std::collections::HashMap<Vec<String>, (usize, usize)> {
    let b = text.as_bytes();
    let n = b.len();
    let mut out = std::collections::HashMap::new();

    fn skip_ws(b: &[u8], mut i: usize) -> usize {
        let n = b.len();
        while i < n {
            match b[i] {
                b' ' | b'\t' | b'\r' | b'\n' => i += 1,
                b'/' if i + 1 < n && b[i + 1] == b'/' => {
                    while i < n && b[i] != b'\n' {
                        i += 1;
                    }
                }
                b'/' if i + 1 < n && b[i + 1] == b'*' => {
                    i += 2;
                    while i + 1 < n && !(b[i] == b'*' && b[i + 1] == b'/') {
                        i += 1;
                    }
                    i = (i + 2).min(n);
                }
                _ => break,
            }
        }
        i
    }
    fn string_end(b: &[u8], mut i: usize) -> usize {
        i += 1;
        while i < b.len() {
            if b[i] == b'\\' {
                i += 2;
                continue;
            }
            if b[i] == b'"' {
                return i + 1;
            }
            i += 1;
        }
        b.len()
    }
    fn value(
        text: &str,
        b: &[u8],
        i: usize,
        path: Vec<String>,
        out: &mut std::collections::HashMap<Vec<String>, (usize, usize)>,
    ) -> usize {
        let n = b.len();
        let mut i = skip_ws(b, i);
        if i >= n {
            return n;
        }
        match b[i] {
            b'{' => {
                i += 1;
                loop {
                    i = skip_ws(b, i);
                    if i >= n {
                        return n;
                    }
                    if b[i] == b'}' {
                        return i + 1;
                    }
                    if b[i] != b'"' {
                        return n;
                    }
                    let end = string_end(b, i);
                    let key = serde_json::from_str::<String>(&text[i..end])
                        .unwrap_or_else(|_| text[i + 1..end.saturating_sub(1)].to_string());
                    i = skip_ws(b, end);
                    if i < n && b[i] == b':' {
                        i += 1;
                    }
                    let mut child = path.clone();
                    child.push(key);
                    i = skip_ws(b, value(text, b, i, child, out));
                    if i < n && b[i] == b',' {
                        i += 1;
                    } else if i < n && b[i] == b'}' {
                        return i + 1;
                    } else if i >= n {
                        return n;
                    }
                }
            }
            b'[' => {
                i += 1;
                let mut item = 0usize;
                loop {
                    i = skip_ws(b, i);
                    if i >= n {
                        return n;
                    }
                    if b[i] == b']' {
                        return i + 1;
                    }
                    let mut child = path.clone();
                    child.push(item.to_string());
                    item += 1;
                    i = skip_ws(b, value(text, b, i, child, out));
                    if i < n && b[i] == b',' {
                        i += 1;
                    } else if i < n && b[i] == b']' {
                        return i + 1;
                    } else if i >= n {
                        return n;
                    }
                }
            }
            b'"' => {
                let end = string_end(b, i);
                out.insert(path, (i, end));
                end
            }
            _ => {
                let start = i;
                while i < n && !matches!(b[i], b',' | b'}' | b']' | b' ' | b'\t' | b'\r' | b'\n') {
                    i += 1;
                }
                out.insert(path, (start, i));
                i
            }
        }
    }
    let start = skip_ws(b, 0);
    value(text, b, start, Vec::new(), &mut out);
    let _ = n;
    out
}

pub fn json_get(text: &str, path: &[String]) -> Option<Value> {
    let (s, e) = *json_positions(text).get(path)?;
    let fragment = &text[s..e];
    Some(serde_json::from_str(fragment).unwrap_or_else(|_| Value::String(fragment.trim_matches('"').to_string())))
}

pub fn json_set(text: &str, path: &[String], value: &Value) -> Result<String> {
    let literal = serde_json::to_string(value)?;
    if let Some((s, e)) = json_positions(text).get(path).copied() {
        return Ok(format!("{}{}{}", &text[..s], literal, &text[e..]));
    }
    if path.len() == 1 {
        if let Some(close) = text.rfind('}') {
            let body = text[..close].trim_end();
            let sep = if body.ends_with('{') || body.ends_with(',') { "" } else { "," };
            return Ok(format!(
                "{body}{sep}\n  {}: {literal}\n{}",
                serde_json::to_string(&path[0])?,
                &text[close..]
            ));
        }
    }
    Err(anyhow!("JSON path not found: {}", path.join(".")))
}

fn toml_get(text: &str, key: &str, path: &[String]) -> Option<Value> {
    let doc: toml::Value = toml::from_str(text).ok()?;
    let mut node = &doc;
    for part in path.iter().map(String::as_str).chain(std::iter::once(key)) {
        node = node.get(part)?;
    }
    serde_json::to_value(node).ok()
}

fn toml_set(text: &str, key: &str, value: &Value, section: &str) -> Result<String> {
    let mut doc: toml_edit::DocumentMut = text.parse().context("invalid TOML")?;
    let item = match value {
        Value::Bool(b) => toml_edit::value(*b),
        Value::Number(n) if n.is_i64() => toml_edit::value(n.as_i64().unwrap_or_default()),
        Value::Number(n) => toml_edit::value(n.as_f64().unwrap_or_default()),
        other => toml_edit::value(value_str(other)),
    };
    if section.is_empty() {
        doc[key] = item;
    } else {
        if doc.get(section).is_none() {
            doc[section] = toml_edit::Item::Table(toml_edit::Table::new());
        }
        doc[section][key] = item;
    }
    Ok(doc.to_string())
}

fn userjs_span(text: &str, key: &str) -> Option<(usize, usize, String)> {
    let needle = format!("\"{key}\"");
    let mut from = 0;
    while let Some(rel) = text[from..].find("user_pref(") {
        let start = from + rel;
        let after = &text[start + "user_pref(".len()..];
        let trimmed = after.trim_start();
        if trimmed.starts_with(&needle) {
            let end_rel = after.find(");")?;
            let inner = &after[..end_rel];
            let comma = inner.find(',')?;
            let raw = inner[comma + 1..].trim().to_string();
            let end = start + "user_pref(".len() + end_rel + 2;
            return Some((start, end, raw));
        }
        from = start + 1;
    }
    None
}

fn userjs_get(text: &str, key: &str) -> Option<Value> {
    let (_, _, raw) = userjs_span(text, key)?;
    Some(serde_json::from_str(&raw).unwrap_or_else(|_| Value::String(raw.trim_matches('"').to_string())))
}

fn userjs_set(text: &str, key: &str, value: &Value) -> String {
    let literal = match value {
        Value::String(s) => serde_json::to_string(s).unwrap_or_default(),
        other => value_str(other),
    };
    let line = format!("user_pref(\"{key}\", {literal});");
    match userjs_span(text, key) {
        Some((s, e, _)) => format!("{}{}{}", &text[..s], line, &text[e..]),
        None => {
            let sep = if text.is_empty() || text.ends_with('\n') { "" } else { "\n" };
            format!("{text}{sep}{line}\n")
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Values
// ─────────────────────────────────────────────────────────────────────────────

fn fmt_number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v}");
        s
    }
}

/// Plain-text form of a value (what goes into INI/kv files).
pub fn value_str(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => if *b { "true" } else { "false" }.into(),
        Value::Number(n) => n.as_f64().map(fmt_number).unwrap_or_else(|| n.to_string()),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn as_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        other => match value_str(other).trim().to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        },
    }
}

fn color_parts(v: &Value) -> Option<(u8, u8, u8)> {
    let s = value_str(v);
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        let hex: String = if hex.len() == 3 || hex.len() == 4 {
            hex.chars().take(3).flat_map(|c| [c, c]).collect()
        } else {
            hex.to_string()
        };
        if hex.len() >= 6 {
            let p = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
            return Some((p(0)?, p(2)?, p(4)?));
        }
        return None;
    }
    let parts: Vec<&str> = s.split(',').map(str::trim).collect();
    if parts.len() >= 3 {
        let p = |x: &str| x.parse::<i64>().ok().map(|n| n.clamp(0, 255) as u8);
        return Some((p(parts[0])?, p(parts[1])?, p(parts[2])?));
    }
    None
}

pub fn color_hex(v: &Value) -> String {
    color_parts(v).map(|(r, g, b)| format!("#{r:02x}{g:02x}{b:02x}")).unwrap_or_default()
}

/// Type the raw file value the way Python's `_coerce` does.
pub fn coerce(f: &FieldDef, raw: Value) -> Option<Value> {
    match f.kind.as_str() {
        "bool" => as_bool(&raw).map(Value::Bool),
        "int" => value_str(&raw).trim().parse::<f64>().ok().map(|n| Value::from(n as i64)),
        "float" => value_str(&raw).trim().parse::<f64>().ok().map(Value::from),
        _ => match raw {
            Value::Null => None,
            Value::String(s) => Some(Value::String(s)),
            other => Some(Value::String(value_str(&other))),
        },
    }
}

/// Validate/convert user input to the raw form written to the file
/// (Python `_normalize_input`): colours become `r,g,b`, numbers numbers.
pub fn normalize_input(f: &FieldDef, v: &Value) -> Option<Value> {
    if f.id == "zen_window_scheme" {
        let n = value_str(v).trim().parse::<f64>().ok()? as i64;
        return (0..=2).contains(&n).then(|| Value::from(n));
    }
    match f.kind.as_str() {
        "bool" => as_bool(v).map(Value::Bool),
        "int" => value_str(v).trim().parse::<f64>().ok().map(|n| Value::from(n as i64)),
        "float" => value_str(v).trim().parse::<f64>().ok().map(Value::from),
        "color" => color_parts(v).map(|(r, g, b)| Value::String(format!("{r},{g},{b}"))),
        _ => Some(Value::String(value_str(v))),
    }
}

/// Current value of a field (coerced), or None when unset/unreadable.
pub fn read(f: &FieldDef, base: &Base) -> Option<Value> {
    read_raw(f, base).and_then(|raw| coerce(f, raw))
}

fn read_raw(f: &FieldDef, base: &Base) -> Option<Value> {
    let path = file_for(f, base)?;
    let text = read_text(&path)?;
    match f.source.as_str() {
        "ini" | "kvantum" => {
            let mut value = ini_get(&text, &f.group, &f.key)?;
            if f.source == "ini" && f.apply == "ini" && value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
                value = value[1..value.len() - 1].to_string();
            }
            Some(Value::String(value))
        }
        "kv" => kv_get(&text, &f.key).map(Value::String),
        "toml" => toml_get(&text, &f.key, &f.path),
        "json" => json_get(&text, &f.path),
        "userjs" => userjs_get(&text, &f.key),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Writing and applying
// ─────────────────────────────────────────────────────────────────────────────

fn write_atomic(path: &Path, text: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mode = std::fs::metadata(path).map(|m| m.permissions().mode()).ok();
    let tmp = path.with_extension("tmp-theme-field");
    std::fs::write(&tmp, text)?;
    if let Some(mode) = mode {
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode))?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn run(cmd: &str, args: &[&str]) -> bool {
    if no_apply() {
        return true;
    }
    std::process::Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn write_config(f: &FieldDef, path: &Path, value: &Value) -> Result<()> {
    if KDE_FILES.contains(&f.file.as_str()) && !no_apply() && which::which("kwriteconfig6").is_ok() {
        let path_str = path.to_string_lossy();
        let text = value_str(value);
        let mut args = vec!["--file", path_str.as_ref(), "--group", f.group.as_str(), "--key", f.key.as_str()];
        if f.kind == "bool" {
            args.extend(["--type", "bool"]);
        }
        args.push(text.as_str());
        if run("kwriteconfig6", &args) {
            return Ok(());
        }
    }
    let text = read_text(path).unwrap_or_default();
    write_atomic(path, &set_ini_text(&text, &f.group, &f.key, &value_str(value)))
}

/// Write one raw value; returns the touched file.
pub fn write(f: &FieldDef, value: &Value) -> Result<PathBuf> {
    let path = file_for(f, &Base::Live).ok_or_else(|| anyhow!("cannot resolve the file for {}", f.id))?;
    if f.source == "kvantum" {
        if !path.is_file() {
            return Err(anyhow!("active Kvantum theme file not found"));
        }
        let text = read_text(&path).unwrap_or_default();
        write_atomic(&path, &set_ini_text(&text, &f.group, &f.key, &value_str(value)))?;
        return Ok(path);
    }
    let text = || read_text(&path).unwrap_or_default();
    match f.apply.as_str() {
        "config" | "cmd_config" => write_config(f, &path, value)?,
        "ini" => write_atomic(&path, &set_ini_text(&text(), &f.group, &f.key, &value_str(value)))?,
        "kv" => write_atomic(&path, &kv_set(&text(), &f.key, &value_str(value)))?,
        "toml" => {
            let section = f.path.first().map(String::as_str).unwrap_or("");
            write_atomic(&path, &toml_set(&text(), &f.key, value, section)?)?
        }
        "json" => {
            let current = read_text(&path).unwrap_or_else(|| "{}".into());
            write_atomic(&path, &json_set(&current, &f.path, value)?)?
        }
        "userjs" => write_atomic(&path, &userjs_set(&text(), &f.key, value))?,
        other => return Err(anyhow!("unsupported apply mode {other} for {}", f.id)),
    }
    Ok(path)
}

fn command_args(f: &FieldDef, value: &Value) -> Vec<String> {
    let live = |id: &str| field(id).and_then(|x| read(x, &Base::Live)).map(|v| value_str(&v)).unwrap_or_default();
    let cursor = {
        let c = live("cursor_theme");
        if c.is_empty() { "default".to_string() } else { c }
    };
    f.command
        .iter()
        .map(|part| {
            part.replace("{value}", &value_str(value))
                .replace("{hex}", &color_hex(value))
                .replace("{scheme}", &live("color_scheme"))
                .replace("{cursor}", &cursor)
                .replace("{theme}", &live("kvantum_theme"))
        })
        .collect()
}

fn run_hook(f: &FieldDef) {
    match f.hook.as_str() {
        "" => {}
        "kwin" => {
            run("qdbus6", &["org.kde.KWin", "/KWin", "reconfigure"]);
        }
        "kbuildsycoca" => {
            run("kbuildsycoca6", &[]);
        }
        "kitty" => {
            let config = live_path(".config/kitty/kitty.conf").to_string_lossy().to_string();
            let opacity = read_text(Path::new(&config)).and_then(|t| kv_get(&t, "background_opacity"));
            for socket in crate::palette::kitty_sockets() {
                run("kitty", &["@", "--to", &socket, "load-config", &config]);
                if let Some(o) = opacity.as_deref() {
                    run("kitty", &["@", "--to", &socket, "set-background-opacity", "--all", o]);
                }
            }
        }
        hook => {
            if let Some(effect) = hook.strip_prefix("effect:") {
                if f.kind == "bool" {
                    let on = read(f, &Base::Live).and_then(|v| as_bool(&v)).unwrap_or(false);
                    run("qdbus6", &["org.kde.KWin", "/Effects", if on { "loadEffect" } else { "unloadEffect" }, effect]);
                }
                run("qdbus6", &["org.kde.KWin", "/Effects", "reconfigureEffect", effect]);
            }
        }
    }
}

/// Outcome of applying a set of values.
#[derive(Debug, Default)]
pub struct ApplyReport {
    pub applied: Vec<String>,
    pub errors: Vec<String>,
}

/// Write `values` (field id → value), run live commands and each hook once.
/// Look & Feel, Plasma style, colour scheme and accent go first.
pub fn apply(values: &[(String, Value)]) -> ApplyReport {
    let rank = |id: &str| APPLY_FIRST.iter().position(|x| *x == id).unwrap_or(APPLY_FIRST.len());
    let mut ordered: Vec<&(String, Value)> = values.iter().collect();
    ordered.sort_by_key(|(id, _)| rank(id));

    let mut report = ApplyReport::default();
    let mut hooks_seen: Vec<String> = Vec::new();
    for (id, value) in ordered {
        let Some(f) = field(id) else {
            report.errors.push(format!("{id}: unknown field"));
            continue;
        };
        let Some(raw) = normalize_input(f, value) else {
            report.errors.push(format!("{id}: invalid value {value}"));
            continue;
        };
        // Fields with a live command (plasma-apply-colorscheme, -lookandfeel,
        // -desktoptheme, -cursortheme, kvantummanager): run it *first* — the
        // tool writes the key itself. Writing the key beforehand made e.g.
        // plasma-apply-colorscheme answer "already in use" and skip the live
        // update. The key is written directly only when the command is not
        // run (KTM_THEME_NO_APPLY) or fails.
        let mut done = false;
        if f.apply == "cmd_config" && !f.command.is_empty() && !no_apply() {
            let args = command_args(f, &raw);
            if let Some((cmd, rest)) = args.split_first() {
                let rest: Vec<&str> = rest.iter().map(String::as_str).collect();
                done = which::which(cmd).is_ok() && run(cmd, &rest);
            }
        }
        if !done {
            if let Err(e) = write(f, &raw) {
                report.errors.push(format!("{id}: {e:#}"));
                continue;
            }
        }
        if !f.hook.is_empty() && !hooks_seen.contains(&f.hook) {
            hooks_seen.push(f.hook.clone());
            run_hook(f);
        }
        report.applied.push(id.clone());
    }
    report
}

/// Snapshot of every profile-tracked field that has a value under `base`.
pub fn capture(base: &Base) -> Vec<(String, Value)> {
    registry()
        .iter()
        .filter(|f| f.profile_tracked)
        .filter_map(|f| read(f, base).map(|v| (f.id.clone(), v)))
        .collect()
}

/// `$HOME`, exposed for callers that build snapshot bases.
pub fn home() -> PathBuf {
    home_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn registry_loads_all_fields() {
        let ids: Vec<&str> = registry().iter().map(|f| f.id.as_str()).collect();
        assert!(ids.len() >= 70, "{}", ids.len());
        for id in ["accent_color", "color_scheme", "kvantum_theme", "blur_menus", "breeze_menu_opacity", "zen_window_scheme"] {
            assert!(ids.contains(&id), "{id}");
        }
        assert!(!field("cursor_theme").unwrap().profile_tracked);
    }

    #[test]
    fn ini_roundtrip_preserves_lines() {
        let text = "# c\n[General]\nColorScheme=Breeze\nName=x\n\n[KDE]\ncontrast=4\n";
        let out = set_ini_text(text, "General", "ColorScheme", "Everforest");
        assert_eq!(out, "# c\n[General]\nColorScheme=Everforest\nName=x\n\n[KDE]\ncontrast=4\n");
        let out = set_ini_text(&out, "KDE", "widgetStyle", "Breeze");
        assert!(out.contains("[KDE]\ncontrast=4\nwidgetStyle=Breeze\n"));
        let out = set_ini_text(&out, "Colors:Window", "BackgroundNormal", "1,2,3");
        assert!(out.ends_with("\n[Colors:Window]\nBackgroundNormal=1,2,3\n"));
        assert_eq!(ini_get(&out, "Colors:Window", "BackgroundNormal").as_deref(), Some("1,2,3"));
        assert_eq!(ini_get("[a]\nk=1\n[a]\nk=2\n", "a", "k").as_deref(), Some("2"));
    }

    #[test]
    fn jsonc_patch_keeps_comments_and_layout() {
        let text = "{\n  // theme\n  \"theme\": {\"mode\": \"dark\", \"dark\": \"One\",},\n  \"ui_font_size\": 15,\n}\n";
        let path = vec!["theme".to_string(), "dark".to_string()];
        assert_eq!(json_get(text, &path), Some(json!("One")));
        let out = json_set(text, &path, &json!("Ayu Dark")).unwrap();
        assert!(out.contains("// theme") && out.contains("\"dark\": \"Ayu Dark\""));
        let out = json_set(&out, &["new_key".to_string()], &json!(true)).unwrap();
        assert!(out.contains("\"new_key\": true"));
        assert!(json_set("{}", &["a".into(), "b".into()], &json!(1)).is_err());
    }

    #[test]
    fn kv_toml_userjs() {
        let kv = "font_size 11\nbackground_opacity 0.5\n";
        assert_eq!(kv_get(kv, "background_opacity").as_deref(), Some("0.5"));
        assert_eq!(kv_set(kv, "background_opacity", "1"), "font_size 11\nbackground_opacity 1\n");
        let toml = "[window]\nopacity = 0.5 # keep\n";
        let out = toml_set(toml, "opacity", &json!(0.9), "window").unwrap();
        assert!(out.contains("opacity = 0.9") && out.contains("[window]"));
        assert_eq!(toml_get(&out, "opacity", &["window".to_string()]), Some(json!(0.9)));
        let js = "user_pref(\"a.b\", true);\nuser_pref(\"zen.view.window.scheme\", 0);\n";
        assert_eq!(userjs_get(js, "zen.view.window.scheme"), Some(json!(0)));
        let out = userjs_set(js, "zen.view.window.scheme", &json!(1));
        assert!(out.contains("user_pref(\"zen.view.window.scheme\", 1);") && out.contains("\"a.b\", true"));
        assert!(userjs_set("", "x.y", &json!("s")).contains("user_pref(\"x.y\", \"s\");"));
    }

    #[test]
    fn values_normalise_like_python() {
        let accent = field("accent_color").unwrap();
        assert_eq!(normalize_input(accent, &json!("#a7c080")), Some(json!("167,192,128")));
        assert_eq!(color_hex(&json!("167,192,128")), "#a7c080");
        let blur = field("blur_menus").unwrap();
        assert_eq!(normalize_input(blur, &json!("on")), Some(json!(true)));
        assert_eq!(coerce(blur, json!("false")), Some(json!(false)));
        let zen = field("zen_window_scheme").unwrap();
        assert_eq!(normalize_input(zen, &json!(3)), None);
        assert_eq!(value_str(&json!(0.5)), "0.5");
        assert_eq!(value_str(&json!(14.0)), "14");
    }

    #[test]
    fn capture_reads_a_snapshot_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".config/kitty")).unwrap();
        std::fs::write(root.join(".config/kdeglobals"), "[General]\nColorScheme=Nord\nAccentColor=1,2,3\n").unwrap();
        std::fs::write(root.join(".config/kitty/kitty.conf"), "background_opacity 0.8\n").unwrap();
        let values = capture(&Base::Snapshot(root.to_path_buf()));
        let get = |id: &str| values.iter().find(|(k, _)| k == id).map(|(_, v)| v.clone());
        assert_eq!(get("color_scheme"), Some(json!("Nord")));
        assert_eq!(get("accent_color"), Some(json!("1,2,3")));
        assert_eq!(get("kitty_opacity"), Some(json!(0.8)));
        assert_eq!(get("cursor_theme"), None, "untracked fields are not captured");
    }
}
