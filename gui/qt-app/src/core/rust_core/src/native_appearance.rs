//! Position-preserving selectors for native TOML, YAML and KDL appearance hosts.
//! Unsupported ambiguous host structures fail before any application writes.
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use toml_edit::{DocumentMut, Item};

pub fn handles(rel: &str) -> bool {
    if crate::structured_appearance::handles(rel) || rel == ".config/cliamp/config.toml" {
        return true;
    }
    matches!(
        rel,
        ".config/lazygit/config.yml"
            | ".config/zellij/config.kdl"
            | ".codex/config.toml"
            | ".config/yazi/theme.toml"
    )
}
const YAZI: &[&[&str]] = &[
    &["mgr", "cwd"],
    &["mgr", "hovered"],
    &["mgr", "preview_hovered"],
    &["mgr", "find_keyword"],
    &["mgr", "find_position"],
    &["mgr", "marker_copied"],
    &["mgr", "marker_cut"],
    &["mgr", "marker_marked"],
    &["mgr", "marker_selected"],
    &["mgr", "tab_active"],
    &["mgr", "tab_inactive"],
    &["mode", "normal_main"],
    &["mode", "normal_alt"],
    &["mode", "select_main"],
    &["mode", "select_alt"],
    &["mode", "unset_main"],
    &["mode", "unset_alt"],
    &["status", "overall"],
    &["status", "progress_normal"],
    &["status", "progress_error"],
    &["input", "border"],
    &["input", "title"],
    &["input", "value"],
    &["pick", "border"],
    &["pick", "active"],
    &["confirm", "border"],
    &["spot", "border"],
    &["help", "on"],
    &["help", "run"],
    &["help", "hovered"],
    &["notify", "title_info"],
];
fn keys(rel: &str) -> Vec<Vec<String>> {
    if rel == ".config/cliamp/config.toml" {
        vec![vec!["theme".into()]]
    } else if rel == ".codex/config.toml" {
        vec![vec!["tui".into(), "theme".into()]]
    } else {
        YAZI.iter()
            .map(|p| p.iter().map(|v| v.to_string()).collect())
            .collect()
    }
}
fn get<'a>(doc: &'a DocumentMut, path: &[String]) -> Option<&'a Item> {
    let mut item = doc.as_item();
    for part in path {
        item = item.get(part)?;
    }
    Some(item)
}
fn set(doc: &mut DocumentMut, path: &[String], value: Option<Item>) {
    let mut item = doc.as_item_mut();
    for part in &path[..path.len() - 1] {
        if item.get(part).is_none() {
            if value.is_none() {
                return;
            }
            item[part] = Item::Table(toml_edit::Table::new());
        }
        item = &mut item[part];
    }
    let key = &path[path.len() - 1];
    if let Some(mut value) = value {
        if let (Some(old), Some(new)) =
            (item.get(key).and_then(Item::as_value), value.as_value_mut())
        {
            *new.decor_mut() = old.decor().clone();
        }
        item[key] = value;
    } else if let Some(table) = item.as_table_like_mut() {
        table.remove(key);
    }
}
fn toml(rel: &str, theme: &str, live: &str) -> Result<String> {
    let source: DocumentMut = theme.parse().context("Invalid appearance TOML")?;
    let mut out: DocumentMut = live
        .parse()
        .context("Invalid personal TOML; refusing replacement")?;
    for key in keys(rel) {
        if let Some(value) = get(&source, &key) {
            let mut parent = out.as_item();
            for part in &key[..key.len() - 1] {
                if let Some(child) = parent.get(part) {
                    anyhow::ensure!(
                        child.is_table_like(),
                        "Appearance TOML parent must be a table: {part}"
                    );
                    parent = child;
                } else {
                    break;
                }
            }
            if rel == ".codex/config.toml" || rel == ".config/cliamp/config.toml" {
                anyhow::ensure!(value.as_str().is_some(), "Codex theme must be a string");
            } else {
                let style = value
                    .as_table_like()
                    .context("Yazi style must be a table")?;
                for (name, field) in style.iter() {
                    let valid = match name {
                        "fg" | "bg" => {
                            field.as_str().is_some()
                                || field.as_integer().is_some_and(|n| (0..=255).contains(&n))
                        }
                        "bold" | "dim" | "italic" | "underline" | "blink" | "reverse"
                        | "hidden" | "crossed" | "reset" => field.as_bool().is_some(),
                        _ => false,
                    };
                    anyhow::ensure!(valid, "Unsupported Yazi appearance style: {name}");
                }
            }
            set(&mut out, &key, Some(value.clone()));
        }
    }
    Ok(out.to_string())
}

