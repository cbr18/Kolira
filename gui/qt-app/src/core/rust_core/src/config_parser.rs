//! Config Parser Utilities
//!
//! Parsers for various config formats used by tracked applications.

use anyhow::{Context, Result};
use configparser::ini::Ini;
use std::fs;
use std::path::Path;
use toml;

/// Parse INI file (kwinrc, kdeglobals, gtk settings.ini)
pub fn parse_ini(path: &Path) -> Result<Ini> {
    let mut ini = Ini::new();
    ini.load(path)
        .map_err(|e| anyhow::anyhow!("Failed to parse INI file {}: {}", path.display(), e))?;
    Ok(ini)
}

/// Parse TOML file (alacritty.toml)
pub fn parse_toml(path: &Path) -> Result<toml::Value> {
    let content = fs::read_to_string(path)
        .context(format!("Failed to read TOML file: {}", path.display()))?;
    let value =
        toml::from_str(&content).context(format!("Failed to parse TOML: {}", path.display()))?;
    Ok(value)
}

/// Parse JSON/JSONC file (vscode, zed, fastfetch)
pub fn parse_json(path: &Path) -> Result<serde_json::Value> {
    let content = fs::read_to_string(path)
        .context(format!("Failed to read JSON file: {}", path.display()))?;

    // Handle JSONC (JSON with comments) - strip comments
    let content = strip_json_comments(&content);

    let value = serde_json::from_str(&content)
        .context(format!("Failed to parse JSON: {}", path.display()))?;
    Ok(value)
}

/// Strip JSONC comments
fn strip_json_comments(content: &str) -> String {
    let mut result = String::new();
    let mut in_string = false;
    let mut escape_next = false;
    let mut chars = content.chars().peekable();

    while let Some(c) = chars.next() {
        if in_string {
            result.push(c);
            if escape_next {
                escape_next = false;
            } else if c == '\\' {
                escape_next = true;
            } else if c == '"' {
                in_string = false;
            }
        } else {
            if c == '/' && chars.peek() == Some(&'/') {
                // Single-line comment - skip to end of line
                chars.next(); // consume second /
                for c in chars.by_ref() {
                    if c == '\n' {
                        result.push(c);
                        break;
                    }
                }
            } else if c == '/' && chars.peek() == Some(&'*') {
                // Multi-line comment - skip until */
                chars.next(); // consume *
                let mut prev = '\0';
                for c in chars.by_ref() {
                    if prev == '*' && c == '/' {
                        break;
                    }
                    prev = c;
                }
            } else {
                if c == '"' {
                    in_string = true;
                }
                result.push(c);
            }
        }
    }
    result
}

/// Parse CSS file (gtk.css, wofi style.css)
pub fn parse_css(path: &Path) -> Result<String> {
    let content =
        fs::read_to_string(path).context(format!("Failed to read CSS file: {}", path.display()))?;
    Ok(content)
}

/// Parse user.js (Zen Browser)
pub fn parse_user_js(path: &Path) -> Result<Vec<(String, String)>> {
    let content =
        fs::read_to_string(path).context(format!("Failed to read user.js: {}", path.display()))?;

    let mut prefs = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("user_pref(") {
            // Parse: user_pref("key", value);
            if let Some(start) = line.find('"') {
                if let Some(end) = line[start + 1..].find('"') {
                    let key = line[start + 1..start + 1 + end].to_string();
                    let value_part = &line[start + 1 + end + 2..]; // skip ", "
                    if let Some(value_end) = value_part.rfind(')') {
                        let value = value_part[..value_end].trim().to_string();
                        prefs.push((key, value));
                    }
                }
            }
        }
    }
    Ok(prefs)
}

/// Extract specific value from INI
pub fn get_ini_value(ini: &Ini, section: &str, key: &str) -> Option<String> {
    ini.get(section, key)
}

/// Extract specific value from TOML
pub fn get_toml_value(value: &toml::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}

