//! What part of each tracked file is *appearance*.
//!
//! A theme must carry only the look: applying someone's theme (or switching
//! between your own) must not reset screen scaling, virtual desktops, tiling
//! layouts, key bindings, editor behaviour or an SSH host. Most config files
//! mix both, so every tracked file has a [`Policy`]:
//!
//! * [`Policy::Whole`] — the file is appearance only (Klassy, GTK CSS, wofi
//!   CSS, Kvantum, colour schemes…);
//! * [`Policy::Ini`] / [`Policy::Kv`] / [`Policy::Jsonc`] — only allowed
//!   groups/keys are stored, and on apply they are *merged* into the live
//!   file: allowed keys come from the theme (absent ones are removed), every
//!   other key stays as the user has it;
//! * [`Policy::KittyHost`] / [`Policy::AlacrittyHost`] — the way ricers do it:
//!   the look lives in a separate file (`kitty/kolira.conf`,
//!   `alacritty/kolira.toml`) that the user's config includes once;
//!   the user's config itself is never stored and only gains the include line;
//! * [`Policy::KwinRc`] — [`KWINRC`] without the settings of effects the theme
//!   keeps off (those stay as the user has them);
//! * [`Policy::KwinRules`] — window rules merged rule by rule, look
//!   properties only;
//! * [`Policy::Appletsrc`] / [`Policy::PanelViews`] — the Plasma shell's look
//!   (wallpaper, Panel Colorizer, panel floating/opacity)
//!   merged into the user's own layout (`layout.rs`).
//!
//! The same functions serve save (live → theme), apply (theme → live) and
//! export (theme → archive), so all three agree.

use anyhow::{Context, Result};
use toml_edit::{Array, DocumentMut, Item, Table, Value as TomlValue};

/// Retired messenger integrations are outside theme ownership, including old snapshots.
pub fn is_removed_messenger_path(rel: &str) -> bool {
    [
        ".config/telegram-desktop",
        ".config/kotatogram-desktop",
        ".local/share/TelegramDesktop",
        ".local/share/KotatogramDesktop",
        ".var/app/org.telegram.desktop",
        ".var/app/io.github.kotatogram",
    ]
    .iter()
    .any(|root| std::path::Path::new(rel).starts_with(root))
}

/// Retired clients are excluded even from legacy snapshots and undo journals.
pub fn is_retired_app_path(rel: &str) -> bool {
    is_removed_messenger_path(rel)
        || [
            ".config/chromium",
            ".config/google-chrome",
            ".var/app/org.chromium.Chromium",
            ".var/app/com.google.Chrome",
        ]
        .iter()
        .any(|root| std::path::Path::new(rel).starts_with(root))
}

pub fn is_preserved(rel: &str) -> bool {
    crate::launcher_apps::preserved(rel)
        || crate::four_apps::preserved(rel)
        || is_retired_app_path(rel)
        || (std::path::Path::new(rel).starts_with(".vim") && rel != crate::third_apps::VIM_DATA)
        || rel == crate::terminal_apps::FZF_LOADER
        || std::path::Path::new(rel).starts_with(".config/libreoffice")
        || std::path::Path::new(rel).starts_with(".config/fastfetch")
        || (crate::browser_appearance::is_profile_file(rel)
            && rel.ends_with(".css")
            && !rel.ends_with("/chrome/userChrome.css")
            && !rel.ends_with("/chrome/kolira-colors.css"))
}

pub const KITTY_HOST: &str = ".config/kitty/kitty.conf";
pub const KITTY_THEME: &str = ".config/kitty/kolira.conf";
pub const KITTY_INCLUDE: &str = "include kolira.conf";
pub const ALACRITTY_HOST: &str = ".config/alacritty/alacritty.toml";
pub const ALACRITTY_THEME: &str = ".config/alacritty/kolira.toml";
pub const ALACRITTY_IMPORT: &str = "kolira.toml";

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
    Native,
    Ini(&'static [Rule]),
    /// `key = value` lines (btop.conf); entries are exact keys or `prefix*`.
    Kv(&'static [&'static str]),
    /// Top-level members of a JSON-with-comments object; exact or `prefix*`.
    Jsonc(&'static [&'static str]),
    /// `user_pref("name", value);` lines (Firefox user.js); exact or `prefix*`.
    Prefs(&'static [&'static str]),
    BrowserHost,
    KittyHost,
    AlacrittyHost,
    /// Shell config with an `oh-my-posh init` line: never stored, only the
    /// `--config` path is pointed at the theme's prompt file.
    OmpHost,
    /// ghostty config: never stored, gains `config-file = ?kolira`.
    GhosttyHost,
    /// Lines selected by a predicate, merged by name (fish colours, bat theme).
    Lines(fn(&str) -> bool),
    KwinRules,
    /// `kwinrc`: [`KWINRC`] without the settings of effects the theme keeps off.
    KwinRc,
    /// `plasma-org.kde.plasma.desktop-appletsrc`: look merged into the live layout.
    Appletsrc,
    /// `plasmashellrc`: per-panel `floating` / `panelOpacity`.
    PanelViews,
}

const FOOT_KEYS: &[&str] = &[
    "background",
    "foreground",
    "selection-background",
    "selection-foreground",
    "cursor",
    "regular0",
    "regular1",
    "regular2",
    "regular3",
    "regular4",
    "regular5",
    "regular6",
    "regular7",
    "bright0",
    "bright1",
    "bright2",
    "bright3",
    "bright4",
    "bright5",
    "bright6",
    "bright7",
];
const FOOT: &[Rule] = &[
    r("colors", Keys::Only(FOOT_KEYS)),
    r("colors-dark", Keys::Only(FOOT_KEYS)),
    r("colors-light", Keys::Only(FOOT_KEYS)),
];
const PRISM: &[Rule] = &[r("General", Keys::Only(&["ApplicationTheme"]))];

const KDEGLOBALS: &[Rule] = &[
    r("Colors:*", Keys::All),
    r("ColorEffects:*", Keys::All),
    r("WM", Keys::All),
    r("Icons", Keys::All),
    r(
        "General",
        Keys::Only(&[
            "ColorScheme",
            "ColorSchemeHash",
            "AccentColor",
            "accentColorFromWallpaper",
            "LastUsedCustomAccentColor",
            "font",
            "fixed",
            "smallestReadableFont",
            "toolBarFont",
            "menuFont",
            "activeFont",
            "XftAntialias",
            "XftHintStyle",
            "XftSubPixel",
        ]),
    ),
    r(
        "KDE",
        Keys::Only(&[
            "LookAndFeelPackage",
            "widgetStyle",
            "contrast",
            "frameContrast",
            "AnimationDurationFactor",
            "ShowIconsOnPushButtons",
            "ShowIconsInMenuItems",
        ]),
    ),
];
const KWINRC: &[Rule] = &[
    r("Effect-*", Keys::All),
    // Rounded Corners (kwin4_effect_shapecorners) keeps its settings here.
    r("Round-Corners", Keys::All),
    r("org.kde.kdecoration2", Keys::All),
    r("Plugins", Keys::KwinEffects),
];
const PLASMARC: &[Rule] = &[r("Theme", Keys::All)];
const GTK_SETTINGS: &[Rule] = &[r(
    "Settings",
    Keys::Only(&[
        "gtk-theme-name",
        "gtk-icon-theme-name",
        "gtk-cursor-theme-name",
        "gtk-font-name",
        "gtk-application-prefer-dark-theme",
        "gtk-decoration-layout",
        "gtk-enable-animations",
        "gtk-button-images",
        "gtk-menu-images",
        "gtk-toolbar-style",
        "gtk-xft-antialias",
        "gtk-xft-hinting",
        "gtk-xft-hintstyle",
        "gtk-xft-rgba",
    ]),
)];
// Sizes (fonts, cursor) are never part of a theme: screens differ. Font
// *families* travel; a size inside a font string is kept from the live file
// (see `keep_live_size`).
const KONSOLE_PROFILE: &[Rule] = &[r("Appearance", Keys::Only(&["ColorScheme", "Font"]))];
const SPICETIFY: &[Rule] = &[r(
    "Setting",
    Keys::Only(&[
        "current_theme",
        "color_scheme",
        "inject_css",
        "replace_colors",
        "overwrite_assets",
        "inject_theme_js",
    ]),
)];
const CAVA_RULES: &[Rule] = &[r("color", Keys::All)];
const KCMINPUTRC: &[Rule] = &[r("Mouse", Keys::Only(&["cursorTheme"]))];
const BTOP: &[&str] = &[
    "color_theme",
    "theme_background",
    "truecolor",
    "rounded_corners",
    "graph_symbol*",
];
// VSCodium's new integration owns colours only; a sparse generated snapshot
// must not remove personal fonts, icons or appearance extension settings.
const VSCODIUM_COLOURS: &[&str] = &[
    "workbench.colorTheme",
    "workbench.colorCustomizations",
    "workbench.preferredDarkColorTheme",
    "workbench.preferredLightColorTheme",
    "window.autoDetectColorScheme",
    "editor.tokenColorCustomizations",
    "editor.semanticTokenColorCustomizations",
];
const VSCODE: &[&str] = &[
    "workbench.colorTheme",
    "workbench.iconTheme",
    "workbench.productIconTheme",
    "workbench.colorCustomizations",
    "workbench.preferredDarkColorTheme",
    "workbench.preferredLightColorTheme",
    "window.autoDetectColorScheme",
    "editor.tokenColorCustomizations",
    "editor.semanticTokenColorCustomizations",
    "editor.fontFamily",
    "editor.fontWeight",
    "editor.fontLigatures",
    "editor.lineHighlight*",
    "terminal.integrated.fontFamily",
    // Appearance extensions: Custom UI Style, APC, VS Code Animations.
    "apc.*",
    "animations.*",
    "custom-ui-style.*",
    "vscode_custom_css.*",
];
const ZED: &[&str] = &[
    "theme",
    "icon_theme",
    "ui_font_family",
    "ui_font_features",
    "ui_font_weight",
    "buffer_font_family",
    "buffer_font_features",
    "buffer_font_weight",
    "theme_overrides",
    "experimental.theme_overrides",
];