pub const YAML_KEYS: &[&str] = &[
    "activeBorderColor",
    "inactiveBorderColor",
    "searchingActiveBorderColor",
    "optionsTextColor",
    "selectedLineFgColor",
    "selectedLineBgColor",
    "inactiveViewSelectedLineBgColor",
    "cherryPickedCommitFgColor",
    "cherryPickedCommitBgColor",
    "markedBaseCommitFgColor",
    "markedBaseCommitBgColor",
    "unstagedChangesColor",
    "defaultFgColor",
];
#[derive(Clone)]
struct Block {
    start: usize,
    end: usize,
    value: String,
}
struct Yaml {
    gui: Option<(usize, usize)>,
    theme: Option<(usize, usize)>,
    entries: BTreeMap<String, Block>,
    child_indent: usize,
    member_indent: usize,
}
fn yaml(text: &str) -> Result<Yaml> {
    let lines: Vec<_> = text.split_inclusive('\n').collect();
    let mut offsets = vec![0];
    for line in &lines {
        offsets.push(offsets.last().unwrap() + line.len());
    }
    let meaningful = |line: &str| !line.trim().is_empty() && !line.trim_start().starts_with('#');
    let indent = |line: &str| line.len() - line.trim_start_matches(' ').len();
    for line in &lines {
        anyhow::ensure!(
            !line
                .chars()
                .take_while(|c| c.is_whitespace())
                .any(|c| c == '\t'),
            "Tabs in YAML indentation are unsupported"
        );
    }
    let root: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| meaningful(l) && indent(l) == 0)
        .collect();
    for (_, line) in &root {
        anyhow::ensure!(
            line.trim() == "---" || line.contains(':'),
            "Invalid YAML root mapping"
        );
    }
    let matches: Vec<_> = root
        .iter()
        .filter(|(_, l)| {
            l.split_once(':')
                .is_some_and(|(k, _)| k.trim_matches(['\'', '"']) == "gui")
        })
        .collect();
    anyhow::ensure!(matches.len() <= 1, "Duplicate YAML gui key");
    let Some((gui, line)) = matches.first().copied() else {
        return Ok(Yaml {
            gui: None,
            theme: None,
            entries: BTreeMap::new(),
            child_indent: 2,
            member_indent: 4,
        });
    };
    let tail = line[4..].trim();
    anyhow::ensure!(
        line.starts_with("gui:") && (tail.is_empty() || tail.starts_with('#')),
        "Use a block YAML gui mapping"
    );
    let gui = *gui;
    let gui_end = root
        .iter()
        .find(|(i, _)| *i > gui)
        .map(|(i, _)| *i)
        .unwrap_or(lines.len());
    let child_indent = (gui + 1..gui_end)
        .filter(|i| meaningful(lines[*i]))
        .map(|i| indent(lines[i]))
        .min()
        .unwrap_or(2);
    let mut found = None;
    for (i, line) in lines.iter().enumerate().take(gui_end).skip(gui + 1) {
        let line = *line;
        let t = line.trim_start();
        if meaningful(line)
            && indent(line) == child_indent
            && t.split_once(':')
                .is_some_and(|(k, _)| k.trim_matches(['\'', '"']) == "theme")
        {
            anyhow::ensure!(
                found.is_none() && t.starts_with("theme:"),
                "Ambiguous YAML gui.theme"
            );
            let value = t[6..].trim();
            anyhow::ensure!(
                value.is_empty() || value.starts_with('#'),
                "Use a block YAML gui.theme mapping"
            );
            found = Some(i);
        }
    }
    let mut out = Yaml {
        gui: Some((offsets[gui], offsets[gui_end])),
        theme: None,
        entries: BTreeMap::new(),
        child_indent,
        member_indent: child_indent * 2,
    };
    if let Some(theme) = found {
        let level = indent(lines[theme]);
        let end = (theme + 1..gui_end)
            .find(|i| meaningful(lines[*i]) && indent(lines[*i]) <= level)
            .unwrap_or(gui_end);
        out.theme = Some((offsets[theme], offsets[end]));
        out.member_indent = (theme + 1..end)
            .filter(|i| meaningful(lines[*i]))
            .map(|i| indent(lines[i]))
            .min()
            .unwrap_or(level + child_indent);

        let mut headers = Vec::new();
        for (i, line) in lines.iter().enumerate().take(end).skip(theme + 1) {
            if !meaningful(line) {
                continue;
            }
            if indent(lines[i]) == out.member_indent {
                let (key, _) = lines[i]
                    .trim()
                    .split_once(':')
                    .context("Invalid YAML theme member")?;
                anyhow::ensure!(
                    !headers.iter().any(|(_, k)| k == key),
                    "Duplicate YAML theme colour"
                );
                headers.push((i, key.to_string()));
            }
        }
        for (n, (i, key)) in headers.iter().enumerate() {
            let mut stop = headers.get(n + 1).map(|(j, _)| *j).unwrap_or(end);
            while stop > i + 1 && !meaningful(lines[stop - 1]) {
                stop -= 1;
            }
            out.entries.insert(
                key.clone(),
                Block {
                    start: offsets[*i],
                    end: offsets[stop],
                    value: lines[*i..stop].concat(),
                },
            );
        }
    }
    Ok(out)
}
fn yaml_set(text: &str, key: &str, value: Option<&str>) -> Result<String> {
    let doc = yaml(text)?;
    let normalized = value.map(|value| {
        let base = value
            .lines()
            .find(|l| !l.trim().is_empty())
            .map(|l| l.len() - l.trim_start_matches(' ').len())
            .unwrap_or(0);
        value
            .split_inclusive('\n')
            .map(|line| {
                if line.trim().is_empty() {
                    line.to_string()
                } else {
                    format!(
                        "{}{}",
                        " ".repeat(doc.member_indent),
                        line.strip_prefix(&" ".repeat(base)).unwrap_or(line)
                    )
                }
            })
            .collect::<String>()
    });
    let value = normalized.as_deref();
    if let Some(old) = doc.entries.get(key) {
        return Ok(format!(
            "{}{}{}",
            &text[..old.start],
            value.unwrap_or(""),
            &text[old.end..]
        ));
    }
    let Some(value) = value else {
        return Ok(text.into());
    };
    let (at, prefix) = if let Some((_, end)) = doc.theme {
        (end, String::new())
    } else if let Some((_, end)) = doc.gui {
        (end, format!("{}theme:\n", " ".repeat(doc.child_indent)))
    } else {
        (text.len(), "gui:\n  theme:\n".into())
    };
    let sep = if at > 0 && !text[..at].ends_with('\n') {
        "\n"
    } else {
        ""
    };
    Ok(format!(
        "{}{sep}{prefix}{value}{}",
        &text[..at],
        &text[at..]
    ))
}
fn yaml_colour_list(block: &str) -> Result<()> {
    let (_, tail) = block.split_once(':').context("Invalid YAML colour entry")?;
    let mut values = Vec::new();
    let first = tail.lines().next().unwrap_or("").trim();
    if first.starts_with('[') {
        let end = first.rfind(']').context("Unclosed YAML colour list")?;
        anyhow::ensure!(
            first[end + 1..].trim().is_empty() || first[end + 1..].trim().starts_with('#'),
            "Invalid YAML colour list tail"
        );
        values.extend(first[1..end].split(',').map(str::trim));
    } else {
        anyhow::ensure!(
            first.is_empty() || first.starts_with('#'),
            "YAML colours must be a list"
        );
        for line in tail
            .lines()
            .skip(1)
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.starts_with('#'))
        {
            values.push(
                line.strip_prefix("- ")
                    .context("YAML colours must be string list entries")?
                    .trim(),
            );
        }
    }
    anyhow::ensure!(!values.is_empty(), "Empty YAML colour list");
    for value in values {
        let color = value.trim_matches(['\'', '"']);
        anyhow::ensure!(
            !color.is_empty()
                && color
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '#' | '_' | '-')),
            "Unsupported YAML colour string"
        );
        if let Some(hex) = color.strip_prefix('#') {
            anyhow::ensure!(
                hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()),
                "Invalid YAML hex colour"
            );
        }
    }
    Ok(())
}
fn yaml_merge(theme: &str, live: &str) -> Result<String> {
    let src = yaml(theme)?;
    let _ = yaml(live)?;
    let mut out = live.to_string();
    for key in YAML_KEYS {
        if let Some(value) = src.entries.get(*key) {
            yaml_colour_list(&value.value)?;
            out = yaml_set(&out, key, Some(&value.value))?;
        }
    }
    Ok(out)
}

