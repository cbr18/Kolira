//! What part of each tracked file is *appearance*.
//!
//! A theme must carry only the look: applying someone's theme (or switching
//! between your own) must not reset screen scaling, virtual desktops, tiling
//! layouts, key bindings, editor behaviour or an SSH host. Most config files
//! mix both, so every tracked file has a [`Policy`]:
//!
//! * [`Policy::Whole`] — the file is appearance only (Klassy, GTK CSS, wofi
//!   CSS, Zen `user.js`/chrome CSS, Kvantum, colour schemes…);
//! * [`Policy::Ini`] / [`Policy::Kv`] / [`Policy::Jsonc`] — only allowed
//!   groups/keys are stored, and on apply they are *merged* into the live
//!   file: allowed keys come from the theme (absent ones are removed), every
//!   other key stays as the user has it;
//! * [`Policy::KittyHost`] / [`Policy::AlacrittyHost`] — the way ricers do it:
//!   the look lives in a separate file (`kitty/theme-profile.conf`,
//!   `alacritty/theme-profile.toml`) that the user's config includes once;
//!   the user's config itself is never stored and only gains the include line;
//! * [`Policy::KwinRules`] — window rules merged rule by rule.
//!
//! The same functions serve save (live → theme), apply (theme → live) and
//! export (theme → archive), so all three agree.

use anyhow::{Context, Result};
use toml_edit::{Array, DocumentMut, Item, Table, Value as TomlValue};

pub const KITTY_HOST: &str = ".config/kitty/kitty.conf";
pub const KITTY_THEME: &str = ".config/kitty/theme-profile.conf";
pub const KITTY_INCLUDE: &str = "include theme-profile.conf";
pub const ALACRITTY_HOST: &str = ".config/alacritty/alacritty.toml";
pub const ALACRITTY_THEME: &str = ".config/alacritty/theme-profile.toml";
pub const ALACRITTY_IMPORT: &str = "~/.config/alacritty/theme-profile.toml";

/// Which keys of an INI group are appearance.
#[derive(Debug, Clone, Copy)]
pub enum Keys {
    All,
    Only(&'static [&'static str]),
    /// `[Plugins]` of kwinrc: `<effect>Enabled`, but not KWin *scripts*
    /// (tiling, snapping… are behaviour).
    KwinEffects,
}

/// `group` is the text between the outer brackets (`Colors:Header][Inactive`);
/// a trailing `*` makes it a prefix.
#[derive(Debug, Clone, Copy)]
pub struct Rule {
    pub group: &'static str,
    pub keys: Keys,
}

const fn r(group: &'static str, keys: Keys) -> Rule {
    Rule { group, keys }
}

#[derive(Debug, Clone, Copy)]
pub enum Policy {
    Whole,
    Ini(&'static [Rule]),
    /// `key = value` lines (btop.conf); entries are exact keys or `prefix*`.
    Kv(&'static [&'static str]),
    /// Top-level members of a JSON-with-comments object; exact or `prefix*`.
    Jsonc(&'static [&'static str]),
    KittyHost,
    AlacrittyHost,
    KwinRules,
}