/// The policy for a tracked file (`$HOME`-relative path).
pub fn policy(rel: &str) -> Policy {
    if rel == crate::launcher_apps::FUZZEL {
        return Policy::Ini(crate::launcher_apps::FUZZEL_RULES);
    }
    if rel == crate::four_apps::FOOT {
        return Policy::Ini(FOOT);
    }
    if crate::native_appearance::handles(rel) {
        return Policy::Native;
    }
    if rel == ".config/opencode/cli.json" || rel == ".claude/settings.json" {
        return Policy::Jsonc(&["theme"]);
    }

    if crate::browser_appearance::is_profile_file(rel) {
        if rel.ends_with("/user.js") {
            return Policy::Prefs(crate::browser_appearance::PREFS);
        }
        if rel.ends_with("/chrome/userChrome.css") {
            return Policy::BrowserHost;
        }
    }
    if rel.ends_with("/.obsidian/appearance.json")
        || (rel.starts_with(crate::obsidian::SNAPSHOT) && rel.ends_with("/appearance.json"))
    {
        return Policy::Jsonc(&["theme", "cssTheme", "enabledCssSnippets"]);
    }
    if rel.starts_with(".local/share/konsole/") && rel.ends_with(".profile") {
        return Policy::Ini(KONSOLE_PROFILE);
    }
    if crate::discord::is_settings(rel) {
        return Policy::Jsonc(crate::discord::SETTINGS_KEYS);
    }
    match rel {
        ".config/kdeglobals" => Policy::Ini(KDEGLOBALS),
        ".config/kwinrc" => Policy::KwinRc,
        ".config/kwinrulesrc" => Policy::KwinRules,
        ".config/plasmarc" => Policy::Ini(PLASMARC),
        ".config/kcminputrc" => Policy::Ini(KCMINPUTRC),
        crate::spicetify::CONFIG => Policy::Ini(SPICETIFY),
        crate::extras::CAVA => Policy::Ini(CAVA_RULES),
        crate::extras::FISH_VARS => Policy::Lines(crate::extras::fish_colour_line),
        crate::extras::BAT_CONFIG => Policy::Lines(crate::extras::bat_theme_line),
        crate::extras::GHOSTTY_HOST => Policy::GhosttyHost,
        rel if crate::extras::SHELL_HOSTS.contains(&rel) => Policy::OmpHost,
        ".config/plasmashellrc" => Policy::PanelViews,
        crate::wallpaper::APPLETSRC_REL => Policy::Appletsrc,
        ".config/gtk-3.0/settings.ini" | ".config/gtk-4.0/settings.ini" => {
            Policy::Ini(GTK_SETTINGS)
        }
        ".config/btop/btop.conf" => Policy::Kv(BTOP),
        ".config/Code/User/settings.json"
        | ".config/Code - OSS/User/settings.json"
        | ".config/Cursor/User/settings.json" => Policy::Jsonc(VSCODE),
        ".config/VSCodium/User/settings.json" => Policy::Jsonc(VSCODIUM_COLOURS),
        ".config/micro/settings.json" => Policy::Jsonc(&["colorscheme"]),
        ".local/share/PrismLauncher/prismlauncher.cfg" => Policy::Ini(PRISM),
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
pub(crate) struct Section {
    /// Raw header line (`[Colors:View]`), `None` for lines before any header.
    pub(crate) header: Option<String>,
    pub(crate) lines: Vec<String>,
}

impl Section {
    pub(crate) fn group(&self) -> &str {
        self.header
            .as_deref()
            .map(|h| {
                h.trim()
                    .strip_prefix('[')
                    .and_then(|h| h.strip_suffix(']'))
                    .unwrap_or(h)
            })
            .unwrap_or("")
    }
}

pub(crate) fn parse_ini(text: &str) -> Vec<Section> {
    let mut out = vec![Section::default()];
    for line in text.lines() {
        if line.starts_with('[') {
            out.push(Section {
                header: Some(line.to_string()),
                lines: Vec::new(),
            });
        } else if let Some(last) = out.last_mut() {
            last.lines.push(line.to_string());
        }
    }
    out
}

pub(crate) fn render_ini(sections: &[Section]) -> String {
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
pub(crate) fn base_key(line: &str) -> Option<&str> {
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
            let lines: Vec<String> = s
                .lines
                .iter()
                .filter(|l| allowed_line(rules, &group, l))
                .cloned()
                .collect();
            (!lines.is_empty()).then_some(Section {
                header: s.header,
                lines,
            })
        })
        .collect();
    render_ini(&sections)
}

/// Font keys whose value holds a size: KDE `family,pointSize,pixelSize,…`
/// and GTK `family[,] size`.
const FONT_KEYS: &[&str] = &[
    "font",
    "fixed",
    "smallestReadableFont",
    "toolBarFont",
    "menuFont",
    "activeFont",
    "Font",
    "gtk-font-name",
];

/// The theme's font with the live size: the family is the look, the size is
/// the user's (their screen). Without a live value the theme's stays.
pub fn keep_live_size(key: &str, theme_value: &str, live_value: Option<&str>) -> String {
    let Some(live) = live_value else {
        return theme_value.to_string();
    };
    if key == "gtk-font-name" {
        let split = |v: &str| -> (String, String) {
            let v = v.trim();
            match v.rsplit_once([' ', ',']) {
                Some((family, size)) if size.parse::<f32>().is_ok() => (
                    family.trim_end_matches([',', ' ']).to_string(),
                    size.to_string(),
                ),
                _ => (v.to_string(), String::new()),
            }
        };
        let (family, _) = split(theme_value);
        let (_, size) = split(live);
        return if size.is_empty() {
            family
        } else {
            format!("{family}, {size}")
        };
    }
    let mut theme: Vec<&str> = theme_value.split(',').collect();
    let live: Vec<&str> = live.split(',').collect();
    for i in [1, 2] {
        if let (Some(slot), Some(l)) = (theme.get_mut(i), live.get(i)) {
            *slot = l;
        }
    }
    theme.join(",")
}

// ─────────────────────────────────────────────────────────────────────────────
// Settings the look does not use
// ─────────────────────────────────────────────────────────────────────────────

/// Effects KWin keeps off unless `<id>Enabled=true` (no key = off).
const EFFECTS_OFF_BY_DEFAULT: &[&str] = &["translucency", "blurplus"];

fn ini_value(sections: &[Section], group: &str, key: &str) -> Option<String> {
    sections
        .iter()
        .filter(|s| s.header.is_some() && s.group() == group)
        .flat_map(|s| s.lines.iter())
        .find(|l| base_key(l) == Some(key))
        .and_then(|l| l.split_once('='))
        .map(|(_, v)| v.trim().to_string())
}

/// Does `group` of a kwinrc hold the settings of an effect that is off in
/// `sections`? Only clear cases: effects that are on by default (overview…)
/// or not listed stay.
fn effect_block_unused(group: &str, sections: &[Section]) -> bool {
    let (id, default_off) = match group.strip_prefix("Effect-") {
        Some(name) => {
            let id = name.replace('-', "_");
            let off = EFFECTS_OFF_BY_DEFAULT.contains(&id.as_str());
            (id, off)
        }
        None if group == "Round-Corners" => ("kwin4_effect_shapecorners".to_string(), false),
        None => return false,
    };
    match ini_value(sections, "Plugins", &format!("{id}Enabled")).as_deref() {
        Some("false") => true,
        Some(_) => false,
        None => default_off,
    }
}

fn kwinrc_filter(text: &str) -> String {
    let sections = parse_ini(text);
    let kept: Vec<Section> = parse_ini(&ini_filter(text, KWINRC))
        .into_iter()
        .filter(|s| s.header.is_some() && !effect_block_unused(s.group(), &sections))
        .collect();
    render_ini(&kept)
}

/// The theme's kwinrc look into the live one. The live settings of an effect
/// the theme keeps off are not the theme's business: they stay.
fn kwinrc_merge(theme: &str, live: &str) -> String {
    let theme_sections = parse_ini(theme);
    let keep: Vec<Section> = parse_ini(live)
        .into_iter()
        .filter(|s| s.header.is_some() && effect_block_unused(s.group(), &theme_sections))
        .collect();
    let mut merged = parse_ini(&ini_merge(&kwinrc_filter(theme), live, KWINRC));
    merged.extend(keep);
    render_ini(&merged)
}

/// Config files nothing in this theme reads: Breeze's and Klassy's settings
/// when neither its widget style nor its window decoration is that one
/// (absent keys mean KDE's default, Breeze).
pub fn unused_files(kdeglobals: &str, kwinrc: &str) -> Vec<&'static str> {
    let of = |text: &str, group: &str, key: &str, default: &str| {
        ini_value(&parse_ini(text), group, key)
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| default.to_string())
            .to_lowercase()
    };
    let style = of(kdeglobals, "KDE", "widgetStyle", "breeze");
    let library = of(kwinrc, "org.kde.kdecoration2", "library", "org.kde.breeze");
    let mut unused = Vec::new();
    if style != "breeze" && library != "org.kde.breeze" {
        unused.push(".config/breezerc");
    }
    if style != "klassy" && library != "org.kde.klassy" {
        unused.extend([".config/klassy/klassyrc", ".config/klassy/windecopresetsrc"]);
    }
    unused
}

