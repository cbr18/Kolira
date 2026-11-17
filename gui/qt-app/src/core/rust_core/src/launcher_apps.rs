//! Colour-only launcher resources. Executable/personal hosts never leave HOME.
//! Rofi uses inline overrides (no layout-resetting @theme or dangling include).
//! Walker patches an existing selected modern theme; unsupported/default embedded
//! styles are left untouched, rather than replacing the client's built-in layout.
use crate::{
    app_sync,
    change_plan::Plan,
    palette::{kde_roles, Palette},
    paths,
    theme::Package,
};
use anyhow::{Context, Result};
use std::{collections::BTreeMap, path::Path};

pub const TARGETS: &[&str] = &["rofi", "fuzzel", "walker"];
pub const ROFI_HOST: &str = ".config/rofi/config.rasi";
pub const ROFI_THEME: &str = ".config/rofi/themes/kolira.rasi";
pub const FUZZEL: &str = ".config/fuzzel/fuzzel.ini";
pub const WALKER: &str = ".config/walker/kolira.css";
pub const ROFI_BEGIN: &str = "/* BEGIN kolira colours */";
pub const ROFI_END: &str = "/* END kolira colours */";
pub const FUZZEL_KEYS: &[&str] = &[
    "background",
    "text",
    "prompt",
    "placeholder",
    "input",
    "match",
    "selection",
    "selection-text",
    "selection-match",
    "border",
    "counter",
];
pub const FUZZEL_RULES: &[crate::appearance::Rule] = &[crate::appearance::Rule {
    group: "colors",
    keys: crate::appearance::Keys::Only(FUZZEL_KEYS),
}];

pub fn paths_for(id: &str) -> Vec<String> {
    match id {
        "rofi" => vec![ROFI_HOST.into(), ROFI_THEME.into()],
        "fuzzel" => vec![FUZZEL.into()],
        "walker" => vec![WALKER.into(), ".config/walker/config.toml".into()],
        _ => vec![],
    }
}
pub fn is_host(rel: &str) -> bool {
    rel == ROFI_HOST || (rel.starts_with(".config/walker/themes/") && rel.ends_with("/style.css"))
}
pub fn preserved(rel: &str) -> bool {
    rel == ROFI_HOST || (Path::new(rel).starts_with(".config/walker") && rel != WALKER)
}
pub fn is_asset(rel: &str) -> bool {
    matches!(rel, ROFI_THEME | WALKER)
}