const KDEGLOBALS: &[Rule] = &[
    r("Colors:*", Keys::All),
    r("ColorEffects:*", Keys::All),
    r("WM", Keys::All),
    r("Icons", Keys::All),
    r(
        "General",
        Keys::Only(&[
            "ColorScheme", "ColorSchemeHash", "AccentColor", "accentColorFromWallpaper",
            "LastUsedCustomAccentColor", "font", "fixed", "smallestReadableFont", "toolBarFont",
            "menuFont", "XftAntialias", "XftHintStyle", "XftSubPixel",
        ]),
    ),
    r(
        "KDE",
        Keys::Only(&[
            "LookAndFeelPackage", "widgetStyle", "contrast", "frameContrast",
            "AnimationDurationFactor", "ShowIconsOnPushButtons", "ShowIconsInMenuItems",
        ]),
    ),
];
const KWINRC: &[Rule] = &[
    r("Effect-*", Keys::All),
    r("org.kde.kdecoration2", Keys::All),
    r("Plugins", Keys::KwinEffects),
];
const PLASMARC: &[Rule] = &[r("Theme", Keys::All)];
const PLASMASHELLRC: &[Rule] = &[r("PlasmaViews*", Keys::All)];
const GTK_SETTINGS: &[Rule] = &[r(
    "Settings",
    Keys::Only(&[
        "gtk-theme-name", "gtk-icon-theme-name", "gtk-cursor-theme-name", "gtk-cursor-theme-size",
        "gtk-font-name", "gtk-application-prefer-dark-theme", "gtk-decoration-layout",
        "gtk-enable-animations", "gtk-button-images", "gtk-menu-images", "gtk-toolbar-style",
        "gtk-xft-antialias", "gtk-xft-hinting", "gtk-xft-hintstyle", "gtk-xft-rgba",
    ]),
)];
const KCMINPUTRC: &[Rule] = &[r("Mouse", Keys::Only(&["cursorTheme", "cursorSize"]))];
const BTOP: &[&str] = &["color_theme", "theme_background", "truecolor", "rounded_corners", "graph_symbol*"];
const VSCODE: &[&str] = &[
    "workbench.colorTheme", "workbench.iconTheme", "workbench.productIconTheme",
    "workbench.colorCustomizations", "workbench.preferredDarkColorTheme",
    "workbench.preferredLightColorTheme", "window.autoDetectColorScheme",
    "editor.tokenColorCustomizations", "editor.semanticTokenColorCustomizations",
    "editor.fontFamily", "editor.fontSize", "editor.fontWeight", "editor.fontLigatures",
    "editor.lineHeight", "editor.lineHighlight*", "terminal.integrated.fontFamily",
    "terminal.integrated.fontSize",
    // Appearance extensions: Custom UI Style, APC, VS Code Animations.
    "apc.*", "animations.*", "custom-ui-style.*", "vscode_custom_css.*",
];
const ZED: &[&str] = &[
    "theme", "icon_theme", "ui_font_*", "buffer_font_*", "theme_overrides", "experimental.theme_overrides",
];

/// The policy for a tracked file (`$HOME`-relative path).
pub fn policy(rel: &str) -> Policy {
    match rel {
        ".config/kdeglobals" => Policy::Ini(KDEGLOBALS),
        ".config/kwinrc" => Policy::Ini(KWINRC),
        ".config/kwinrulesrc" => Policy::KwinRules,
        ".config/plasmarc" => Policy::Ini(PLASMARC),
        ".config/kcminputrc" => Policy::Ini(KCMINPUTRC),
        ".config/plasmashellrc" => Policy::Ini(PLASMASHELLRC),
        ".config/gtk-3.0/settings.ini" | ".config/gtk-4.0/settings.ini" => Policy::Ini(GTK_SETTINGS),
        ".config/btop/btop.conf" => Policy::Kv(BTOP),
        ".config/Code/User/settings.json"
        | ".config/Code - OSS/User/settings.json"
        | ".config/VSCodium/User/settings.json"
        | ".config/Cursor/User/settings.json" => Policy::Jsonc(VSCODE),
        ".config/zed/settings.json" => Policy::Jsonc(ZED),
        KITTY_HOST => Policy::KittyHost,
        ALACRITTY_HOST => Policy::AlacrittyHost,
        _ => Policy::Whole,
    }
}

fn pattern_matches(pattern: &str, name: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => pattern == name,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// INI
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
struct Section {
    /// Raw header line (`[Colors:View]`), `None` for lines before any header.
    header: Option<String>,
    lines: Vec<String>,
}

impl Section {
    fn group(&self) -> &str {
        self.header
            .as_deref()
            .map(|h| h.trim().strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(h))
            .unwrap_or("")
    }
}

fn parse_ini(text: &str) -> Vec<Section> {
    let mut out = vec![Section::default()];
    for line in text.lines() {
        if line.starts_with('[') {
            out.push(Section { header: Some(line.to_string()), lines: Vec::new() });
        } else if let Some(last) = out.last_mut() {
            last.lines.push(line.to_string());
        }
    }
    out
}

fn render_ini(sections: &[Section]) -> String {
    let mut out: Vec<String> = Vec::new();
    for s in sections {
        if s.header.is_none() && s.lines.iter().all(|l| l.trim().is_empty()) {
            continue;
        }
        if let Some(h) = &s.header {
            if out.last().is_some_and(|l| !l.is_empty()) {
                out.push(String::new());
            }
            out.push(h.clone());
        }
        let mut lines = s.lines.clone();
        while lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
        }
        out.extend(lines);
    }
    let mut text = out.join("\n");
    text.push('\n');
    text
}

/// `Name[ru]` / `Key[$e]` → `Name` / `Key`.
fn base_key(line: &str) -> Option<&str> {
    let (key, _) = line.split_once('=')?;
    let key = key.trim();
    if key.starts_with('#') || key.starts_with(';') {
        return None;
    }
    Some(key.split('[').next().unwrap_or(key))
}