/// [`unused_files`] of theme folder `dir`; nothing when it cannot tell (a
/// partial snapshot without `kdeglobals` or `kwinrc`).
pub fn unused_files_in(dir: &std::path::Path) -> Vec<&'static str> {
    match (
        std::fs::read_to_string(dir.join(".config/kdeglobals")),
        std::fs::read_to_string(dir.join(".config/kwinrc")),
    ) {
        (Ok(kdeglobals), Ok(kwinrc)) => unused_files(&kdeglobals, &kwinrc),
        _ => Vec::new(),
    }
}

/// `live` with its appearance keys replaced by the theme's: allowed keys the
/// theme lacks are removed, every other key is kept as it is; fonts keep the
/// live size.
pub fn ini_merge(theme: &str, live: &str, rules: &[Rule]) -> String {
    let theme = parse_ini(&ini_filter(theme, rules));
    let mut live = parse_ini(live);
    let live_font = |sections: &[Section], group: &str, key: &str| -> Option<String> {
        sections
            .iter()
            .filter(|s| s.group() == group)
            .flat_map(|s| s.lines.iter())
            .find_map(|l| {
                (base_key(l) == Some(key))
                    .then(|| l.split_once('=').map(|(_, v)| v.to_string()))
                    .flatten()
            })
    };
    let theme: Vec<Section> = theme
        .into_iter()
        .map(|mut t| {
            let group = t.group().to_string();
            for line in t.lines.iter_mut() {
                if let (Some(key), Some((k, v))) = (
                    base_key(line).map(str::to_string),
                    line.clone().split_once('='),
                ) {
                    if FONT_KEYS.contains(&key.as_str()) {
                        *line = format!(
                            "{k}={}",
                            keep_live_size(&key, v, live_font(&live, &group, &key).as_deref())
                        );
                    }
                }
            }
            t
        })
        .collect();
    for s in live.iter_mut().filter(|s| s.header.is_some()) {
        let group = s.group().to_string();
        s.lines.retain(|l| !allowed_line(rules, &group, l));
    }
    for t in theme.into_iter().filter(|s| s.header.is_some()) {
        let group = t.group().to_string();
        match live
            .iter_mut()
            .find(|s| s.header.is_some() && s.group() == group)
        {
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
        sections
            .iter()
            .filter(|s| s.header.is_some() && s.group() != "General")
            .map(|s| s.group().to_string())
            .collect()
    };
    let mut order = ids(&live);
    for id in ids(&theme) {
        if !order.contains(&id) {
            order.push(id);
        }
    }
    for t in theme
        .into_iter()
        .filter(|s| s.header.is_some() && s.group() != "General")
    {
        match live
            .iter_mut()
            .find(|s| s.header.is_some() && s.group() == t.group())
        {
            Some(s) => s.lines = crate::layout::kwinrule_lines(&t.lines, &s.lines),
            None => live.push(t),
        }
    }
    let general = match live
        .iter_mut()
        .find(|s| s.group() == "General" && s.header.is_some())
    {
        Some(g) => g,
        None => {
            live.insert(
                1.min(live.len()),
                Section {
                    header: Some("[General]".into()),
                    lines: Vec::new(),
                },
            );
            live.iter_mut()
                .find(|s| s.group() == "General" && s.header.is_some())
                .expect("just inserted")
        }
    };
    let list = crate::layout::rule_list_lines(&order, &general.lines);
    general
        .lines
        .retain(|l| !matches!(base_key(l), Some("count" | "rules" | "Order")));
    for (i, line) in list.into_iter().enumerate() {
        general.lines.insert(i, line);
    }
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
    let mut out: String = text
        .lines()
        .filter(|l| kv_allowed(keys, l))
        .collect::<Vec<_>>()
        .join("\n");
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
// user.js prefs (Firefox)
// ─────────────────────────────────────────────────────────────────────────────

fn pref_name(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix("user_pref(")?.trim_start();
    let rest = rest.strip_prefix('"')?;
    Some(&rest[..rest.find('"')?])
}

fn pref_allowed(keys: &[&str], line: &str) -> bool {
    pref_name(line).is_some_and(|n| keys.iter().any(|p| pattern_matches(p, n)))
}

pub fn prefs_filter(text: &str, keys: &[&str]) -> String {
    let mut out = text
        .lines()
        .filter(|l| pref_allowed(keys, l))
        .collect::<Vec<_>>()
        .join("\n");
    out.push('\n');
    out
}

/// Allowed prefs replaced in place, the theme's new ones appended, allowed
/// ones the theme lacks removed; every other line kept.
pub fn prefs_merge(theme: &str, live: &str, keys: &[&str]) -> String {
    let theme_lines: Vec<&str> = theme.lines().filter(|l| pref_allowed(keys, l)).collect();
    let mut used = vec![false; theme_lines.len()];
    let mut out: Vec<String> = Vec::new();
    for line in live.lines() {
        if !pref_allowed(keys, line) {
            out.push(line.to_string());
            continue;
        }
        if let Some(i) = theme_lines
            .iter()
            .position(|t| pref_name(t) == pref_name(line))
        {
            if !used[i] {
                out.push(theme_lines[i].to_string());
                used[i] = true;
            }
        }
    }
    out.extend(
        theme_lines
            .iter()
            .zip(&used)
            .filter(|(_, u)| !**u)
            .map(|(l, _)| l.to_string()),
    );
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
            while j < b.len()
                && !matches!(b[j], b',' | b'}' | b']' | b'\n' | b'\r')
                && !(b[j] == b'/' && matches!(b.get(j + 1), Some(b'/') | Some(b'*')))
            {
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
        out.push(Member {
            key,
            start,
            value: (vstart, vend),
        });
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
        .map(|m| {
            format!(
                "  {}: {}",
                serde_json::to_string(&m.key).unwrap_or_default(),
                &text[m.value.0..m.value.1]
            )
        })
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
    for m in live_members
        .into_iter()
        .filter(|m| json_allowed(keys, &m.key))
    {
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
    let missing: Vec<&(String, String)> = theme_members
        .iter()
        .filter(|(k, _)| !done.contains(k))
        .collect();
    if !missing.is_empty() {
        if let Some(close) = text.rfind('}') {
            let body = text[..close].trim_end().to_string();
            let mut add = String::new();
            let mut sep = if body.ends_with('{') || body.ends_with(',') {
                ""
            } else {
                ","
            };
            for (k, v) in missing {
                add.push_str(&format!(
                    "{sep}\n  {}: {v}",
                    serde_json::to_string(k).unwrap_or_default()
                ));
                sep = ",";
            }
            text = format!("{body}{add}\n{}", &text[close..]);
        }
    }
    text
}

/// Reverse only values still equal to what Apply wrote. An unrelated user edit
/// must not prevent restoring the old appearance, nor be overwritten itself.
pub(crate) fn restore_changed(
    rel: &str,
    before: &str,
    after: &str,
    current: &str,
) -> Result<String> {
    if current == after {
        return Ok(before.into());
    }
    if rel == crate::launcher_apps::FUZZEL {
        return Ok(crate::launcher_apps::restore_fuzzel(before, after, current));
    }
    if crate::launcher_apps::is_host(rel) {
        return crate::launcher_apps::restore_host(before, after, current);
    }
    if crate::four_apps::is_host(rel) {
        return crate::four_apps::restore_host(rel, before, after, current);
    }
    if rel.ends_with("/extensions.json") && rel.contains("/extensions/") {
        let parse = |s: &str| {
            if s.trim().is_empty() {
                Some(Vec::<serde_json::Value>::new())
            } else {
                serde_json::from_str::<Vec<serde_json::Value>>(s).ok()
            }
        };
        if let (Some(old), Some(written), Some(mut live)) =
            (parse(before), parse(after), parse(current))
        {
            for added in written.iter().filter(|v| !old.contains(v)) {
                live.retain(|v| v != added);
            }
            for removed in old.iter().filter(|v| !written.contains(v)) {
                if !live
                    .iter()
                    .any(|v| v["relativeLocation"] == removed["relativeLocation"])
                {
                    live.push(removed.clone());
                }
            }
            return Ok(serde_json::to_string_pretty(&live)?);
        }
        return Ok(current.into());
    }
    if before == after {
        return Ok(current.into());
    }
    Ok(match policy(rel) {
        Policy::Native => crate::native_appearance::restore(rel, before, after, current)?,
        Policy::Jsonc(_) => restore_json(before, after, current),
        Policy::Ini(_)
        | Policy::KwinRc
        | Policy::KwinRules
        | Policy::Appletsrc
        | Policy::PanelViews => restore_ini(before, after, current),
        Policy::Whole if rel == crate::terminal_apps::WOFI_HOST => {
            const IMPORT: &str = "@import url(\"kolira.css\");";
            if !before.lines().any(|l| l.trim() == IMPORT)
                && after.lines().any(|l| l.trim() == IMPORT)
            {
                current
                    .split_inclusive('\n')
                    .filter(|l| l.trim() != IMPORT)
                    .collect()
            } else {
                current.into()
            }
        }
        Policy::Whole => current.to_string(),
        _ => {
            let restored = inverse_lines(before, after, current);
            if rel == crate::extras::FISH_VARS
                && restored.lines().any(|l| l.starts_with("SETUVAR "))
            {
                crate::extras::fish_header(&restored)?
            } else {
                restored
            }
        }
    })
}

fn json_values(text: &str) -> std::collections::BTreeMap<String, String> {
    members(text)
        .into_iter()
        .map(|m| (m.key, text[m.value.0..m.value.1].to_string()))
        .collect()
}
fn json_equal(a: &str, b: &str) -> bool {
    match (
        serde_json::from_str::<serde_json::Value>(a),
        serde_json::from_str::<serde_json::Value>(b),
    ) {
        (Ok(a), Ok(b)) => a == b,
        _ => a.trim() == b.trim(),
    }
}
pub(crate) fn restore_json(before: &str, after: &str, current: &str) -> String {
    let (old, written, live) = (
        json_values(before),
        json_values(after),
        json_values(current),
    );
    let keys: std::collections::BTreeSet<_> = old.keys().chain(written.keys()).collect();
    let mut text = current.to_string();
    for key in keys {
        let (a, b, c) = (old.get(key), written.get(key), live.get(key));
        if a.zip(b).is_some_and(|(a, b)| json_equal(a, b)) || a.is_none() && b.is_none() {
            continue;
        }
        let value = if c.zip(b).is_some_and(|(c, b)| json_equal(c, b)) || c.is_none() && b.is_none()
        {
            Some(a.cloned())
        } else if let (Some(b), Some(c)) = (b, c) {
            if b.trim_start().starts_with('{') && c.trim_start().starts_with('{') {
                Some(Some(restore_json(
                    a.map(String::as_str).unwrap_or("{}"),
                    b,
                    c,
                )))
            } else if let (
                Ok(serde_json::Value::Array(written)),
                Ok(serde_json::Value::Array(mut current)),
            ) = (
                serde_json::from_str::<serde_json::Value>(b),
                serde_json::from_str::<serde_json::Value>(c),
            ) {
                let old = a
                    .and_then(|a| serde_json::from_str::<serde_json::Value>(a).ok())
                    .and_then(|v| v.as_array().cloned())
                    .unwrap_or_default();
                current.retain(|value| old.contains(value) || !written.contains(value));
                for (index, value) in old.iter().enumerate() {
                    if !written.contains(value) && !current.contains(value) {
                        current.insert(index.min(current.len()), value.clone());
                    }
                }
                Some(Some(serde_json::to_string(&current).unwrap()))
            } else {
                None
            }
        } else {
            None
        };
        if let Some(value) = value {
            let snippet = match value {
                Some(value) => format!("{{{}:{value}}}", serde_json::to_string(key).unwrap()),
                None => "{}".into(),
            };
            text = jsonc_merge(&snippet, &text, &[key.as_str()]);
        }
    }
    text
}

fn ini_values(text: &str) -> std::collections::BTreeMap<(String, String), String> {
    let mut values = std::collections::BTreeMap::new();
    for section in parse_ini(text) {
        for line in &section.lines {
            if let Some(key) = base_key(line) {
                values.insert((section.group().into(), key.into()), line.clone());
            }
        }
    }
    values
}
fn ini_line_value(line: &str) -> &str {
    line.split_once('=').map(|(_, v)| v.trim()).unwrap_or(line)
}
pub(crate) fn restore_ini(before: &str, after: &str, current: &str) -> String {
    let (old, written, live) = (ini_values(before), ini_values(after), ini_values(current));
    let keys: std::collections::BTreeSet<_> = old.keys().chain(written.keys()).collect();
    let mut sections = parse_ini(current);
    for key in keys {
        let (a, b, c) = (old.get(key), written.get(key), live.get(key));
        if c.zip(b).is_some_and(|(current, written)| {
            current.split_once('=').map(|(k, _)| k.trim())
                != written.split_once('=').map(|(k, _)| k.trim())
        }) {
            continue;
        }
        let mut original = a.cloned();
        let unchanged = a.map(|s| s.trim()) == b.map(|s| s.trim());
        if unchanged {
            continue;
        }
        if c.map(|s| ini_line_value(s)) != b.map(|s| ini_line_value(s)) {
            if let (Some(a), Some(b), Some(c)) = (a, b, c) {
                if FONT_KEYS.contains(&key.1.as_str())
                    && keep_live_size(&key.1, ini_line_value(b), Some(ini_line_value(c)))
                        == ini_line_value(c)
                {
                    original = Some(format!(
                        "{}={}",
                        a.split_once('=').unwrap().0,
                        keep_live_size(&key.1, ini_line_value(a), Some(ini_line_value(c)))
                    ));
                } else {
                    continue;
                }
            } else {
                continue;
            }
        }
        let a = original.as_ref();
        if let Some(section) = sections.iter_mut().rev().find(|s| s.group() == key.0) {
            let position = section
                .lines
                .iter()
                .position(|line| base_key(line) == Some(key.1.as_str()));
            section
                .lines
                .retain(|line| base_key(line) != Some(key.1.as_str()));
            if let Some(line) = a {
                section.lines.insert(
                    position
                        .unwrap_or(section.lines.len())
                        .min(section.lines.len()),
                    line.clone(),
                );
            }
        } else if let Some(line) = a {
            sections.push(Section {
                header: (!key.0.is_empty()).then(|| format!("[{}]", key.0)),
                lines: vec![line.clone()],
            });
        }
    }
    let original_groups: std::collections::BTreeSet<_> = parse_ini(before)
        .into_iter()
        .map(|s| s.group().to_string())
        .collect();
    sections.retain(|s| {
        s.header.is_none()
            || original_groups.contains(s.group())
            || s.lines.iter().any(|l| !l.trim().is_empty())
    });
    render_ini(&sections)
}

/// Invert exact line hunks in host configs without reverting unrelated edits.
/// Ambiguous/edited hunks are left alone. Bound the LCS work for unusually large
/// manually edited files; unchanged files always restore byte for byte above.
fn inverse_lines(before: &str, after: &str, current: &str) -> String {
    let a: Vec<_> = before.split_inclusive('\n').collect();
    let b: Vec<_> = after.split_inclusive('\n').collect();
    if (a.len() + 1).saturating_mul(b.len() + 1) > 4_000_000 {
        return current.into();
    }
    let stride = b.len() + 1;
    let mut lcs = vec![0u32; (a.len() + 1) * stride];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i * stride + j] = if a[i] == b[j] {
                1 + lcs[(i + 1) * stride + j + 1]
            } else {
                lcs[(i + 1) * stride + j].max(lcs[i * stride + j + 1])
            };
        }
    }
    let mut hunks = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            i += 1;
            j += 1;
            continue;
        }
        let (start_i, start_j) = (i, j);
        while i < a.len() || j < b.len() {
            if i < a.len() && j < b.len() && a[i] == b[j] {
                break;
            }
            if i < a.len() && (j == b.len() || lcs[(i + 1) * stride + j] >= lcs[i * stride + j + 1])
            {
                i += 1;
            } else {
                j += 1;
            }
        }
        hunks.push((start_i, i, start_j, j));
    }
    let mut live: Vec<&str> = current.split_inclusive('\n').collect();
    for (ai, az, bi, bz) in hunks.into_iter().rev() {
        let matches: Vec<usize> = (0..=live.len())
            .filter(|&position| {
                if bi != bz {
                    return live.get(position..position + (bz - bi)) == Some(&b[bi..bz]);
                }
                let left = bi == 0 && position == 0
                    || bi > 0 && position > 0 && live[position - 1] == b[bi - 1];
                let right = bz == b.len() && position == live.len()
                    || bz < b.len() && live.get(position) == Some(&b[bz]);
                left && right
            })
            .collect();
        if matches.len() == 1 {
            live.splice(
                matches[0]..matches[0] + (bz - bi),
                a[ai..az].iter().copied(),
            );
        }
    }
    live.concat()
}

