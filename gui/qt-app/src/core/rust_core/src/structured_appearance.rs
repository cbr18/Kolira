//! Small native selectors: patch spans, retain comments and unrelated personal data.
use anyhow::Result;
use std::{collections::BTreeMap, ops::Range};
#[derive(Clone)]
struct Span {
    range: Range<usize>,
    raw: String,
}
const GIMP: &str = ".config/GIMP/3.0/gimprc";
const LSD: &str = ".config/lsd/config.yaml";
const GLOW: &str = ".config/glow/glow.yml";
const MPV: &str = ".config/mpv/mpv.conf";
pub fn handles(rel: &str) -> bool {
    matches!(rel, GIMP | LSD | GLOW | MPV) || rel == crate::third_apps::HEROIC
}
fn allowed(rel: &str, key: &str) -> bool {
    match rel {
        GIMP => matches!(key, "theme" | "theme-color-scheme"),
        LSD => key == "theme",
        GLOW => key == "style",
        MPV => matches!(key, "osd-color" | "osd-border-color" | "osd-shadow-color"),
        _ => false,
    }
}
fn insert(
    map: &mut BTreeMap<String, Span>,
    key: &str,
    start: usize,
    end: usize,
    text: &str,
) -> Result<()> {
    anyhow::ensure!(!map.contains_key(key), "Duplicate appearance key: {key}");
    map.insert(
        key.into(),
        Span {
            range: start..end,
            raw: text[start..end].into(),
        },
    );
    Ok(())
}
fn sexpr(text: &str) -> Result<BTreeMap<String, Span>> {
    let mut out = BTreeMap::new();
    let b = text.as_bytes();
    let (mut i, mut depth, mut start) = (0, 0, 0);
    while i < b.len() {
        match b[i] {
            b'#' => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'"' => {
                i += 1;
                loop {
                    anyhow::ensure!(i < b.len(), "Unclosed GIMP string");
                    if b[i] == b'\\' {
                        i += 2;
                    } else if b[i] == b'"' {
                        break;
                    } else {
                        i += 1;
                    }
                }
            }
            b'(' => {
                if depth == 0 {
                    start = i;
                }
                depth += 1;
            }
            b')' => {
                anyhow::ensure!(depth > 0, "Unbalanced GIMP config");
                depth -= 1;
                if depth == 0 {
                    let key = text[start + 1..i].split_whitespace().next().unwrap_or("");
                    insert(&mut out, key, start, i + 1, text)?;
                }
            }
            c if depth == 0 && !c.is_ascii_whitespace() => {
                anyhow::bail!("Unsupported GIMP top-level syntax")
            }
            _ => {}
        }
        i += 1;
    }
    anyhow::ensure!(depth == 0, "Unclosed GIMP expression");
    Ok(out)
}
fn lines(rel: &str, text: &str) -> Result<BTreeMap<String, Span>> {
    let mut out = BTreeMap::new();
    let mut offset = 0;
    let mut color = false;
    let mut seen_color = false;
    let mut child_indent = None;
    for line in text.split_inclusive('\n') {
        let t = line.trim();
        let indent = line.len() - line.trim_start_matches(' ').len();
        if rel == LSD || rel == GLOW {
            anyhow::ensure!(
                !line
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .any(|c| c == '\t'),
                "Tabs in YAML indentation are unsupported"
            );
        }
        if !t.is_empty() && !t.starts_with('#') {
            if rel == MPV && t.starts_with('[') {
                break;
            }
            if rel == LSD {
                if indent == 0 {
                    color = false;
                    if let Some(tail) = t.strip_prefix("color:") {
                        anyhow::ensure!(!seen_color, "Duplicate YAML color mapping");
                        seen_color = true;
                        let tail = tail.trim();
                        anyhow::ensure!(
                            tail.is_empty() || tail.starts_with('#'),
                            "Use a block YAML color mapping"
                        );
                        color = true;
                    }
                } else if color {
                    anyhow::ensure!(
                        child_indent.is_none_or(|n| n == indent),
                        "Nested or inconsistent lsd color indentation is unsupported"
                    );
                    child_indent = Some(indent);
                    if let Some((key, _)) = t.split_once(':') {
                        if key.trim() == "theme" {
                            insert(&mut out, "theme", offset, offset + line.len(), text)?;
                        }
                    }
                }
            } else if let Some((key, _)) = t.split_once(if rel == MPV { '=' } else { ':' }) {
                let key = key.trim();
                if (rel == MPV || indent == 0) && allowed(rel, key) {
                    insert(&mut out, key, offset, offset + line.len(), text)?;
                }
            }
        }
        offset += line.len();
    }
    Ok(out)
}
fn spans(rel: &str, text: &str) -> Result<BTreeMap<String, Span>> {
    if rel == GIMP {
        sexpr(text)
    } else {
        lines(rel, text)
    }
}
fn value(raw: &str, separator: char) -> &str {
    raw.split_once(separator)
        .map(|(_, v)| v.trim())
        .unwrap_or("")
}
fn validate(rel: &str, key: &str, raw: &str) -> Result<()> {
    let v = if rel == GIMP {
        raw.trim()
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .and_then(|s| s.split_once(char::is_whitespace))
            .map(|(_, v)| v.trim())
            .unwrap_or("")
    } else {
        value(raw, if rel == MPV { '=' } else { ':' })
    };
    if rel == GIMP && key == "theme-color-scheme" {
        anyhow::ensure!(
            matches!(v, "dark" | "light" | "gray"),
            "Invalid GIMP scheme"
        );
    } else if rel == GIMP {
        let name: String = serde_json::from_str(v)?;
        anyhow::ensure!(
            !name.is_empty() && !name.contains(['/', '\\']) && !name.chars().any(char::is_control),
            "Invalid native GIMP theme name"
        );
    } else if rel == MPV {
        anyhow::ensure!(
            (v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')),
            "mpv #colours must be quoted; an unquoted # starts a comment"
        );
        let c = v.trim_matches(['\'', '"']);
        anyhow::ensure!(
            c.starts_with('#')
                && matches!(c.len(), 7 | 9)
                && c[1..].bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid mpv OSD colour"
        );
    } else {
        let name = v.trim_matches(['\'', '"']);
        anyhow::ensure!(
            !name.is_empty()
                && !name.chars().any(char::is_control)
                && !name.contains(['{', '}', '[', ']', '&', '*', '!']),
            "Unsupported native YAML appearance value"
        );
        if rel == GLOW {
            anyhow::ensure!(
                matches!(
                    name,
                    "auto"
                        | "dark"
                        | "light"
                        | "notty"
                        | "ascii"
                        | "pink"
                        | "dracula"
                        | "tokyo-night"
                        | "kolira"
                ) || name.ends_with("/glow/kolira.json"),
                "External Glow style paths are personal; select a builtin or the managed palette"
            );
        }
    }
    Ok(())
}
fn portable_glow(raw: &str) -> String {
    let value = raw
        .trim()
        .strip_prefix("style:")
        .unwrap_or("")
        .trim()
        .trim_matches(['\'', '"']);
    if value == "kolira" || value.ends_with("/glow/kolira.json") {
        format!(
            "style: {}\n",
            serde_json::to_string(
                &crate::paths::live_path(".config/glow/kolira.json").to_string_lossy()
            )
            .unwrap()
        )
    } else {
        raw.into()
    }
}
fn set(rel: &str, text: &str, key: &str, raw: Option<&str>) -> Result<String> {
    let map = spans(rel, text)?;
    let raw = raw.map(|s| {
        if rel == GLOW {
            portable_glow(s)
        } else {
            s.into()
        }
    });
    if let Some(old) = map.get(key) {
        let raw = if rel == LSD {
            raw.map(|s| {
                format!(
                    "{}{}",
                    " ".repeat(old.raw.len() - old.raw.trim_start_matches(' ').len()),
                    s.trim_start()
                )
            })
        } else {
            raw
        };
        return Ok(format!(
            "{}{}{}",
            &text[..old.range.start],
            raw.as_deref().unwrap_or(""),
            &text[old.range.end..]
        ));
    }
    let Some(raw) = raw else {
        return Ok(text.into());
    };
    if rel == LSD {
        let mut offset = 0;
        for line in text.split_inclusive('\n') {
            if line.starts_with("color:") {
                let at = offset + line.len();
                let indent = text[at..]
                    .lines()
                    .find(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
                    .filter(|l| l.starts_with(' '))
                    .map(|l| l.len() - l.trim_start_matches(' ').len())
                    .unwrap_or(2);
                return Ok(format!(
                    "{}{}{}{}",
                    &text[..at],
                    " ".repeat(indent),
                    raw.trim_start(),
                    &text[at..]
                ));
            }
            offset += line.len();
        }
        return Ok(format!(
            "{}{}color:\n  {}",
            text,
            if text.is_empty() || text.ends_with('\n') {
                ""
            } else {
                "\n"
            },
            raw.trim_start()
        ));
    }
    let at = if rel == MPV {
        text.split_inclusive('\n')
            .scan(0, |pos, line| {
                let old = *pos;
                *pos += line.len();
                Some((old, line))
            })
            .find(|(_, l)| l.trim_start().starts_with('['))
            .map(|(i, _)| i)
            .unwrap_or(text.len())
    } else {
        text.len()
    };
    Ok(format!(
        "{}{}{}{}",
        &text[..at],
        if at == 0 || text[..at].ends_with('\n') {
            ""
        } else {
            "\n"
        },
        raw,
        if raw.ends_with('\n') || rel == GIMP {
            String::from(&text[at..])
        } else {
            format!("\n{}", &text[at..])
        }
    ))
}
pub fn merge(rel: &str, theme: &str, live: &str) -> Result<String> {
    if rel == crate::third_apps::HEROIC {
        return crate::third_apps::heroic_merge(theme, live);
    }
    let source = spans(rel, theme)?;
    let _ = spans(rel, live)?;
    let mut out = live.to_string();
    for (key, v) in source.iter().filter(|(k, _)| allowed(rel, k)) {
        validate(rel, key, &v.raw)?;
        if rel == GIMP && key == "theme" {
            if let Some(paths) = sexpr(live)?.get("theme-path") {
                anyhow::ensure!(
                    paths.raw.contains("${gimp_dir}/themes"),
                    "Custom GIMP theme-path excludes native themes; refusing ineffective selection"
                );
            }
        }
        out = set(rel, &out, key, Some(&v.raw))?;
    }
    Ok(out)
}
pub fn capture(rel: &str, text: &str) -> Result<String> {
    if rel == crate::third_apps::HEROIC {
        return crate::third_apps::heroic_capture(text);
    }
    let map = spans(rel, text)?;
    let mut out = String::new();
    for (key, v) in map.iter().filter(|(k, _)| allowed(rel, k)) {
        if validate(rel, key, &v.raw).is_err() {
            continue;
        }
        let raw = if rel == GLOW
            && (v.raw.contains("/glow/kolira.json") || v.raw.contains("\"kolira\""))
        {
            "style: \"kolira\"\n"
        } else {
            &v.raw
        };
        out = set(rel, &out, key, Some(raw))?;
    }
    if rel == GLOW && out.contains("/glow/kolira.json") {
        Ok("style: \"kolira\"\n".into())
    } else {
        Ok(out)
    }
}
pub fn restore(rel: &str, before: &str, after: &str, current: &str) -> Result<String> {
    if rel == crate::third_apps::HEROIC {
        return crate::third_apps::heroic_restore(before, after, current);
    }
    let old = spans(rel, before)?;
    let written = spans(rel, after)?;
    let now = spans(rel, current)?;
    let mut out = current.to_string();
    for (key, v) in written.iter().filter(|(k, _)| allowed(rel, k)) {
        if now.get(key).is_some_and(|c| c.raw == v.raw) {
            out = set(rel, &out, key, old.get(key).map(|b| b.raw.as_str()))?;
        }
    }
    if rel == LSD && !before.lines().any(|s| s.starts_with("color:")) {
        let mut lines = out.lines().peekable();
        let mut clean = String::new();
        while let Some(line) = lines.next() {
            if line == "color:" && lines.peek().is_none_or(|s| !s.starts_with(' ')) {
                continue;
            }
            clean.push_str(line);
            clean.push('\n');
        }
        out = clean;
    }
    Ok(out)
}