fn kwin_script(plugin: &str) -> bool {
    let home = crate::paths::home_dir();
    [
        crate::paths::data_home().join("kwin/scripts"),
        home.join(".local/share/kwin/scripts"),
        std::path::PathBuf::from("/usr/share/kwin/scripts"),
        std::path::PathBuf::from("/usr/share/kwin-x11/scripts"),
        std::path::PathBuf::from("/usr/share/kwin-wayland/scripts"),
    ]
    .iter()
    .any(|root| root.join(plugin).exists())
}

fn rule_for<'a>(rules: &'a [Rule], group: &str) -> Option<&'a Rule> {
    rules.iter().find(|r| pattern_matches(r.group, group))
}

fn key_allowed(rule: &Rule, key: &str) -> bool {
    match rule.keys {
        Keys::All => true,
        Keys::Only(list) => list.contains(&key),
        Keys::KwinEffects => key.strip_suffix("Enabled").is_some_and(|p| !kwin_script(p)),
    }
}

fn allowed_line(rules: &[Rule], group: &str, line: &str) -> bool {
    match (rule_for(rules, group), base_key(line)) {
        (Some(rule), Some(key)) => key_allowed(rule, key),
        _ => false,
    }
}

/// Only the appearance part of an INI file.
pub fn ini_filter(text: &str, rules: &[Rule]) -> String {
    let sections: Vec<Section> = parse_ini(text)
        .into_iter()
        .filter(|s| s.header.is_some())
        .filter_map(|s| {
            let group = s.group().to_string();
            let lines: Vec<String> = s.lines.iter().filter(|l| allowed_line(rules, &group, l)).cloned().collect();
            (!lines.is_empty()).then_some(Section { header: s.header, lines })
        })
        .collect();
    render_ini(&sections)
}

/// `live` with its appearance keys replaced by the theme's: allowed keys the
/// theme lacks are removed, every other key is kept as it is.
pub fn ini_merge(theme: &str, live: &str, rules: &[Rule]) -> String {
    let theme = parse_ini(&ini_filter(theme, rules));
    let mut live = parse_ini(live);
    for s in live.iter_mut().filter(|s| s.header.is_some()) {
        let group = s.group().to_string();
        s.lines.retain(|l| !allowed_line(rules, &group, l));
    }
    for t in theme.into_iter().filter(|s| s.header.is_some()) {
        let group = t.group().to_string();
        match live.iter_mut().find(|s| s.header.is_some() && s.group() == group) {
            Some(s) => {
                while s.lines.last().is_some_and(|l| l.trim().is_empty()) {
                    s.lines.pop();
                }
                s.lines.extend(t.lines);
            }
            None => live.push(t),
        }
    }
    // Drop sections emptied by the removal (e.g. a colour group the theme lacks).
    live.retain(|s| s.header.is_none() || s.lines.iter().any(|l| !l.trim().is_empty()));
    render_ini(&live)
}

/// Window rules: the theme's rule groups replace live ones with the same id;
/// the user's other rules stay. `[General]` lists the union.
pub fn kwinrules_merge(theme: &str, live: &str) -> String {
    let theme = parse_ini(theme);
    let mut live = parse_ini(live);
    let ids = |sections: &[Section]| -> Vec<String> {
        sections.iter().filter(|s| s.header.is_some() && s.group() != "General").map(|s| s.group().to_string()).collect()
    };
    let mut order = ids(&live);
    for id in ids(&theme) {
        if !order.contains(&id) {
            order.push(id);
        }
    }
    for t in theme.into_iter().filter(|s| s.header.is_some() && s.group() != "General") {
        match live.iter_mut().find(|s| s.header.is_some() && s.group() == t.group()) {
            Some(s) => s.lines = t.lines,
            None => live.push(t),
        }
    }
    let general = match live.iter_mut().find(|s| s.group() == "General" && s.header.is_some()) {
        Some(g) => g,
        None => {
            live.insert(1.min(live.len()), Section { header: Some("[General]".into()), lines: Vec::new() });
            live.iter_mut().find(|s| s.group() == "General" && s.header.is_some()).expect("just inserted")
        }
    };
    general.lines.retain(|l| !matches!(base_key(l), Some("count" | "rules" | "Order")));
    general.lines.insert(0, format!("rules={}", order.join(",")));
    general.lines.insert(0, format!("count={}", order.len()));
    render_ini(&live)
}

// ─────────────────────────────────────────────────────────────────────────────
// key = value (btop.conf)
// ─────────────────────────────────────────────────────────────────────────────