// ─────────────────────────────────────────────────────────────────────────────
// kitty: appearance in kitty/kolira.conf, included by kitty.conf
// ─────────────────────────────────────────────────────────────────────────────

const KITTY_LOOK: &[&str] = &[
    "font_family",
    "bold_font",
    "italic_font",
    "bold_italic_font",
    "font_features",
    "disable_ligatures",
    "symbol_map",
    "narrow_symbols",
    "text_composition_strategy",
    "foreground",
    "background",
    "background_*",
    "selection_*",
    "cursor",
    "cursor_*",
    "url_color",
    "url_style",
    "mark1_*",
    "mark2_*",
    "mark3_*",
    "active_border_color",
    "inactive_border_color",
    "bell_border_color",
    "window_border_width",
    "window_margin_width",
    "window_padding_width",
    "single_window_*",
    "draw_minimal_borders",
    "hide_window_decorations",
    "tab_bar_*",
    "tab_powerline_style",
    "tab_separator",
    "tab_activity_symbol",
    "active_tab_*",
    "inactive_tab_*",
    "dim_opacity",
    "transparent_background_colors",
    "wayland_titlebar_color",
    "undercurl_style",
];

fn kitty_look(line: &str) -> bool {
    let t = line.trim_start();
    let Some(word) = t.split_whitespace().next() else {
        return false;
    };
    if word.starts_with('#') {
        return false;
    }
    if let Some(n) = word.strip_prefix("color") {
        return !n.is_empty() && n.chars().all(|c| c.is_ascii_digit());
    }
    KITTY_LOOK.iter().any(|p| pattern_matches(p, word))
}