/// Extract specific value from JSON
pub fn get_json_value(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tolerant single-value readers used by `theme-profile status`.
//
// These deliberately do NOT parse the whole file: KConfig files may contain
// unquoted values and JSONC files may contain comments and trailing commas.
// Each reader only looks for the exact key in the exact position, which is
// what makes `status` fast (single pass, no full reserialisation) and immune
// to the "fragments of JSON in the status output" bug.
// ─────────────────────────────────────────────────────────────────────────────

/// Read `key=value` from the first line starting with `prefix` (`theme=`,
/// `gtk-theme-name=`, `background_opacity `, …).
pub fn line_key_value(path: &Path, prefix: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix(prefix) {
            let value = rest.trim_end().trim();
            return Some(value.to_string());
        }
    }
    None
}

/// Read the (last) value of `key` inside `[group]` of an INI/KConfig file.
pub fn ini_group_key(path: &Path, group: &str, key: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let want = format!("[{group}]");
    let mut in_group = false;
    let mut value: Option<String> = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_group = trimmed == want;
            continue;
        }
        if in_group {
            if let Some((k, v)) = trimmed.split_once('=') {
                if k.trim() == key {
                    value = Some(v.trim().to_string());
                }
            }
        }
    }
    value
}

/// Lines inside `[group]` (diagnostic extraction, matching `write_ini_group`).
pub fn ini_group_lines(path: &Path, group: &str) -> Vec<String> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let want = format!("[{group}]");
    let mut in_group = false;
    let mut out = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_group = trimmed == want;
            continue;
        }
        if in_group {
            out.push(trimmed.to_string());
        }
    }
    out
}

/// `user_pref("key", value);` — value returned unquoted.
pub fn user_js_pref(path: &Path, key: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let needle = format!("user_pref(\"{key}\",");
    for line in content.lines() {
        let line = line.trim_start();
        if line.starts_with(&needle) {
            let rest = &line[needle.len()..];
            let mut value = rest.trim().to_string();
            if let Some(idx) = value.rfind(')') {
                value.truncate(idx);
                value = value.trim().to_string();
            }
            if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                value = value[1..value.len() - 1].to_string();
            }
            return Some(value);
        }
    }
    None
}

/// Top-level `"key": "value"` in a (JSONC) file. The key must start the line
/// (after optional whitespace and an opening `{` for one-line objects) — this
/// is what stops `status` from echoing JSON fragments.
pub fn json_key_value(path: &Path, key: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let key_prefix = format!("\"{key}\"");
    for line in content.lines() {
        let mut rest = line.trim_start();
        if let Some(stripped) = rest.strip_prefix('{') {
            rest = stripped.trim_start();
        }
        let Some(tail) = rest.strip_prefix(&key_prefix) else {
            continue;
        };
        let tail = tail.trim_start();
        let Some(after) = tail.strip_prefix(':') else {
            continue;
        };
        let value = after.trim_start();
        let Some(inner) = value.strip_prefix('"') else {
            continue;
        };
        if let Some(end) = inner.find('"') {
            return Some(inner[..end].to_string());
        }
    }
    None
}

/// `"member": "value"` inside the first `"object": {` block of a JSONC file.
/// Tolerates comments and trailing commas by only matching quoted keys.
pub fn json_nested_value(path: &Path, object: &str, member: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let object_key = format!("\"{object}\"");
    let member_key = format!("\"{member}\"");
    let mut in_object = false;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if !in_object {
            if let Some(rest) = trimmed.strip_prefix(&object_key) {
                in_object = rest.trim_start().starts_with(':');
            }
            continue;
        }
        if trimmed.starts_with('}') {
            return None;
        }
        let trimmed = trimmed.trim_start();
        let Some(rest) = trimmed.strip_prefix(&member_key) else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(after) = rest.strip_prefix(':') else {
            continue;
        };
        let value = after.trim_start();
        let Some(inner) = value.strip_prefix('"') else {
            continue;
        };
        if let Some(end) = inner.find('"') {
            return Some(inner[..end].to_string());
        }
    }
    None
}