fn kv_key(line: &str) -> Option<&str> {
    let t = line.trim_start();
    if t.starts_with('#') {
        return None;
    }
    Some(t.split_once('=')?.0.trim())
}

fn kv_allowed(keys: &[&str], line: &str) -> bool {
    kv_key(line).is_some_and(|k| keys.iter().any(|p| pattern_matches(p, k)))
}

pub fn kv_filter(text: &str, keys: &[&str]) -> String {
    let mut out: String = text.lines().filter(|l| kv_allowed(keys, l)).collect::<Vec<_>>().join("\n");
    out.push('\n');
    out
}

/// Allowed lines replaced in place (order kept), new ones appended.
pub fn kv_merge(theme: &str, live: &str, keys: &[&str]) -> String {
    let theme_lines: Vec<&str> = theme.lines().filter(|l| kv_allowed(keys, l)).collect();
    let mut used = vec![false; theme_lines.len()];
    let mut out: Vec<String> = Vec::new();
    for line in live.lines() {
        if !kv_allowed(keys, line) {
            out.push(line.to_string());
            continue;
        }
        let key = kv_key(line);
        if let Some(i) = theme_lines.iter().position(|t| kv_key(t) == key) {
            out.push(theme_lines[i].to_string());
            used[i] = true;
        }
    }
    for (i, line) in theme_lines.iter().enumerate() {
        if !used[i] {
            out.push(line.to_string());
        }
    }
    let mut text = out.join("\n");
    text.push('\n');
    text
}

// ─────────────────────────────────────────────────────────────────────────────
// JSON with comments (VS Code, Cursor, Zed) — patched by position
// ─────────────────────────────────────────────────────────────────────────────

/// A top-level member: key, byte span of the whole member (key … value) and
/// of its value.
#[derive(Debug, Clone)]
struct Member {
    key: String,
    start: usize,
    value: (usize, usize),
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() {
        match b[i] {
            b' ' | b'\t' | b'\r' | b'\n' => i += 1,
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(b.len());
            }
            _ => break,
        }
    }
    i
}

fn string_end(b: &[u8], mut i: usize) -> usize {
    i += 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    b.len()
}