/// Split a `kitty.conf` into (appearance lines incl. the generated palette
/// block, the rest without the `include kolira.conf` line).
pub fn kitty_split(host: &str) -> (String, String) {
    let mut look: Vec<&str> = Vec::new();
    let mut rest: Vec<&str> = Vec::new();
    let mut in_palette = false;
    for line in host.lines() {
        if line.starts_with("# BEGIN kolira palette") {
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

const KITTY_NOTE: &str = "# Theme (kolira): appearance lives in kolira.conf";

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
// Alacritty: appearance in alacritty/kolira.toml, imported by alacritty.toml
// ─────────────────────────────────────────────────────────────────────────────

const ALACRITTY_WINDOW: &[&str] = &[
    "opacity",
    "blur",
    "padding",
    "dynamic_padding",
    "decorations",
    "decorations_theme_variant",
];
const ALACRITTY_CURSOR: &[&str] = &["style", "unfocused_hollow", "thickness"];

/// Split `alacritty.toml` into (appearance TOML, the rest with the import).
pub fn alacritty_split(host: &str) -> Result<(String, String)> {
    let mut doc: DocumentMut = host.parse().context("alacritty.toml is not valid TOML")?;
    let mut look = DocumentMut::new();
    if let Some(item) = doc.remove("colors") {
        look["colors"] = item;
    }
    // [font]: the faces travel, size/offsets stay (screens differ).
    if let Some(font) = doc.get_mut("font").and_then(Item::as_table_like_mut) {
        for face in [
            "normal",
            "bold",
            "italic",
            "bold_italic",
            "builtin_box_drawing",
        ] {
            if let Some(item) = font.remove(face) {
                if look.get("font").is_none() {
                    look["font"] = Item::Table(Table::new());
                }
                look["font"][face] = item;
            }
        }
        if font.is_empty() {
            doc.remove("font");
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

fn non_empty(text: String) -> Option<String> {
    (!text.trim().is_empty()).then_some(text)
}

/// What a theme stores for tracked file `rel` given its current text; `None`
/// means "store nothing" (include hosts).
pub fn for_theme(rel: &str, text: &str) -> Option<String> {
    if is_preserved(rel) {
        return None;
    }
    if rel == crate::four_apps::STARSHIP {
        return crate::four_apps::starship_capture(text)
            .ok()
            .and_then(non_empty);
    }
    if rel == crate::four_apps::WEZ_DATA {
        return crate::four_apps::valid_wez(text)
            .ok()
            .map(|_| text.to_string());
    }
    if rel == crate::four_apps::TMUX_DATA {
        return crate::tmux_runtime::parse(text)
            .ok()
            .map(|_| text.to_string());
    }
    if rel == crate::launcher_apps::FUZZEL {
        return crate::launcher_apps::fuzzel_source(text).ok();
    }
    if crate::launcher_apps::is_asset(rel) {
        return crate::launcher_apps::valid_asset(rel, text).ok();
    }
    if crate::third_apps::is_asset(rel) {
        return crate::third_apps::valid_asset(rel, text).ok();
    }
    match policy(rel) {
        Policy::Whole if crate::browser_appearance::is_colors(rel) => {
            crate::browser_appearance::safe_colors(text).ok()
        }
        Policy::Native => crate::native_appearance::capture(rel, text).ok(),
        Policy::Whole if rel == crate::terminal_apps::WOFI_COLORS => {
            crate::terminal_apps::valid_wofi(text).ok()
        }
        Policy::Whole if rel == crate::terminal_apps::FZF_DATA => {
            crate::terminal_apps::valid_fzf(text).ok()
        }
        Policy::Whole => Some(text.to_string()),
        Policy::KwinRc => Some(kwinrc_filter(text)),
        Policy::KwinRules => non_empty(crate::layout::kwinrules_for_theme(text)),
        Policy::Appletsrc => non_empty(crate::layout::appletsrc_for_theme(text)),
        Policy::PanelViews => non_empty(crate::layout::panel_views_for_theme(text)),
        Policy::Ini(rules) => Some(ini_filter(text, rules)),
        Policy::Kv(keys) => Some(kv_filter(text, keys)),
        Policy::Jsonc(keys) => Some(jsonc_filter(text, keys)),
        Policy::Prefs(keys) => Some(prefs_filter(text, keys)),
        Policy::Lines(keep) => Some(crate::extras::lines_filter(text, keep)),
        Policy::BrowserHost
        | Policy::KittyHost
        | Policy::AlacrittyHost
        | Policy::OmpHost
        | Policy::GhosttyHost => None,
    }
}

/// The new live text of `rel` when applying `theme_text` over `live_text`.
pub fn applied(rel: &str, theme_text: &str, live_text: Option<&str>) -> Result<String> {
    if rel == crate::four_apps::FOOT {
        return crate::four_apps::foot_merge(theme_text, live_text.unwrap_or(""));
    }
    if rel == crate::four_apps::WEZ_DATA {
        crate::four_apps::valid_wez(theme_text)?;
    }
    if rel == crate::four_apps::TMUX_DATA {
        crate::tmux_runtime::parse(theme_text)?;
    }
    if is_preserved(rel) {
        return Ok(live_text.unwrap_or("").to_string());
    }
    if crate::launcher_apps::is_asset(rel) {
        return crate::launcher_apps::valid_asset(rel, theme_text);
    }
    if rel == crate::launcher_apps::FUZZEL {
        return crate::launcher_apps::fuzzel_merge(theme_text, live_text.unwrap_or(""));
    }
    if crate::third_apps::is_asset(rel) {
        return crate::third_apps::valid_asset(rel, theme_text);
    }
    let live = live_text.unwrap_or("");
    if matches!(rel, ".claude/settings.json" | ".config/opencode/cli.json") {
        let source: serde_json::Value =
            serde_json::from_str(theme_text).context("Invalid native AI appearance JSON")?;
        anyhow::ensure!(source.is_object(), "AI appearance JSON must be an object");
        if let Some(theme) = source.get("theme") {
            if rel == ".claude/settings.json" {
                anyhow::ensure!(theme.is_string(), "Claude theme must be a native string");
            } else {
                anyhow::ensure!(
                    theme.is_object()
                        && theme.get("name").is_some_and(serde_json::Value::is_string),
                    "OpenCode V2 theme must be a native object with name"
                );
                if let Some(mode) = theme.get("mode") {
                    anyhow::ensure!(
                        matches!(mode.as_str(), Some("dark" | "light" | "auto")),
                        "Invalid OpenCode theme mode"
                    );
                }
            }
        }
    }
    Ok(match policy(rel) {
        Policy::Whole if crate::browser_appearance::is_colors(rel) => {
            crate::browser_appearance::safe_colors(theme_text)?
        }
        Policy::Native => crate::native_appearance::merge(rel, theme_text, live)?,
        Policy::Whole if rel == crate::terminal_apps::WOFI_COLORS => {
            crate::terminal_apps::valid_wofi(theme_text)?
        }
        Policy::Whole if rel == crate::terminal_apps::FZF_DATA => {
            crate::terminal_apps::valid_fzf(theme_text)?
        }
        Policy::Whole => theme_text.to_string(),
        Policy::KwinRc => kwinrc_merge(theme_text, live),
        Policy::Ini(rules) => ini_merge(theme_text, live, rules),
        Policy::KwinRules => kwinrules_merge(&crate::layout::kwinrules_for_theme(theme_text), live),
        Policy::Appletsrc => crate::layout::appletsrc_merge(theme_text, live),
        Policy::PanelViews => crate::layout::panel_views_merge(theme_text, live, None, None),
        Policy::Kv(keys) => kv_merge(theme_text, live, keys),
        Policy::Jsonc(keys) if rel == ".claude/settings.json" => {
            let current = crate::field::json_get(live, &["theme".into()]);
            let incoming = crate::field::json_get(theme_text, &["theme".into()]);
            let adjusted = if current
                .and_then(|v| v.as_str().map(str::to_owned))
                .is_some_and(|s| s.ends_with("-daltonized"))
            {
                if let Some(mode) = incoming
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .and_then(|s| {
                        if s.starts_with("dark") {
                            Some("dark")
                        } else if s.starts_with("light") {
                            Some("light")
                        } else {
                            None
                        }
                    })
                {
                    crate::field::json_set(
                        theme_text,
                        &["theme".into()],
                        &serde_json::json!(format!("{mode}-daltonized")),
                    )?
                } else {
                    theme_text.into()
                }
            } else {
                theme_text.into()
            };
            jsonc_merge(&adjusted, live, keys)
        }
        Policy::Jsonc(keys) => jsonc_merge(theme_text, live, keys),
        Policy::Prefs(keys) => {
            let filtered = theme_text
                .lines()
                .filter(|line| {
                    !line.contains("toolkit.legacyUserProfileCustomizations.stylesheets")
                })
                .collect::<Vec<_>>()
                .join("\n");
            let keys: Vec<_> = keys
                .iter()
                .copied()
                .filter(|k| *k != "toolkit.legacyUserProfileCustomizations.stylesheets")
                .collect();
            let merged = prefs_merge(&filtered, live, &keys);
            let key = if crate::browser_appearance::is_zen(rel) {
                "zen.view.window.scheme"
            } else {
                "browser.theme.toolbar-theme"
            };
            match crate::field::userjs_get(theme_text, key).and_then(|v| v.as_u64()) {
                Some(mode @ 0..=1) => crate::browser_appearance::mode(
                    &merged,
                    mode == 0,
                    crate::browser_appearance::is_zen(rel),
                ),
                _ => merged,
            }
        }
        Policy::BrowserHost => live.to_string(),
        Policy::KittyHost => kitty_host(live),
        Policy::OmpHost => crate::extras::omp_host(live),
        Policy::GhosttyHost => crate::extras::ghostty_host(live),
        Policy::Lines(keep) => {
            let merged = crate::extras::lines_merge(theme_text, live, keep);
            if rel == crate::extras::FISH_VARS {
                crate::extras::fish_header(&merged)?
            } else {
                merged
            }
        }
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

/// The theme file a host config includes (kitty, Alacritty, ghostty).
pub fn include_of(host: &str) -> Option<&'static str> {
    match host {
        KITTY_HOST => Some(KITTY_THEME),
        ALACRITTY_HOST => Some(ALACRITTY_THEME),
        crate::extras::GHOSTTY_HOST => Some(crate::extras::GHOSTTY_THEME),
        _ => None,
    }
}

/// A stored theme file as it should be: the look only, include hosts turned
/// into their include file. Returns the entries to keep (`rel`, text) — empty
/// when nothing of the file belongs in a theme. `has_include` tells whether
/// the theme already carries the host's include file.
pub fn normalize(rel: &str, text: &str, has_include: bool) -> Result<Vec<(String, String)>> {
    if is_preserved(rel) {
        return Ok(Vec::new());
    }
    if crate::launcher_apps::is_asset(rel) {
        return Ok(vec![(
            rel.into(),
            crate::launcher_apps::valid_asset(rel, text)?,
        )]);
    }
    if rel == crate::launcher_apps::FUZZEL {
        return Ok(vec![(
            rel.into(),
            crate::launcher_apps::fuzzel_source(text)?,
        )]);
    }
    if crate::third_apps::is_asset(rel) {
        return Ok(vec![(
            rel.into(),
            crate::third_apps::valid_asset(rel, text)?,
        )]);
    }
    Ok(match policy(rel) {
        Policy::Whole if crate::browser_appearance::is_colors(rel) => {
            vec![(rel.into(), crate::browser_appearance::safe_colors(text)?)]
        }
        Policy::Native => vec![(rel.into(), crate::native_appearance::capture(rel, text)?)],
        Policy::Whole if rel == crate::terminal_apps::WOFI_COLORS => {
            vec![(rel.into(), crate::terminal_apps::valid_wofi(text)?)]
        }
        Policy::Whole if rel == crate::terminal_apps::FZF_DATA => {
            vec![(rel.into(), crate::terminal_apps::valid_fzf(text)?)]
        }
        Policy::Whole => vec![(rel.to_string(), text.to_string())],
        Policy::OmpHost | Policy::BrowserHost => Vec::new(),
        Policy::GhosttyHost => {
            let (look, _) = crate::extras::ghostty_split(text);
            if look.trim().is_empty() || has_include {
                Vec::new()
            } else {
                vec![(crate::extras::GHOSTTY_THEME.to_string(), look)]
            }
        }
        Policy::Lines(keep) => vec![(rel.to_string(), crate::extras::lines_filter(text, keep))],
        Policy::KittyHost | Policy::AlacrittyHost => match include_from_host(rel, text)? {
            Some((inc, look)) if !has_include => vec![(inc.to_string(), look)],
            _ => Vec::new(),
        },
        _ => for_theme(rel, text)
            .map(|t| vec![(rel.to_string(), t)])
            .unwrap_or_default(),
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
        assert!(
            f.contains("ColorScheme=Dracula")
                && f.contains("[Colors:Window]")
                && f.contains("widgetStyle=kvantum")
        );
        assert!(!f.contains("Terminal") && !f.contains("KScreen") && !f.contains("Scale"));
    }

    #[test]
    fn ini_merge_keeps_personal_keys_and_replaces_the_look() {
        let m = ini_merge(KDEGLOBALS_THEME, KDEGLOBALS_LIVE, KDEGLOBALS);
        // Personal: untouched.
        assert!(
            m.contains("TerminalApplication=kitty")
                && m.contains("ScreenScaleFactors=eDP-2=1.25;")
                && m.contains("SingleClick=false")
        );
        // Look: from the theme; the theme's absent accent / colour group are removed.
        assert!(
            m.contains("ColorScheme=Dracula")
                && m.contains("widgetStyle=kvantum")
                && m.contains("[Colors:Window]")
        );
        assert!(
            !m.contains("AccentColor=1,2,3")
                && !m.contains("[Colors:View]")
                && !m.contains("ColorScheme=Mine")
        );
        // Idempotent.
        assert_eq!(ini_merge(KDEGLOBALS_THEME, &m, KDEGLOBALS), m);
    }

    #[test]
    fn font_sizes_stay_the_users() {
        // KDE: family from the theme, point/pixel size from the live file.
        assert_eq!(
            keep_live_size(
                "font",
                "Inter,10,-1,5,50,0,0,0,0,0",
                Some("Noto Sans,13,-1,5,50,0,0,0,0,0")
            ),
            "Inter,13,-1,5,50,0,0,0,0,0"
        );
        assert_eq!(
            keep_live_size("gtk-font-name", "Inter, 10", Some("Noto Sans,  13")),
            "Inter, 13"
        );
        assert_eq!(keep_live_size("font", "Inter,10", None), "Inter,10");
        let m = ini_merge(
            "[General]\nfont=Inter,10,-1,5,50,0,0,0,0,0\n",
            "[General]\nfont=Noto Sans,14,-1,5,50,0,0,0,0,0\n",
            KDEGLOBALS,
        );
        assert!(m.contains("font=Inter,14,"), "{m}");
        // Sizes never stored or moved.
        assert!(
            !jsonc_filter("{\"ui_font_size\": 16, \"theme\": \"x\"}", ZED).contains("ui_font_size")
        );
        let (look, rest) = kitty_split("font_family Inter\nfont_size 13\n");
        assert!(
            look.contains("font_family")
                && !look.contains("font_size")
                && rest.contains("font_size 13")
        );
        let (look, _) =
            alacritty_split("[font]\nsize = 13\n[font.normal]\nfamily = \"Inter\"\n").unwrap();
        assert!(look.contains("Inter") && !look.contains("size"));
    }

    #[test]
    fn firefox_user_js_keeps_privacy_prefs() {
        let rel = ".mozilla/firefox/ab.default/user.js";
        assert!(matches!(policy(rel), Policy::Prefs(_)));
        let live = "// betterfox\nuser_pref(\"privacy.resistFingerprinting\", true);\nuser_pref(\"browser.theme.toolbar-theme\", 1);\nuser_pref(\"extensions.activeThemeID\", \"old@x\");\n";
        let theme = "user_pref(\"browser.theme.toolbar-theme\", 0);\nuser_pref(\"toolkit.legacyUserProfileCustomizations.stylesheets\", true);\nuser_pref(\"privacy.resistFingerprinting\", false);\n";
        let m = applied(rel, theme, Some(live)).unwrap();
        assert!(m.contains("// betterfox") && m.contains("\"privacy.resistFingerprinting\", true"));
        assert!(
            m.contains("\"browser.theme.toolbar-theme\", 0")
                && !m.contains("legacyUserProfileCustomizations")
        );
        assert!(m.contains("old@x"), "browser add-on selection is personal");
        assert!(!for_theme(rel, live).unwrap().contains("privacy"));
    }

    #[test]
    fn kwin_scripts_are_not_effects() {
        let rule = Rule {
            group: "Plugins",
            keys: Keys::KwinEffects,
        };
        assert!(key_allowed(&rule, "blurEnabled"));
        assert!(!key_allowed(&rule, "blurStrength"));
    }

    #[test]
    fn kwinrules_merge_keeps_users_rules() {
        let live = "[General]\ncount=1\nrules=a\n\n[a]\nDescription=mine\n";
        let theme = "[General]\ncount=1\nrules=b\n\n[b]\nDescription=theme\nopacityactive=90\n";
        let m = kwinrules_merge(theme, live);
        assert!(
            m.contains("count=2")
                && m.contains("rules=a,b")
                && m.contains("Description=mine")
                && m.contains("opacityactive=90")
        );
        assert!(!m.contains("Order="));
    }

    #[test]
    fn kwinrc_drops_settings_of_effects_that_are_off() {
        let theme = "[Effect-blur]\nBlurStrength=10\n\n[Effect-better-blur-dx]\nBlurStrength=5\n\n[Effect-blurplus]\nBlurStrength=14\n\n[Effect-translucency]\nMenus=96\n\n[Effect-overview]\nBorderActivate=9\n\n[Round-Corners]\nSize=10\n\n[Plugins]\nbetter_blur_dxEnabled=true\nblurEnabled=false\nkwin4_effect_shapecornersEnabled=false\n";
        let t = kwinrc_filter(theme);
        for gone in [
            "[Effect-blur]",
            "[Effect-blurplus]",
            "[Effect-translucency]",
            "[Round-Corners]",
        ] {
            assert!(!t.contains(gone), "{gone} stored:\n{t}");
        }
        assert!(
            t.contains("[Effect-better-blur-dx]")
                && t.contains("[Effect-overview]")
                && t.contains("blurEnabled=false"),
            "{t}"
        );
        assert_eq!(kwinrc_filter(&t), t);

        // On in the theme, the theme's settings count.
        let on = "[Effect-translucency]\nMenus=96\n\n[Plugins]\ntranslucencyEnabled=true\n";
        assert!(kwinrc_filter(on).contains("Menus=96"));

        // The user's settings of an effect the theme keeps off stay; on ones follow the theme.
        let live = "[Effect-blur]\nBlurStrength=3\n\n[Effect-better-blur-dx]\nBlurStrength=9\n\n[Plugins]\nblurEnabled=true\n\n[Windows]\nFocusPolicy=mine\n";
        let m = kwinrc_merge(&t, live);
        assert!(
            m.contains("[Effect-blur]\nBlurStrength=3")
                && m.contains("BlurStrength=5")
                && !m.contains("BlurStrength=9"),
            "{m}"
        );
        assert!(
            m.contains("blurEnabled=false")
                && !m.contains("blurEnabled=true")
                && m.contains("FocusPolicy=mine"),
            "{m}"
        );
        assert_eq!(
            kwinrc_merge(&t, &m)
                .lines()
                .filter(|l| l.starts_with('['))
                .count(),
            m.lines().filter(|l| l.starts_with('[')).count()
        );
    }

    #[test]
    fn breeze_and_klassy_settings_go_with_their_use() {
        let kv = "[KDE]\nwidgetStyle=kvantum\n";
        let aurorae = "[org.kde.kdecoration2]\nlibrary=org.kde.kwin.aurorae\n";
        let klassy = "[org.kde.kdecoration2]\nlibrary=org.kde.klassy\n";
        let breeze = "[org.kde.kdecoration2]\nlibrary=org.kde.breeze\n";
        let all = [
            ".config/breezerc",
            ".config/klassy/klassyrc",
            ".config/klassy/windecopresetsrc",
        ];
        assert_eq!(unused_files(kv, aurorae), all);
        assert_eq!(unused_files(kv, klassy), [".config/breezerc"]);
        assert_eq!(unused_files(kv, breeze), &all[1..]);
        assert_eq!(
            unused_files("[KDE]\nwidgetStyle=Breeze\n", klassy),
            Vec::<&str>::new()
        );
        assert_eq!(
            unused_files("[KDE]\nwidgetStyle=Klassy\n", aurorae),
            [".config/breezerc"]
        );
        // No keys: KDE's default is Breeze.
        assert_eq!(unused_files("", ""), &all[1..]);
    }

    #[test]
    fn kwinrules_merge_keeps_the_order_format_of_kwin_6() {
        // KWin 6.8 writes [General] Order=…; rewriting it as count/rules
        // would leave KWin without its rule list.
        let live = "[General]\nOrder=a\n\n[a]\nDescription=mine\nnoborder=true\nnoborderrule=2\n";
        let theme =
            "[General]\nOrder=b\n\n[b]\nDescription=theme\nopacityactive=90\nopacityactiverule=2\n";
        let m = kwinrules_merge(theme, live);
        assert!(
            m.contains("Order=a,b") && !m.contains("count=") && !m.contains("rules="),
            "{m}"
        );
        assert_eq!(kwinrules_merge(theme, &m), m);
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
        assert!(
            m.contains("// mine")
                && m.contains("203.0.113.7")
                && m.contains("\"editor.wordWrap\": \"on\"")
        );
        assert!(m.contains("\"workbench.colorTheme\": \"Dracula Theme\"") && m.contains("#282a36"));
        let parsed: serde_json::Value =
            serde_json::from_str(&m.replace("  // mine\n", "")).expect("valid JSON");
        assert_eq!(parsed["workbench.colorTheme"], "Dracula Theme");
        // Filter stores only the look.
        let f = jsonc_filter(live, VSCODE);
        assert!(f.contains("Doki") && !f.contains("203.0.113") && !f.contains("wordWrap"));
        // A look key the theme lacks is removed.
        let m2 = jsonc_merge(
            "{\"theme\": \"One Dark\"}",
            "{\n  \"theme\": \"Ayu\",\n  \"icon_theme\": \"Zed\",\n  \"agent\": {\"x\": 1}\n}\n",
            ZED,
        );
        let p2: serde_json::Value = serde_json::from_str(&m2).expect("valid JSON");
        assert_eq!(p2["theme"], "One Dark");
        assert!(p2.get("icon_theme").is_none() && p2["agent"]["x"] == 1);
    }

    #[test]
    fn kitty_split_moves_only_the_look() {
        let host = "font_size 11\nmap ctrl+c copy\nbackground #000000\ncolor1 #ff0000\nscrollback_lines 5000\nbackground_opacity 0.5\ndynamic_background_opacity yes\nconfirm_os_window_close 0\n";
        let (look, rest) = kitty_split(host);
        assert_eq!(
            look,
            "background #000000\ncolor1 #ff0000\nbackground_opacity 0.5\n"
        );
        assert!(rest.contains("font_size 11"), "font sizes stay the user's");
        assert!(
            rest.contains("map ctrl+c copy")
                && rest.contains("dynamic_background_opacity yes")
                && rest.contains("confirm_os_window_close")
        );
        let h = kitty_host(host);
        assert!(h.trim_end().ends_with(KITTY_INCLUDE) && !h.contains("color1"));
        assert_eq!(kitty_host(&h), h, "idempotent");
    }

    #[test]
    fn alacritty_split_and_import() {
        let host = "[env]\nTERM = \"xterm-256color\"\n\n[window]\nopacity = 0.5\nstartup_mode = \"Maximized\"\n\n[colors.primary]\nbackground = \"#000000\"\n\n[[keyboard.bindings]]\nkey = \"N\"\nmods = \"Control\"\naction = \"SpawnNewInstance\"\n";
        let (look, _) = alacritty_split(host).unwrap();
        assert!(
            look.contains("opacity = 0.5") && look.contains("#000000") && !look.contains("TERM")
        );
        let h = alacritty_host(host).unwrap();
        assert!(
            h.contains("startup_mode")
                && h.contains("SpawnNewInstance")
                && h.contains(ALACRITTY_IMPORT)
        );
        assert!(!h.contains("opacity") && !h.contains("#000000"));
        assert_eq!(alacritty_host(&h).unwrap(), h, "idempotent");
    }
}

#[cfg(test)]
mod reversible_tests {
    use super::*;
    #[test]
    fn inverse_json_preserves_nested_edits_comments_and_user_added_themes() {
        let before = r##"{"workbench.colorCustomizations":{"a":"#111","b":"#222"},"enabledThemes":["own"],"editor.fontSize":12}"##;
        let after = r##"{"workbench.colorCustomizations":{"a":"#aaa","b":"#bbb"},"enabledThemes":["own","kolira"],"editor.fontSize":12}"##;
        let current = r##"{
 // new personal note
 "workbench.colorCustomizations":{"a":"#aaa","b":"#ccc"},
 "enabledThemes":["own","kolira","new-personal"],"editor.fontSize":19
}"##;
        let restored =
            restore_changed(".config/Code/User/settings.json", before, after, current).unwrap();
        assert!(restored.contains("// new personal note"));
        let value: serde_json::Value =
            serde_json::from_str(&restored.replace("// new personal note", "")).unwrap();
        assert_eq!(value["workbench.colorCustomizations"]["a"], "#111");
        assert_eq!(value["workbench.colorCustomizations"]["b"], "#ccc");
        assert_eq!(
            value["enabledThemes"],
            serde_json::json!(["own", "new-personal"])
        );
        assert_eq!(value["editor.fontSize"], 19);
    }
    #[test]
    fn restore_font_family_preserves_later_user_size() {
        let restored = restore_changed(
            ".config/gtk-3.0/settings.ini",
            "[Settings]\ngtk-font-name=Original, 12\n",
            "[Settings]\ngtk-font-name=Other, 12\n",
            "[Settings]\ngtk-font-name=Other, 18\n[User]\nkeep=yes\n",
        )
        .unwrap();
        assert!(
            restored.contains("gtk-font-name=Original, 18"),
            "{restored}"
        );
        assert!(restored.contains("keep=yes"));
    }
}