fn declarations(rel: &str) -> &'static [(&'static str, &'static str)] {
    if rel == ROFI_THEME {
        &[
            ("window", "background-color"),
            ("window", "text-color"),
            ("window", "border-color"),
            ("mainbox", "background-color"),
            ("inputbar", "background-color"),
            ("entry", "text-color"),
            ("entry", "cursor-color"),
            ("prompt", "text-color"),
            ("listview", "background-color"),
            ("element", "background-color"),
            ("element", "text-color"),
            ("element selected", "background-color"),
            ("element selected", "text-color"),
            ("element-text", "background-color"),
            ("element-text", "text-color"),
            ("element-icon", "background-color"),
        ]
    } else {
        &[
            (".box-wrapper", "background-color"),
            (".box-wrapper", "color"),
            (".box-wrapper", "border-color"),
            (".input", "background-color"),
            (".input", "color"),
            (".input", "caret-color"),
            (".list", "color"),
            ("child:selected .item-box", "background-color"),
            ("row:selected .item-box", "background-color"),
            ("child:selected .item-box", "color"),
            ("row:selected .item-box", "color"),
        ]
    }
}
fn rgb(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|b| b.is_ascii_hexdigit())
}
/// A tiny data grammar, not a CSS/Rasi interpreter: known selectors/properties,
/// literal RGB (plus fixed transparent/inherit), no imports, functions or layout.
pub fn valid_asset(rel: &str, text: &str) -> Result<String> {
    anyhow::ensure!(is_asset(rel), "Unknown launcher colour resource");
    let mut clean = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("/*") {
        clean.push_str(&rest[..start]);
        let end = rest[start + 2..]
            .find("*/")
            .context("Unclosed launcher comment")?
            + start
            + 4;
        rest = &rest[end..];
    }
    clean.push_str(rest);
    let mut values = BTreeMap::new();
    let mut rest = clean.trim();
    while !rest.is_empty() {
        let (selector, body) = rest
            .split_once('{')
            .context("Invalid launcher colour block")?;
        let (body, tail) = body
            .split_once('}')
            .context("Unclosed launcher colour block")?;
        let selector = selector.trim();
        for field in body.split(';').filter(|s| !s.trim().is_empty()) {
            let (key, value) = field
                .split_once(':')
                .context("Invalid launcher colour declaration")?;
            let (key, value) = (key.trim(), value.trim());
            anyhow::ensure!(
                declarations(rel).contains(&(selector, key))
                    || (rel == ROFI_THEME
                        && matches!(key, "background-color" | "text-color")
                        && selector.strip_prefix("element ").is_some_and(|s| [
                            "normal.normal",
                            "normal.active",
                            "normal.urgent",
                            "alternate.normal",
                            "alternate.active",
                            "alternate.urgent",
                            "selected.normal",
                            "selected.active",
                            "selected.urgent"
                        ]
                        .contains(&s))),
                "Non-colour launcher declaration: {selector} / {key}"
            );
            anyhow::ensure!(
                rgb(value) || (rel == ROFI_THEME && matches!(value, "transparent" | "inherit")),
                "Invalid launcher RGB: {value}"
            );
            anyhow::ensure!(
                values
                    .insert((selector.to_string(), key.to_string()), value.to_string())
                    .is_none(),
                "Duplicate launcher colour"
            );
        }
        rest = tail.trim();
    }
    anyhow::ensure!(!values.is_empty(), "Empty launcher colour resource");
    Ok(values
        .into_iter()
        .map(|((s, k), v)| format!("{s} {{ {k}: {v}; }}\n"))
        .collect())
}
pub fn fuzzel_source(text: &str) -> Result<String> {
    let filtered = crate::appearance::ini_filter(text, FUZZEL_RULES);
    for line in filtered.lines() {
        if let Some((_, value)) = line.split_once('=') {
            let value = value.trim();
            anyhow::ensure!(
                value.len() == 8 && value.bytes().all(|b| b.is_ascii_hexdigit()),
                "Fuzzel colours must be RRGGBBAA literals"
            );
        }
    }
    Ok(filtered)
}
/// RGB follows the theme; alpha remains the user's opacity choice.
pub fn fuzzel_merge(source: &str, live: &str) -> Result<String> {
    let mut source = fuzzel_source(source)?;
    for key in FUZZEL_KEYS {
        let old = crate::field::ini_get(live, "colors", key);
        let new = crate::field::ini_get(&source, "colors", key);
        if let (Some(old), Some(new)) = (old, new) {
            if old.len() == 8 && old.bytes().all(|b| b.is_ascii_hexdigit()) {
                source = crate::field::set_ini_text(
                    &source,
                    "colors",
                    key,
                    &format!("{}{}", &new[..6], &old[6..]),
                );
            }
        }
    }
    Ok(crate::appearance::ini_merge(&source, live, FUZZEL_RULES))
}
pub fn restore_fuzzel(before: &str, after: &str, current: &str) -> String {
    let mut normalized = current.to_string();
    let mut alpha = Vec::new();
    for key in FUZZEL_KEYS {
        let (Some(old), Some(written), Some(live)) = (
            crate::field::ini_get(before, "colors", key),
            crate::field::ini_get(after, "colors", key),
            crate::field::ini_get(current, "colors", key),
        ) else {
            continue;
        };
        if [&old, &written, &live]
            .iter()
            .all(|s| s.len() == 8 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            && live[..6] == written[..6]
            && live[6..] != written[6..]
        {
            normalized = crate::field::set_ini_text(&normalized, "colors", key, &written);
            alpha.push((*key, live[6..].to_string()));
        }
    }
    let mut out = crate::appearance::restore_ini(before, after, &normalized);
    for (key, alpha) in alpha {
        if let Some(value) = crate::field::ini_get(&out, "colors", key) {
            out = crate::field::set_ini_text(
                &out,
                "colors",
                key,
                &format!("{}{}", &value[..6], alpha),
            );
        }
    }
    out
}
pub fn bundle_resources(p: &Palette) -> BTreeMap<String, String> {
    let r = kde_roles(p);
    let (bg, fg, alt, accent, selected, border) = (
        r.window.hex(),
        r.text.hex(),
        r.window_alt.hex(),
        r.accent.hex(),
        r.selection_text.hex(),
        p.slot(2).hex(),
    );
    let mut rofi = format!("window {{ background-color: {bg}; text-color: {fg}; border-color: {border}; }}\nmainbox {{ background-color: transparent; }}\ninputbar {{ background-color: {alt}; }}\nentry {{ text-color: {fg}; cursor-color: {accent}; }}\nprompt {{ text-color: {accent}; }}\nlistview {{ background-color: transparent; }}\nelement {{ background-color: transparent; text-color: {fg}; }}\nelement selected {{ background-color: {accent}; text-color: {selected}; }}\nelement-text {{ background-color: transparent; text-color: inherit; }}\nelement-icon {{ background-color: transparent; }}\n");
    for state in [
        "normal.normal",
        "normal.active",
        "normal.urgent",
        "alternate.normal",
        "alternate.active",
        "alternate.urgent",
        "selected.normal",
        "selected.active",
        "selected.urgent",
    ] {
        let selected_state = state.starts_with("selected");
        let (background, foreground) = if selected_state {
            (accent.as_str(), selected.as_str())
        } else {
            (bg.as_str(), fg.as_str())
        };
        rofi.push_str(&format!(
            "element {state} {{ background-color: {background}; text-color: {foreground}; }}\n"
        ));
    }
    let css=format!(".box-wrapper {{ background-color: {bg}; color: {fg}; border-color: {border}; }}\n.input {{ background-color: {alt}; color: {fg}; caret-color: {accent}; }}\n.list {{ color: {fg}; }}\nchild:selected .item-box {{ background-color: {accent}; color: {selected}; }}\nrow:selected .item-box {{ background-color: {accent}; color: {selected}; }}\n");
    let colors = [
        ("background", &bg),
        ("text", &fg),
        ("prompt", &accent),
        ("placeholder", &fg),
        ("input", &fg),
        ("match", &accent),
        ("selection", &accent),
        ("selection-text", &selected),
        ("selection-match", &selected),
        ("border", &border),
        ("counter", &fg),
    ];
    let ini = format!(
        "[colors]\n{}",
        colors
            .into_iter()
            .map(|(key, value)| format!("{key}={}ff\n", &value[1..]))
            .collect::<String>()
    );
    [
        (
            ROFI_THEME.into(),
            valid_asset(ROFI_THEME, &rofi).expect("fixed colour grammar"),
        ),
        (FUZZEL.into(), ini),
        (
            WALKER.into(),
            valid_asset(WALKER, &css).expect("fixed colour grammar"),
        ),
    ]
    .into_iter()
    .collect()
}
/// Complete, unique markers are required; never truncate malformed personal text.
fn block(text: &str) -> Result<Option<(usize, usize, &str)>> {
    let starts: Vec<_> = text.match_indices(ROFI_BEGIN).collect();
    let ends: Vec<_> = text.match_indices(ROFI_END).collect();
    if starts.is_empty() && ends.is_empty() {
        return Ok(None);
    }
    anyhow::ensure!(
        starts.len() == 1 && ends.len() == 1,
        "Ambiguous launcher colour markers"
    );
    let (start, end) = (starts[0].0, ends[0].0);
    anyhow::ensure!(
        start < end && (start == 0 || text.as_bytes()[start - 1] == b'\n'),
        "Malformed launcher colour markers"
    );
    let end_after = end + ROFI_END.len();
    anyhow::ensure!(
        text[end_after..].is_empty() || text.as_bytes()[end_after] == b'\n',
        "Malformed launcher colour marker end"
    );
    let end_after = end_after + usize::from(text.as_bytes().get(end_after) == Some(&b'\n'));
    Ok(Some((
        start,
        end_after,
        &text[start + ROFI_BEGIN.len()..end],
    )))
}
fn overlay(rel: &str, old: &str, colours: &str) -> Result<String> {
    let colours = valid_asset(rel, colours)?;
    if let Some((start, end, body)) = block(old)? {
        // A personal edit inside the block is not silently deleted on reapply.
        valid_asset(rel, body)
            .context("Personally modified launcher block requires restoration before applying")?;
        let mut out = old.to_string();
        out.replace_range(start..end, &format!("{ROFI_BEGIN}\n{colours}{ROFI_END}\n"));
        return Ok(out);
    }
    Ok(format!("{old}\n{ROFI_BEGIN}\n{colours}{ROFI_END}\n"))
}
// Mask comments and strings at their original byte positions. This lets return
// patch only literal colour declarations, preserving comments and private CSS.
fn css_mask(text: &str) -> Option<(String, Vec<std::ops::Range<usize>>)> {
    let mut bytes = text.as_bytes().to_vec();
    let mut comments = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"/*") {
            let start = i;
            let end = text[i + 2..].find("*/")? + i + 4;
            for b in &mut bytes[start..end] {
                if *b != b'\n' {
                    *b = b' ';
                }
            }
            comments.push(start..end);
            i = end;
        } else if matches!(bytes[i], b'\"' | b'\'') {
            let quote = bytes[i];
            let start = i;
            i += 1;
            let mut closed = false;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i += 2;
                } else if bytes[i] == quote {
                    i += 1;
                    closed = true;
                    break;
                } else {
                    i += 1;
                }
            }
            if !closed {
                return None;
            }
            bytes[start..i].fill(b'_');
        } else {
            i += 1;
        }
    }
    Some((String::from_utf8(bytes).ok()?, comments))
}
#[derive(Clone)]
struct ColourField {
    key: (String, String),
    value: String,
    declaration: std::ops::Range<usize>,
    value_span: std::ops::Range<usize>,
}
struct ColourRule {
    range: std::ops::Range<usize>,
    fields: Vec<ColourField>,
}
fn colour_rules(mask: &str) -> Option<Vec<ColourRule>> {
    let mut cursor = 0;
    let mut rules = Vec::new();
    while !mask[cursor..].trim().is_empty() {
        let start = cursor + mask[cursor..].len() - mask[cursor..].trim_start().len();
        let open = mask[start..].find('{')? + start;
        let close = mask[open + 1..].find('}')? + open + 1;
        if mask[open + 1..close].contains('{') {
            return None;
        }
        let selector = mask[start..open]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let mut fields = Vec::new();
        let mut pos = open + 1;
        for part in mask[open + 1..close].split_inclusive(';') {
            let trimmed = part.trim().trim_end_matches(';').trim_end();
            if let Some((key, value)) = trimmed.split_once(':') {
                let value = value.trim();
                if rgb(value) || matches!(value, "transparent" | "inherit") {
                    let begin = pos + part.len() - part.trim_start().len();
                    let value_start = begin + trimmed.find(':')? + 1;
                    let value_start = value_start + mask[value_start..pos + part.len()].len()
                        - mask[value_start..pos + part.len()].trim_start().len();
                    fields.push(ColourField {
                        key: (selector.clone(), key.trim().into()),
                        value: value.to_ascii_lowercase(),
                        declaration: begin..pos + part.len(),
                        value_span: value_start..value_start + value.len(),
                    });
                }
            }
            pos += part.len();
        }
        rules.push(ColourRule {
            range: start..close + 1,
            fields,
        });
        cursor = close + 1;
    }
    Some(rules)
}
fn return_css(original: &str, written: &str, live: &str) -> Option<String> {
    let map = |text: &str| -> Option<BTreeMap<(String, String), String>> {
        let (mask, _) = css_mask(text)?;
        Some(
            colour_rules(&mask)?
                .into_iter()
                .flat_map(|r| r.fields.into_iter().map(|f| (f.key, f.value)))
                .collect(),
        )
    };
    let (old, after) = (map(original)?, map(written)?);
    let (mask, _) = css_mask(live)?;
    let rules = colour_rules(&mask)?;
    let mut out = live.to_string();
    for rule in rules.into_iter().rev() {
        let mut body = live[rule.range.clone()].to_string();
        for field in rule.fields.iter().rev() {
            if after.get(&field.key) != Some(&field.value) {
                continue;
            }
            let (range, replacement) = if let Some(value) = old.get(&field.key) {
                (field.value_span.clone(), value.clone())
            } else {
                let raw = &live[field.declaration.clone()];
                let (_, comments) = css_mask(raw)?;
                (
                    field.declaration.clone(),
                    comments
                        .into_iter()
                        .map(|r| format!("{}\n", &raw[r]))
                        .collect(),
                )
            };
            body.replace_range(
                range.start - rule.range.start..range.end - rule.range.start,
                &replacement,
            );
        }
        let (masked, comments) = css_mask(&body)?;
        let open = masked.find('{')?;
        let close = masked.rfind('}')?;
        if masked[open + 1..close].trim().is_empty() {
            body = comments
                .into_iter()
                .map(|r| format!("{}\n", &body[r]))
                .collect();
        }
        out.replace_range(rule.range, &body);
    }
    let current_keys: std::collections::BTreeSet<_> = colour_rules(&mask)?
        .into_iter()
        .flat_map(|r| r.fields.into_iter().map(|f| f.key))
        .collect();
    for ((selector, key), value) in old
        .iter()
        .filter(|(key, _)| !after.contains_key(*key) && !current_keys.contains(*key))
    {
        out.push_str(&format!("\n{selector} {{ {key}: {value}; }}\n"));
    }
    Some(out.trim().to_string() + if out.trim().is_empty() { "" } else { "\n" })
}
pub fn restore_host(before: &str, after: &str, current: &str) -> Result<String> {
    let written_block = block(after).ok().flatten();
    if written_block.is_none() {
        let managed = format!("@theme \"{}\"", paths::live_path(ROFI_THEME).display());
        if !before.lines().any(|l| l.trim() == managed)
            && after.lines().any(|l| l.trim() == managed)
        {
            return Ok(current
                .split_inclusive('\n')
                .filter(|l| l.trim() != managed)
                .collect());
        }
        return Ok(current.into());
    }
    let (Some((_, _, written)), Some((start, end, live))) =
        (written_block, block(current).ok().flatten())
    else {
        return Ok(current.into());
    };
    let original = block(before)
        .ok()
        .flatten()
        .map(|(_, _, b)| b)
        .unwrap_or("");
    let Some(kept) = return_css(original, written, live) else {
        return Ok(current.into());
    };
    let mut out = current.to_string();
    let remove_start = if original.is_empty()
        && kept.is_empty()
        && start > 0
        && current.as_bytes()[start - 1] == b'\n'
    {
        start - 1
    } else {
        start
    };
    let replacement = if !original.is_empty() {
        format!("{ROFI_BEGIN}\n{kept}{ROFI_END}\n")
    } else {
        kept
    };
    out.replace_range(remove_start..end, &replacement);
    Ok(out)
}
/// Walker 2.x uses the selected theme's CSS. Do not replace embedded defaults,
/// SCSS, an external theme search path, XML layout or selection to force colours.
fn walker_host(plan: &mut Plan) -> Result<Option<String>> {
    let text = plan
        .text(&paths::live_path(".config/walker/config.toml"))?
        .unwrap_or_default();
    let config: toml_edit::DocumentMut = text.parse().context("Invalid Walker config")?;
    if config.get("additional_theme_location").is_some() {
        return Ok(None);
    }
    let name = config
        .get("theme")
        .map(|v| v.as_str().context("Invalid Walker theme name"))
        .transpose()?
        .unwrap_or("default");
    paths::validate_profile_name(name)?;
    let rel = format!(".config/walker/themes/{name}/style.css");
    if paths::live_path(&format!(".config/walker/themes/{name}/style.scss")).exists() {
        return Ok(None);
    }
    Ok(plan.text(&paths::live_path(&rel))?.map(|_| rel))
}
fn check_version(id: &str) -> Result<bool> {
    if crate::field::no_apply() || which::which(id).is_err() {
        return Ok(true);
    }
    let (status, bytes) =
        paths::output_bounded(std::process::Command::new(id).arg(if id == "rofi" {
            "-version"
        } else {
            "--version"
        }))
        .context("Cannot verify launcher version")?;
    anyhow::ensure!(status.success(), "Cannot verify {id} version");
    let text = String::from_utf8_lossy(&bytes);
    let version = text
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .find(|s| s.starts_with(|c: char| c.is_ascii_digit()))
        .context("Invalid launcher version")?;
    let parts: Vec<_> = version
        .split('.')
        .filter_map(|v| v.parse::<u32>().ok())
        .collect();
    let v = (
        parts.first().copied().unwrap_or(0),
        parts.get(1).copied().unwrap_or(0),
    );
    // Walker has incompatible older generations; those are deliberately skipped.
    if id == "walker" {
        return Ok(v.0 == 2);
    }
    anyhow::ensure!(
        v >= if id == "rofi" { (1, 7) } else { (1, 15) },
        "Unsupported {id} version {version}"
    );
    Ok(true)
}
pub fn plan(source: &Path, package: &Package, plan: &mut Plan) -> Result<()> {
    let generated = package
        .palette
        .as_ref()
        .map(bundle_resources)
        .unwrap_or_default();
    for (id, resource) in [("rofi", ROFI_THEME), ("fuzzel", FUZZEL), ("walker", WALKER)] {
        let requested = package.app_targets.iter().any(|v| v == id);
        let text = if source.join(resource).is_file() {
            Some(std::fs::read_to_string(source.join(resource))?)
        } else if requested {
            generated.get(resource).cloned()
        } else {
            None
        };
        let Some(text) = text else { continue };
        let configured = paths::live_path(&format!(".config/{id}")).is_dir();
        if !configured && which::which(id).is_err() {
            continue;
        }
        if !check_version(id)? {
            continue;
        }
        if id == "fuzzel" {
            let path = paths::live_path(FUZZEL);
            anyhow::ensure!(
                !path.is_symlink(),
                "Linked launcher host requires explicit integration"
            );
            let old = plan.text(&path)?.unwrap_or_default();
            let merged = fuzzel_merge(&text, &old)?;
            app_sync::write(plan, &path, merged)?;
        } else {
            let text = valid_asset(resource, &text)?;
            let host = if id == "rofi" {
                Some(ROFI_HOST.to_string())
            } else {
                walker_host(plan)?
            };
            let Some(host) = host else { continue };
            let resource_path = paths::live_path(resource);
            app_sync::validate_live(&resource_path)?;
            anyhow::ensure!(
                !resource_path.is_symlink(),
                "Linked launcher colour resource requires explicit integration"
            );
            match std::fs::read_to_string(&resource_path) {
                Ok(existing) => {
                    valid_asset(resource, &existing).context(
                        "Existing launcher colour resource contains personal non-colour data",
                    )?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e).context("Cannot read existing launcher colour resource"),
            }
            let path = paths::live_path(&host);
            anyhow::ensure!(
                !path.is_symlink(),
                "Linked launcher host requires explicit integration"
            );
            let old = plan.text(&path)?.unwrap_or_default();
            app_sync::write(plan, &path, overlay(resource, &old, &text)?)?;
            app_sync::write(plan, &paths::live_path(resource), text)?;
        }
    }
    Ok(())
}