/// End of the JSON value starting at `i` (objects/arrays balanced, strings and
/// comments respected).
fn value_end(b: &[u8], i: usize) -> usize {
    let i = skip_ws(b, i);
    match b.get(i) {
        Some(b'"') => string_end(b, i),
        Some(b'{') | Some(b'[') => {
            let mut depth = 0i32;
            let mut j = i;
            while j < b.len() {
                match b[j] {
                    b'"' => {
                        j = string_end(b, j);
                        continue;
                    }
                    b'/' if matches!(b.get(j + 1), Some(b'/') | Some(b'*')) => {
                        j = skip_ws(b, j);
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            return j + 1;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            b.len()
        }
        Some(_) => {
            let mut j = i;
            while j < b.len() && !matches!(b[j], b',' | b'}' | b']' | b'\n' | b'\r') && !(b[j] == b'/' && matches!(b.get(j + 1), Some(b'/') | Some(b'*'))) {
                j += 1;
            }
            while j > i && matches!(b[j - 1], b' ' | b'\t') {
                j -= 1;
            }
            j
        }
        None => b.len(),
    }
}

/// Top-level members of the root object (empty when the text is not one).
fn members(text: &str) -> Vec<Member> {
    let b = text.as_bytes();
    let mut i = skip_ws(b, 0);
    if b.get(i) != Some(&b'{') {
        return Vec::new();
    }
    i += 1;
    let mut out = Vec::new();
    loop {
        i = skip_ws(b, i);
        match b.get(i) {
            Some(b'"') => {}
            _ => return out,
        }
        let start = i;
        let kend = string_end(b, i);
        let key = serde_json::from_str::<String>(&text[i..kend]).unwrap_or_default();
        i = skip_ws(b, kend);
        if b.get(i) == Some(&b':') {
            i += 1;
        }
        let vstart = skip_ws(b, i);
        let vend = value_end(b, vstart);
        out.push(Member { key, start, value: (vstart, vend) });
        i = skip_ws(b, vend);
        if b.get(i) == Some(&b',') {
            i += 1;
        }
    }
}

fn json_allowed(keys: &[&str], key: &str) -> bool {
    keys.iter().any(|p| pattern_matches(p, key))
}

/// The appearance members as a small JSON object.
pub fn jsonc_filter(text: &str, keys: &[&str]) -> String {
    let body: Vec<String> = members(text)
        .into_iter()
        .filter(|m| json_allowed(keys, &m.key))
        .map(|m| format!("  {}: {}", serde_json::to_string(&m.key).unwrap_or_default(), &text[m.value.0..m.value.1]))
        .collect();
    if body.is_empty() {
        return "{}\n".into();
    }
    format!("{{\n{}\n}}\n", body.join(",\n"))
}

/// `live` with its appearance members replaced by the theme's — positional
/// patches, the rest of the file (comments, order, other keys) untouched.
pub fn jsonc_merge(theme: &str, live: &str, keys: &[&str]) -> String {
    let theme_members: Vec<(String, String)> = members(theme)
        .into_iter()
        .filter(|m| json_allowed(keys, &m.key))
        .map(|m| (m.key.clone(), theme[m.value.0..m.value.1].to_string()))
        .collect();
    if members(live).is_empty() && !live.trim_start().starts_with('{') {
        return jsonc_filter(theme, keys);
    }
    let mut text = live.to_string();
    let mut done: Vec<String> = Vec::new();
    // Replace or remove, from the end so earlier spans stay valid.
    let mut live_members = members(&text);
    live_members.reverse();
    for m in live_members.into_iter().filter(|m| json_allowed(keys, &m.key)) {
        if let Some((_, value)) = theme_members.iter().find(|(k, _)| *k == m.key) {
            text.replace_range(m.value.0..m.value.1, value);
            done.push(m.key);
            continue;
        }
        // Remove the member and one adjacent comma.
        let b = text.as_bytes();
        let after = skip_ws(b, m.value.1);
        if b.get(after) == Some(&b',') {
            let end = skip_ws(b, after + 1);
            text.replace_range(m.start..end, "");
        } else {
            let mut s = m.start;
            let before = text[..s].trim_end_matches([' ', '\t', '\r', '\n']);
            if before.ends_with(',') {
                s = before.len() - 1;
            }
            text.replace_range(s..m.value.1, "");
        }
    }
    let missing: Vec<&(String, String)> = theme_members.iter().filter(|(k, _)| !done.contains(k)).collect();
    if !missing.is_empty() {
        if let Some(close) = text.rfind('}') {
            let body = text[..close].trim_end().to_string();
            let mut add = String::new();
            let mut sep = if body.ends_with('{') || body.ends_with(',') { "" } else { "," };
            for (k, v) in missing {
                add.push_str(&format!("{sep}\n  {}: {v}", serde_json::to_string(k).unwrap_or_default()));
                sep = ",";
            }
            text = format!("{body}{add}\n{}", &text[close..]);
        }
    }
    text
}

// ─────────────────────────────────────────────────────────────────────────────
// kitty: appearance in kitty/theme-profile.conf, included by kitty.conf
// ─────────────────────────────────────────────────────────────────────────────

const KITTY_LOOK: &[&str] = &[
    "font_family", "bold_font", "italic_font", "bold_italic_font", "font_size", "font_features",
    "modify_font", "disable_ligatures", "symbol_map", "narrow_symbols", "text_composition_strategy",
    "foreground", "background", "background_*", "selection_*", "cursor", "cursor_*", "url_color",
    "url_style", "mark1_*", "mark2_*", "mark3_*", "active_border_color", "inactive_border_color",
    "bell_border_color", "window_border_width", "window_margin_width", "window_padding_width",
    "single_window_*", "draw_minimal_borders", "hide_window_decorations", "tab_bar_*",
    "tab_powerline_style", "tab_separator", "tab_activity_symbol", "active_tab_*",
    "inactive_tab_*", "dim_opacity", "transparent_background_colors", "wayland_titlebar_color",
    "undercurl_style",
];

fn kitty_look(line: &str) -> bool {
    let t = line.trim_start();
    let Some(word) = t.split_whitespace().next() else { return false };
    if word.starts_with('#') {
        return false;
    }
    if let Some(n) = word.strip_prefix("color") {
        return !n.is_empty() && n.chars().all(|c| c.is_ascii_digit());
    }
    KITTY_LOOK.iter().any(|p| pattern_matches(p, word))
}

/// Split a `kitty.conf` into (appearance lines incl. the generated palette
/// block, the rest without the `include theme-profile.conf` line).
pub fn kitty_split(host: &str) -> (String, String) {
    let mut look: Vec<&str> = Vec::new();
    let mut rest: Vec<&str> = Vec::new();
    let mut in_palette = false;
    for line in host.lines() {
        if line.starts_with("# BEGIN theme-profile palette") {
            in_palette = true;
        }
        if in_palette {
            look.push(line);
            if line.starts_with(crate::palette::KITTY_END) {
                in_palette = false;
            }
            continue;
        }
        if line.trim() == KITTY_INCLUDE {
            continue;
        }
        if kitty_look(line) {
            look.push(line);
        } else {
            rest.push(line);
        }
    }
    let join = |v: Vec<&str>| {
        let mut s = v.join("\n");
        while s.ends_with("\n\n") {
            s.pop();
        }
        if !s.is_empty() && !s.ends_with('\n') {
            s.push('\n');
        }
        s
    };
    (join(look), join(rest))
}

const KITTY_NOTE: &str = "# Theme (theme-profile): appearance lives in theme-profile.conf";

/// The user's kitty.conf without appearance lines, ending with the include
/// (last, so the theme wins over anything above it). Idempotent.
pub fn kitty_host(host: &str) -> String {
    let (_, rest) = kitty_split(host);
    let rest: Vec<&str> = rest.lines().filter(|l| l.trim() != KITTY_NOTE).collect();
    let rest = rest.join("\n");
    let rest = rest.trim_end();
    if rest.is_empty() {
        format!("{KITTY_INCLUDE}\n")
    } else {
        format!("{rest}\n\n{KITTY_NOTE}\n{KITTY_INCLUDE}\n")
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Alacritty: appearance in alacritty/theme-profile.toml, imported by alacritty.toml
// ─────────────────────────────────────────────────────────────────────────────

const ALACRITTY_WINDOW: &[&str] = &["opacity", "blur", "padding", "dynamic_padding", "decorations", "decorations_theme_variant"];
const ALACRITTY_CURSOR: &[&str] = &["style", "unfocused_hollow", "thickness"];

/// Split `alacritty.toml` into (appearance TOML, the rest with the import).
pub fn alacritty_split(host: &str) -> Result<(String, String)> {
    let mut doc: DocumentMut = host.parse().context("alacritty.toml is not valid TOML")?;
    let mut look = DocumentMut::new();
    for table in ["colors", "font"] {
        if let Some(item) = doc.remove(table) {
            look[table] = item;
        }
    }
    for (table, keys) in [("window", ALACRITTY_WINDOW), ("cursor", ALACRITTY_CURSOR)] {
        if let Some(t) = doc.get_mut(table).and_then(Item::as_table_like_mut) {
            for key in keys {
                if let Some(item) = t.remove(key) {
                    if look.get(table).is_none() {
                        look[table] = Item::Table(Table::new());
                    }
                    look[table][key] = item;
                }
            }
            if t.is_empty() {
                doc.remove(table);
            }
        }
    }
    Ok((look.to_string(), doc.to_string()))
}

/// The user's alacritty.toml without appearance, importing the theme file.
pub fn alacritty_host(host: &str) -> Result<String> {
    let (_, rest) = alacritty_split(host)?;
    let mut doc: DocumentMut = rest.parse().context("alacritty.toml is not valid TOML")?;
    // Alacritty ≥ 0.14 reads `general.import`; older ones a top-level `import`.
    let legacy = doc.get("import").is_some();
    let imports = if legacy {
        doc.get_mut("import")
    } else {
        if doc.get("general").is_none() {
            doc["general"] = Item::Table(Table::new());
        }
        if doc["general"].get("import").is_none() {
            doc["general"]["import"] = toml_edit::value(Array::new());
        }
        doc["general"].get_mut("import")
    };
    if let Some(arr) = imports.and_then(Item::as_array_mut) {
        if !arr.iter().any(|v| v.as_str() == Some(ALACRITTY_IMPORT)) {
            arr.push(TomlValue::from(ALACRITTY_IMPORT));
        }
    }
    Ok(doc.to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// Save / apply / export entry points
// ─────────────────────────────────────────────────────────────────────────────

/// What a theme stores for tracked file `rel` given its current text; `None`
/// means "store nothing" (include hosts).
pub fn for_theme(rel: &str, text: &str) -> Option<String> {
    match policy(rel) {
        Policy::Whole | Policy::KwinRules => Some(text.to_string()),
        Policy::Ini(rules) => Some(ini_filter(text, rules)),
        Policy::Kv(keys) => Some(kv_filter(text, keys)),
        Policy::Jsonc(keys) => Some(jsonc_filter(text, keys)),
        Policy::KittyHost | Policy::AlacrittyHost => None,
    }
}

/// The new live text of `rel` when applying `theme_text` over `live_text`.
pub fn applied(rel: &str, theme_text: &str, live_text: Option<&str>) -> Result<String> {
    let live = live_text.unwrap_or("");
    Ok(match policy(rel) {
        Policy::Whole => theme_text.to_string(),
        Policy::Ini(rules) => ini_merge(theme_text, live, rules),
        Policy::KwinRules => kwinrules_merge(theme_text, live),
        Policy::Kv(keys) => kv_merge(theme_text, live, keys),
        Policy::Jsonc(keys) => jsonc_merge(theme_text, live, keys),
        Policy::KittyHost => kitty_host(live),
        Policy::AlacrittyHost => alacritty_host(live)?,
    })
}

/// The include file an old-style theme (appearance still inside the host
/// config) implies: `(include rel, text)`.
pub fn include_from_host(rel: &str, host_text: &str) -> Result<Option<(&'static str, String)>> {
    Ok(match policy(rel) {
        Policy::KittyHost => {
            let (look, _) = kitty_split(host_text);
            (!look.trim().is_empty()).then_some((KITTY_THEME, look))
        }
        Policy::AlacrittyHost => {
            let (look, _) = alacritty_split(host_text)?;
            (!look.trim().is_empty()).then_some((ALACRITTY_THEME, look))
        }
        _ => None,
    })
}

/// A stored theme file as it should be: the look only, include hosts turned
/// into their include file. Returns the entries to keep (`rel`, text) — empty
/// when nothing of the file belongs in a theme. `has_include` tells whether
/// the theme already carries the host's include file.
pub fn normalize(rel: &str, text: &str, has_include: bool) -> Result<Vec<(String, String)>> {
    Ok(match policy(rel) {
        Policy::Whole | Policy::KwinRules => vec![(rel.to_string(), text.to_string())],
        Policy::KittyHost | Policy::AlacrittyHost => match include_from_host(rel, text)? {
            Some((inc, look)) if !has_include => vec![(inc.to_string(), look)],
            _ => Vec::new(),
        },
        _ => for_theme(rel, text).map(|t| vec![(rel.to_string(), t)]).unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const KDEGLOBALS_LIVE: &str = "[General]\nColorScheme=Mine\nTerminalApplication=kitty\nAccentColor=1,2,3\n\n[KScreen]\nScreenScaleFactors=eDP-2=1.25;\n\n[Colors:View]\nBackgroundNormal=1,1,1\n\n[KDE]\nwidgetStyle=Breeze\nSingleClick=false\n";
    const KDEGLOBALS_THEME: &str = "[General]\nColorScheme=Dracula\nTerminalApplication=konsole\n\n[KScreen]\nScreenScaleFactors=eDP-2=2;\n\n[Colors:Window]\nBackgroundNormal=40,42,54\n\n[KDE]\nwidgetStyle=kvantum\n";

    #[test]
    fn ini_filter_keeps_only_appearance() {
        let f = ini_filter(KDEGLOBALS_THEME, KDEGLOBALS);
        assert!(f.contains("ColorScheme=Dracula") && f.contains("[Colors:Window]") && f.contains("widgetStyle=kvantum"));
        assert!(!f.contains("Terminal") && !f.contains("KScreen") && !f.contains("Scale"));
    }

    #[test]
    fn ini_merge_keeps_personal_keys_and_replaces_the_look() {
        let m = ini_merge(KDEGLOBALS_THEME, KDEGLOBALS_LIVE, KDEGLOBALS);
        // Personal: untouched.
        assert!(m.contains("TerminalApplication=kitty") && m.contains("ScreenScaleFactors=eDP-2=1.25;") && m.contains("SingleClick=false"));
        // Look: from the theme; the theme's absent accent / colour group are removed.
        assert!(m.contains("ColorScheme=Dracula") && m.contains("widgetStyle=kvantum") && m.contains("[Colors:Window]"));
        assert!(!m.contains("AccentColor=1,2,3") && !m.contains("[Colors:View]") && !m.contains("ColorScheme=Mine"));
        // Idempotent.
        assert_eq!(ini_merge(KDEGLOBALS_THEME, &m, KDEGLOBALS), m);
    }

    #[test]
    fn kwin_scripts_are_not_effects() {
        let rule = Rule { group: "Plugins", keys: Keys::KwinEffects };
        assert!(key_allowed(&rule, "blurEnabled"));
        assert!(!key_allowed(&rule, "blurStrength"));
    }

    #[test]
    fn kwinrules_merge_keeps_users_rules() {
        let live = "[General]\ncount=1\nrules=a\n\n[a]\nDescription=mine\n";
        let theme = "[General]\ncount=1\nrules=b\n\n[b]\nDescription=theme\nopacityactive=90\n";
        let m = kwinrules_merge(theme, live);
        assert!(m.contains("count=2") && m.contains("rules=a,b") && m.contains("Description=mine") && m.contains("opacityactive=90"));
    }

    #[test]
    fn btop_merge_keeps_behaviour() {
        let live = "color_theme = \"Default\"\nupdate_ms = 1000\nproc_sorting = \"cpu lazy\"\nrounded_corners = True\n";
        let theme = "color_theme = \"dracula\"\nupdate_ms = 2000\nrounded_corners = False\n";
        let m = kv_merge(theme, live, BTOP);
        assert_eq!(m, "color_theme = \"dracula\"\nupdate_ms = 1000\nproc_sorting = \"cpu lazy\"\nrounded_corners = False\n");
    }

    #[test]
    fn jsonc_merge_patches_only_the_look() {
        let live = "{\n  // mine\n  \"remote.SSH.remotePlatform\": {\"203.0.113.7\": \"linux\"},\n  \"workbench.colorTheme\": \"Doki\",\n  \"editor.wordWrap\": \"on\"\n}\n";
        let theme = "{\n  \"workbench.colorTheme\": \"Dracula Theme\",\n  \"workbench.colorCustomizations\": {\"editor.background\": \"#282a36\"},\n  \"editor.wordWrap\": \"off\"\n}";
        let m = jsonc_merge(theme, live, VSCODE);
        assert!(m.contains("// mine") && m.contains("203.0.113.7") && m.contains("\"editor.wordWrap\": \"on\""));
        assert!(m.contains("\"workbench.colorTheme\": \"Dracula Theme\"") && m.contains("#282a36"));
        let parsed: serde_json::Value = serde_json::from_str(&m.replace("  // mine\n", "")).expect("valid JSON");
        assert_eq!(parsed["workbench.colorTheme"], "Dracula Theme");
        // Filter stores only the look.
        let f = jsonc_filter(live, VSCODE);
        assert!(f.contains("Doki") && !f.contains("203.0.113") && !f.contains("wordWrap"));
        // A look key the theme lacks is removed.
        let m2 = jsonc_merge("{\"theme\": \"One Dark\"}", "{\n  \"theme\": \"Ayu\",\n  \"icon_theme\": \"Zed\",\n  \"agent\": {\"x\": 1}\n}\n", ZED);
        let p2: serde_json::Value = serde_json::from_str(&m2).expect("valid JSON");
        assert_eq!(p2["theme"], "One Dark");
        assert!(p2.get("icon_theme").is_none() && p2["agent"]["x"] == 1);
    }

    #[test]
    fn kitty_split_moves_only_the_look() {
        let host = "font_size 11\nmap ctrl+c copy\nbackground #000000\ncolor1 #ff0000\nscrollback_lines 5000\nbackground_opacity 0.5\ndynamic_background_opacity yes\nconfirm_os_window_close 0\n";
        let (look, rest) = kitty_split(host);
        assert_eq!(look, "font_size 11\nbackground #000000\ncolor1 #ff0000\nbackground_opacity 0.5\n");
        assert!(rest.contains("map ctrl+c copy") && rest.contains("dynamic_background_opacity yes") && rest.contains("confirm_os_window_close"));
        let h = kitty_host(host);
        assert!(h.trim_end().ends_with(KITTY_INCLUDE) && !h.contains("color1"));
        assert_eq!(kitty_host(&h), h, "idempotent");
    }

    #[test]
    fn alacritty_split_and_import() {
        let host = "[env]\nTERM = \"xterm-256color\"\n\n[window]\nopacity = 0.5\nstartup_mode = \"Maximized\"\n\n[colors.primary]\nbackground = \"#000000\"\n\n[[keyboard.bindings]]\nkey = \"N\"\nmods = \"Control\"\naction = \"SpawnNewInstance\"\n";
        let (look, _) = alacritty_split(host).unwrap();
        assert!(look.contains("opacity = 0.5") && look.contains("#000000") && !look.contains("TERM"));
        let h = alacritty_host(host).unwrap();
        assert!(h.contains("startup_mode") && h.contains("SpawnNewInstance") && h.contains(ALACRITTY_IMPORT));
        assert!(!h.contains("opacity") && !h.contains("#000000"));
        assert_eq!(alacritty_host(&h).unwrap(), h, "idempotent");
    }
}