// Only root selection nodes are edited; raw strings/comments/nested keybinds
// remain opaque. A custom selection syntax is refused, never regex-rewritten.
const KDL_KEYS: &[&str] = &["theme", "theme_dark", "theme_light"];
const KDL_PARSE_KEYS: &[&str] = &["theme", "theme_dark", "theme_light", "theme_dir"];
fn kdl(text: &str) -> Result<BTreeMap<String, Block>> {
    let b = text.as_bytes();
    let mut i = 0;
    let mut depth = 0usize;
    let mut start = 0;
    let mut nodes = BTreeMap::new();
    while i < b.len() {
        if b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b[i..].starts_with(b"/*") {
            let leading = depth == 0 && text[start..i].trim().is_empty();
            let mut n = 1;
            i += 2;
            while i < b.len() && n > 0 {
                if b[i..].starts_with(b"/*") {
                    n += 1;
                    i += 2;
                } else if b[i..].starts_with(b"*/") {
                    n -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            anyhow::ensure!(n == 0, "Unclosed KDL comment");
            if leading {
                start = i;
            }
            continue;
        }
        if b[i] == b'r' {
            let mut quote = i + 1;
            while b.get(quote) == Some(&b'#') {
                quote += 1;
            }
            if b.get(quote) == Some(&b'"') {
                let hashes = quote - i - 1;
                let closing = format!("\"{}", "#".repeat(hashes));
                let offset = text[quote + 1..]
                    .find(&closing)
                    .context("Unclosed KDL raw string")?;
                i = quote + 1 + offset + closing.len();
                continue;
            }
        }
        if b[i] == b'"' {
            i += 1;
            let mut closed = false;
            while i < b.len() {
                if b[i] == b'\\' {
                    i += 2;
                } else if b[i] == b'"' {
                    i += 1;
                    closed = true;
                    break;
                } else {
                    i += 1;
                }
            }
            anyhow::ensure!(closed, "Unclosed KDL string");
            continue;
        }
        if b[i] == b'{' {
            depth += 1;
        } else if b[i] == b'}' {
            depth = depth.checked_sub(1).context("Unbalanced KDL")?;
        }
        if depth == 0 && matches!(b[i], b'\n' | b';' | b'}') {
            let raw = &text[start..i + 1];
            let t = raw.trim_start();
            for key in KDL_PARSE_KEYS {
                if t.strip_prefix(key)
                    .is_some_and(|tail| tail.starts_with(char::is_whitespace))
                {
                    anyhow::ensure!(!nodes.contains_key(*key), "Duplicate KDL theme selector");
                    let without = t
                        .split("//")
                        .next()
                        .unwrap()
                        .trim()
                        .trim_end_matches(';')
                        .trim();
                    let v = without[key.len()..].trim();
                    let value: String = serde_json::from_str(v)
                        .context("KDL theme selector must be one quoted string")?;
                    nodes.insert(
                        (*key).into(),
                        Block {
                            start,
                            end: i + 1,
                            value,
                        },
                    );
                }
            }
            start = i + 1;
        }
        i += 1;
    }
    anyhow::ensure!(depth == 0, "Unclosed KDL block");
    if start < text.len() {
        let tail = format!("{}\n", &text[start..]);
        for (key, mut block) in kdl(&tail)? {
            block.start += start;
            block.end = text.len();
            anyhow::ensure!(!nodes.contains_key(&key), "Duplicate KDL theme selector");
            nodes.insert(key, block);
        }
    }
    Ok(nodes)
}
fn kdl_set(text: &str, key: &str, value: Option<&str>) -> Result<String> {
    let doc = kdl(text)?;
    if let Some(old) = doc.get(key) {
        let raw = &text[old.start..old.end];
        let note = raw.find("//").map(|i| raw[i..].trim_end()).unwrap_or("");
        let replacement = value
            .map(|v| {
                format!(
                    "{key} {}{}\n",
                    serde_json::to_string(v).unwrap(),
                    if note.is_empty() {
                        String::new()
                    } else {
                        format!(" {note}")
                    }
                )
            })
            .unwrap_or_else(|| {
                if note.is_empty() {
                    String::new()
                } else {
                    format!("{note}\n")
                }
            });
        return Ok(format!(
            "{}{}{}",
            &text[..old.start],
            replacement,
            &text[old.end..]
        ));
    }
    Ok(match value {
        Some(v) => format!(
            "{}{key} {}\n",
            if text.is_empty() || text.ends_with('\n') {
                text.into()
            } else {
                format!("{text}\n")
            },
            serde_json::to_string(v)?
        ),
        None => text.into(),
    })
}

pub fn merge(rel: &str, theme: &str, live: &str) -> Result<String> {
    if crate::structured_appearance::handles(rel) {
        return crate::structured_appearance::merge(rel, theme, live);
    }
    match rel {
        ".config/lazygit/config.yml" => yaml_merge(theme, live),
        ".config/zellij/config.kdl" => {
            let src = kdl(theme)?;
            let live_doc = kdl(live)?;
            if let Some(dir) = live_doc.get("theme_dir") {
                anyhow::ensure!(std::path::Path::new(&dir.value)==crate::paths::config_home().join("zellij/themes"),"A custom Zellij theme_dir needs explicit native integration; refusing an ineffective theme selector");
            }
            let mut out = live.to_string();
            for key in KDL_KEYS {
                if let Some(v) = src.get(*key) {
                    out = kdl_set(&out, key, Some(&v.value))?;
                }
            }
            Ok(out)
        }
        _ => toml(rel, theme, live),
    }
}
pub fn capture(rel: &str, text: &str) -> Result<String> {
    if crate::structured_appearance::handles(rel) {
        return crate::structured_appearance::capture(rel, text);
    }
    merge(rel, text, "")
}
pub fn restore(rel: &str, before: &str, after: &str, current: &str) -> Result<String> {
    if crate::structured_appearance::handles(rel) {
        return crate::structured_appearance::restore(rel, before, after, current);
    }
    if rel == ".config/lazygit/config.yml" {
        let a = yaml(before)?;
        let b = yaml(after)?;
        let c = yaml(current)?;
        let mut out = current.to_string();
        for key in YAML_KEYS {
            let old = a.entries.get(*key);
            let written = b.entries.get(*key);
            let live = c.entries.get(*key);
            if live.map(|v| v.value.trim()) == written.map(|v| v.value.trim()) {
                out = yaml_set(&out, key, old.map(|v| v.value.as_str()))?;
            }
        }
        if a.theme.is_none() {
            let parsed = yaml(&out)?;
            if parsed.entries.is_empty() {
                if let Some((start, _)) = parsed.theme {
                    let end = out[start..]
                        .find('\n')
                        .map(|n| start + n + 1)
                        .unwrap_or(out.len());
                    out.replace_range(start..end, "");
                }
            }
        }
        return Ok(out);
    }
    if rel == ".config/zellij/config.kdl" {
        let a = kdl(before)?;
        let b = kdl(after)?;
        let c = kdl(current)?;
        let mut out = current.to_string();
        for key in KDL_KEYS {
            if c.get(*key).map(|v| &v.value) == b.get(*key).map(|v| &v.value) {
                out = kdl_set(&out, key, a.get(*key).map(|v| v.value.as_str()))?;
            }
        }
        return Ok(out);
    }
    let a: DocumentMut = before.parse()?;
    let b: DocumentMut = after.parse()?;
    let c: DocumentMut = current.parse()?;
    let mut out: DocumentMut = current.parse()?;
    for key in keys(rel) {
        let old = get(&a, &key);
        let written = get(&b, &key);
        let live = get(&c, &key);
        if live.map(Item::to_string) == written.map(Item::to_string) {
            set(&mut out, &key, old.cloned());
        }
    }
    for key in keys(rel) {
        for n in (1..key.len()).rev() {
            let parent = &key[..n];
            if get(&a, parent).is_none()
                && get(&out, parent)
                    .and_then(Item::as_table_like)
                    .is_some_and(|t| t.is_empty())
            {
                set(&mut out, parent, None);
            }
        }
    }
    Ok(out.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn yaml_four_space_indentation_and_late_non_theme_edits_survive() {
        let before="# personal\ngui:\n    mouseEvents: false\n    theme:\n        activeBorderColor: [red]\n# git note\ngit:\n    autoFetch: false\n";
        let theme="gui:\n  theme:\n    activeBorderColor: [\"#aabbcc\"]\n    selectedLineBgColor: [\"#202020\"]\n";
        let after = merge(".config/lazygit/config.yml", theme, before).unwrap();
        assert!(after.contains("        activeBorderColor: [\"#aabbcc\"]"));
        assert!(after.contains("# git note"));
        assert_eq!(
            merge(".config/lazygit/config.yml", theme, &after).unwrap(),
            after
        );
        let current = after.replace("mouseEvents: false", "mouseEvents: true");
        let restored = restore(".config/lazygit/config.yml", before, &after, &current).unwrap();
        assert!(restored.contains("mouseEvents: true"));
        assert!(restored.contains("activeBorderColor: [red]"));
        assert!(!restored.contains("selectedLineBgColor"));
    }
    #[test]
    fn kdl_preserves_raw_nested_strings_and_leading_comments() {
        let before="/* personal */ theme \"mine\" // colour\nkeybinds { normal { bind \"Alt x\" { Run r#\"echo {\\\"hello\\\"}\"#; }; }; }\n";
        let after = merge(".config/zellij/config.kdl", "theme \"kolira\"\n", before).unwrap();
        assert!(after.starts_with("/* personal */"));
        assert!(after.contains("Run r#\"echo"));
        assert!(after.contains("// colour"));
        let restored = restore(
            ".config/zellij/config.kdl",
            before,
            &after,
            &after.replace("Alt x", "Alt y"),
        )
        .unwrap();
        assert!(restored.contains("theme \"mine\""));
        assert!(restored.contains("Alt y"));
    }
    #[test]
    fn toml_removes_only_new_empty_tables_and_keeps_user_comments() {
        let before = "# model\nmodel = 'first'\n";
        let after = merge(".codex/config.toml", "[tui]\ntheme = 'kolira'\n", before).unwrap();
        let current = after.replace("first", "later");
        let restored = restore(".codex/config.toml", before, &after, &current).unwrap();
        assert!(restored.contains("# model"));
        assert!(restored.contains("'later'"));
        assert!(!restored.contains("[tui]"));
    }
}
