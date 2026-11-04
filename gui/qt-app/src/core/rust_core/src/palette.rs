//! Theme palette: base16 + an explicit KDE accent, and generators built on it.
//!
//! The palette is the single colour source of a generated theme. Its format is
//! base16 (tinted-theming), so any of the hundreds of published schemes can be
//! imported as-is; `accent` is extra because KDE needs one accent colour and
//! base16 has no such slot (default: `base0D`, the conventional "blue").
//!
//! Role mapping base16 → KDE is documented in `docs/PALETTE.md`; keep it in
//! sync with [`kde_color_scheme`].

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

/// Slot names in order (`base00`…`base0F`).
pub const SLOTS: [&str; 16] = [
    "base00", "base01", "base02", "base03", "base04", "base05", "base06", "base07",
    "base08", "base09", "base0A", "base0B", "base0C", "base0D", "base0E", "base0F",
];

/// Minimal WCAG AA contrast for body text.
pub const MIN_TEXT_CONTRAST: f64 = 4.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn parse(text: &str) -> Result<Rgb> {
        let hex = text.trim().trim_start_matches('#');
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(anyhow!("invalid colour {text:?} (expected #rrggbb)"));
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).expect("validated hex");
        Ok(Rgb(byte(0), byte(2), byte(4)))
    }

    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    /// `r,g,b` — the KDE colour-scheme notation.
    pub fn kde(self) -> String {
        format!("{},{},{}", self.0, self.1, self.2)
    }

    /// WCAG relative luminance (0 = black, 1 = white).
    pub fn luminance(self) -> f64 {
        let channel = |c: u8| {
            let c = f64::from(c) / 255.0;
            if c <= 0.039_28 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.0) + 0.7152 * channel(self.1) + 0.0722 * channel(self.2)
    }

    /// Linear blend: `amount` 0 → self, 1 → `other`.
    pub fn mix(self, other: Rgb, amount: f64) -> Rgb {
        let t = amount.clamp(0.0, 1.0);
        let blend = |a: u8, b: u8| (f64::from(a) + (f64::from(b) - f64::from(a)) * t).round() as u8;
        Rgb(blend(self.0, other.0), blend(self.1, other.1), blend(self.2, other.2))
    }
}

/// WCAG contrast ratio (1…21).
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (a.luminance(), b.luminance());
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Variant {
    Dark,
    Light,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Palette {
    pub name: String,
    #[serde(default)]
    pub author: String,
    pub variant: Variant,
    /// `base00`…`base0F` as `#rrggbb`.
    pub base: Vec<String>,
    pub accent: String,
    /// Where it came from: `base16:<file>`, `gallery:<id>`, `wallpaper:<path>`, `manual`.
    #[serde(default)]
    pub source: String,
}

impl Palette {
    pub fn slot(&self, index: usize) -> Rgb {
        Rgb::parse(&self.base[index]).expect("palette validated on construction")
    }

    pub fn accent_rgb(&self) -> Rgb {
        Rgb::parse(&self.accent).expect("palette validated on construction")
    }

    /// Check slot count and colour syntax (after deserialising untrusted JSON).
    pub fn validate(&self) -> Result<()> {
        if self.base.len() != 16 {
            return Err(anyhow!("palette needs 16 base colours, got {}", self.base.len()));
        }
        for (slot, value) in SLOTS.iter().zip(&self.base) {
            Rgb::parse(value).with_context(|| slot.to_string())?;
        }
        Rgb::parse(&self.accent).context("accent")?;
        if self.name.trim().is_empty() {
            return Err(anyhow!("palette name is empty"));
        }
        Ok(())
    }

    /// Set `base00`…`base0F` or `accent` (case-insensitive slot name).
    pub fn set(&mut self, slot: &str, value: &str) -> Result<()> {
        let rgb = Rgb::parse(value)?;
        if slot.eq_ignore_ascii_case("accent") {
            self.accent = rgb.hex();
            return Ok(());
        }
        let index = SLOTS
            .iter()
            .position(|s| s.eq_ignore_ascii_case(slot))
            .ok_or_else(|| anyhow!("unknown slot {slot:?} (base00…base0F or accent)"))?;
        self.base[index] = rgb.hex();
        Ok(())
    }

    /// Readability problems worth showing to the user (empty = fine).
    pub fn contrast_warnings(&self) -> Vec<String> {
        let roles = kde_roles(self);
        let mut out = Vec::new();
        for (label, fg, bg) in [
            ("text on window", roles.text, roles.window),
            ("text on view", roles.text, roles.view),
            ("selected text", roles.selection_text, roles.accent),
        ] {
            let ratio = contrast(fg, bg);
            if ratio < MIN_TEXT_CONTRAST {
                out.push(format!("{label}: contrast {ratio:.1}:1 < {MIN_TEXT_CONTRAST}:1"));
            }
        }
        out
    }
}

/// Parse a base16 scheme: tinted-theming YAML (`palette:` block, `#rrggbb`) or
/// the legacy flat format (`scheme:`, `base00: "rrggbb"`); JSON palettes
/// written by [`save`] are accepted too. Only the subset of YAML those files
/// use is understood (scalar `key: value` lines, `#` comments).
pub fn parse_base16(text: &str, source: &str) -> Result<Palette> {
    if text.trim_start().starts_with('{') {
        let palette: Palette = serde_json::from_str(text).context("invalid palette JSON")?;
        palette.validate()?;
        return Ok(palette);
    }

    let mut name = String::new();
    let mut author = String::new();
    let mut variant: Option<Variant> = None;
    let mut accent: Option<String> = None;
    let mut base: [Option<String>; 16] = Default::default();

    for raw in text.lines() {
        let line = strip_yaml_comment(raw);
        let Some((key, value)) = line.split_once(':') else { continue };
        let key = key.trim();
        let value = value.trim().trim_matches('"').trim_matches('\'').trim();
        match key {
            "name" | "scheme" => name = value.to_string(),
            "author" => author = value.to_string(),
            "variant" => {
                variant = match value {
                    "light" => Some(Variant::Light),
                    "dark" => Some(Variant::Dark),
                    _ => None,
                }
            }
            "accent" => accent = Some(Rgb::parse(value).context("accent")?.hex()),
            _ => {
                if let Some(index) = SLOTS.iter().position(|s| s.eq_ignore_ascii_case(key)) {
                    base[index] = Some(Rgb::parse(value).with_context(|| key.to_string())?.hex());
                }
            }
        }
    }

    let missing: Vec<&str> = SLOTS
        .iter()
        .zip(&base)
        .filter(|(_, v)| v.is_none())
        .map(|(s, _)| *s)
        .collect();
    if !missing.is_empty() {
        return Err(anyhow!("not a base16 scheme: missing {}", missing.join(", ")));
    }
    let base: Vec<String> = base.into_iter().map(|v| v.expect("checked above")).collect();
    // Schemes without `variant` (legacy format): decide by background luminance.
    let variant = variant.unwrap_or_else(|| {
        if Rgb::parse(&base[0]).expect("parsed").luminance() < 0.5 {
            Variant::Dark
        } else {
            Variant::Light
        }
    });
    let palette = Palette {
        name: if name.is_empty() { "Untitled".to_string() } else { name },
        author,
        variant,
        accent: accent.unwrap_or_else(|| base[0x0D].clone()),
        base,
        source: source.to_string(),
    };
    palette.validate()?;
    Ok(palette)
}

/// Drop a `# comment` that is not inside quotes.
fn strip_yaml_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    for (i, c) in line.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (None, '#') if i == 0 || line[..i].ends_with(char::is_whitespace) => return &line[..i],
            _ => {}
        }
    }
    line
}

pub fn load(path: &Path) -> Result<Palette> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    parse_base16(&text, &format!("base16:{}", path.display()))
}

pub fn save(palette: &Palette, path: &Path) -> Result<()> {
    palette.validate()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(palette)? + "\n";
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// The live palette (`<theme root>/palette.json`, tracked by profiles).
pub fn live_palette_path() -> std::path::PathBuf {
    crate::paths::live_path(LIVE_PALETTE_REL)
}

/// Profile-snapshot path of the live palette.
pub const LIVE_PALETTE_REL: &str = ".config/theme-profile/palette.json";

// ─────────────────────────────────────────────────────────────────────────────
// KDE colour scheme
// ─────────────────────────────────────────────────────────────────────────────

/// Resolved KDE roles — shared by the generator, contrast checks and preview.
#[derive(Debug, Clone, Copy)]
pub struct KdeRoles {
    pub window: Rgb,
    pub window_alt: Rgb,
    pub view: Rgb,
    pub view_alt: Rgb,
    pub button: Rgb,
    pub button_alt: Rgb,
    pub header: Rgb,
    pub text: Rgb,
    pub text_inactive: Rgb,
    pub accent: Rgb,
    pub selection_text: Rgb,
    pub link: Rgb,
    pub visited: Rgb,
    pub negative: Rgb,
    pub neutral: Rgb,
    pub positive: Rgb,
    pub hover: Rgb,
}

pub fn kde_roles(p: &Palette) -> KdeRoles {
    let accent = p.accent_rgb();
    // Views (lists, editors) sit one step "deeper" than the window: darker on
    // dark schemes, lighter on light ones — base16 has no slot for that.
    let deep = match p.variant {
        Variant::Dark => p.slot(0x00).mix(Rgb(0, 0, 0), 0.22),
        Variant::Light => p.slot(0x00).mix(Rgb(255, 255, 255), 0.55),
    };
    // Selected text: whichever of base00/base07 reads better on the accent.
    let selection_text = [p.slot(0x00), p.slot(0x07), p.slot(0x05)]
        .into_iter()
        .max_by(|a, b| contrast(*a, accent).total_cmp(&contrast(*b, accent)))
        .expect("non-empty");
    KdeRoles {
        window: p.slot(0x00),
        window_alt: p.slot(0x01),
        view: deep,
        view_alt: p.slot(0x00),
        button: p.slot(0x01),
        button_alt: p.slot(0x02),
        header: deep,
        text: p.slot(0x05),
        text_inactive: p.slot(0x03),
        accent,
        selection_text,
        link: p.slot(0x0D),
        visited: p.slot(0x0E),
        negative: p.slot(0x08),
        neutral: p.slot(0x0A),
        positive: p.slot(0x0B),
        hover: accent.mix(p.slot(0x05), 0.25),
    }
}

const COLOR_EFFECTS: &str = "\
[ColorEffects:Disabled]
Color=56,56,56
ColorAmount=0
ColorEffect=0
ContrastAmount=0.65
ContrastEffect=1
IntensityAmount=0.1
IntensityEffect=2

[ColorEffects:Inactive]
ChangeSelectionColor=true
Color=112,111,110
ColorAmount=0.025
ColorEffect=2
ContrastAmount=0.1
ContrastEffect=2
Enable=false
IntensityAmount=0
IntensityEffect=0
";

/// File stem of the generated scheme: `ThemeProfile<CamelName>` — never the
/// name of a scheme the user made by hand (e.g. `Everforest.colors`).
pub fn kde_scheme_id(p: &Palette) -> String {
    let camel: String = p
        .name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            let first = chars.next().expect("non-empty").to_ascii_uppercase();
            first.to_string() + chars.as_str()
        })
        .collect();
    format!("ThemeProfile{}", if camel.is_empty() { "Palette".into() } else { camel })
}

/// Full KDE `.colors` file for the palette.
pub fn kde_color_scheme(p: &Palette) -> String {
    let r = kde_roles(p);
    let mut out = String::from(COLOR_EFFECTS);

    let group = |out: &mut String, name: &str, bg: Rgb, alt: Rgb, overrides: &[(&str, Rgb)]| {
        let mut keys: Vec<(&str, Rgb)> = vec![
            ("BackgroundAlternate", alt),
            ("BackgroundNormal", bg),
            ("DecorationFocus", r.accent),
            ("DecorationHover", r.hover),
            ("ForegroundActive", r.accent),
            ("ForegroundInactive", r.text_inactive),
            ("ForegroundLink", r.link),
            ("ForegroundNegative", r.negative),
            ("ForegroundNeutral", r.neutral),
            ("ForegroundNormal", r.text),
            ("ForegroundPositive", r.positive),
            ("ForegroundVisited", r.visited),
        ];
        for (key, value) in overrides {
            if let Some(slot) = keys.iter_mut().find(|(k, _)| k == key) {
                slot.1 = *value;
            }
        }
        let _ = writeln!(out, "\n[Colors:{name}]");
        for (key, value) in keys {
            let _ = writeln!(out, "{key}={}", value.kde());
        }
    };

    group(&mut out, "Button", r.button, r.button_alt, &[]);
    group(&mut out, "Complementary", r.view, r.window, &[]);
    group(&mut out, "Header", r.header, r.window, &[]);
    group(&mut out, "Header][Inactive", r.window, r.header, &[]);
    let on_accent = r.selection_text;
    group(
        &mut out,
        "Selection",
        r.accent,
        r.accent.mix(r.window, 0.3),
        &[
            ("ForegroundNormal", on_accent),
            ("ForegroundActive", on_accent),
            ("ForegroundInactive", on_accent.mix(r.accent, 0.35)),
            ("ForegroundLink", on_accent),
            ("ForegroundVisited", on_accent),
        ],
    );
    group(&mut out, "Tooltip", r.window_alt, r.window, &[]);
    group(&mut out, "View", r.view, r.view_alt, &[]);
    group(&mut out, "Window", r.window, r.window_alt, &[]);

    let id = kde_scheme_id(p);
    let _ = write!(
        out,
        "\n[General]\nColorScheme={id}\nName={name}\nshadeSortColumn=true\n\n\
         [KDE]\ncontrast=4\n\n\
         [WM]\nactiveBackground={hdr}\nactiveBlend={txt}\nactiveForeground={txt}\n\
         inactiveBackground={win}\ninactiveBlend={dim}\ninactiveForeground={dim}\n",
        name = p.name,
        hdr = r.header.kde(),
        txt = r.text.kde(),
        win = r.window.kde(),
        dim = r.text_inactive.kde(),
    );
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Terminals (kitty, Alacritty)
// ─────────────────────────────────────────────────────────────────────────────

/// Markers of the palette block inside `kitty.conf`; everything outside them
/// is the user's and is never touched.
pub const KITTY_BEGIN: &str = "# BEGIN theme-profile palette (generated, edits are overwritten)";
pub const KITTY_END: &str = "# END theme-profile palette";

/// ANSI colours 0–15 from base16. `color0`/`color8` use `base02`/`base03`
/// rather than `base00`, so "black" text stays visible on the background
/// (same choice as the official Everforest/Gruvbox terminal ports).
pub fn ansi16(p: &Palette) -> [Rgb; 16] {
    let s = |i: usize| p.slot(i);
    [
        s(0x02), s(0x08), s(0x0B), s(0x0A), s(0x0D), s(0x0E), s(0x0C), s(0x05),
        s(0x03), s(0x08), s(0x0B), s(0x0A), s(0x0D), s(0x0E), s(0x0C), s(0x07),
    ]
}

/// The kitty colour block (with markers).
pub fn kitty_colors(p: &Palette) -> String {
    let r = kde_roles(p);
    let mut out = format!("{KITTY_BEGIN}\n# palette: {}\n", p.name);
    let mut line = |key: &str, value: Rgb| {
        let _ = writeln!(out, "{key} {}", value.hex());
    };
    line("foreground", r.text);
    line("background", r.window);
    line("selection_foreground", r.selection_text);
    line("selection_background", r.accent);
    line("cursor", r.text);
    line("cursor_text_color", r.window);
    line("url_color", r.link);
    line("active_border_color", r.accent);
    line("inactive_border_color", p.slot(0x02));
    line("bell_border_color", r.neutral);
    line("active_tab_foreground", r.selection_text);
    line("active_tab_background", r.accent);
    line("inactive_tab_foreground", r.text_inactive);
    line("inactive_tab_background", r.window_alt);
    line("tab_bar_background", r.view);
    for (i, color) in ansi16(p).iter().enumerate() {
        line(&format!("color{i}"), *color);
    }
    out.push_str(KITTY_END);
    out.push('\n');
    out
}

/// `kitty.conf` with the palette block replaced (or appended at the end, so
/// it wins over earlier colour lines — kitty uses the last value).
pub fn kitty_conf_with_palette(existing: &str, p: &Palette) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in existing.lines() {
        if line.starts_with("# BEGIN theme-profile palette") {
            inside = true;
            continue;
        }
        if inside {
            if line.starts_with(KITTY_END) {
                inside = false;
            }
            continue;
        }
        kept.push(line);
    }
    while kept.last().is_some_and(|l| l.trim().is_empty()) {
        kept.pop();
    }
    let mut out = kept.join("\n");
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(&kitty_colors(p));
    out
}

/// `alacritty.toml` with `[colors.*]` set from the palette; comments, order
/// and every other key are preserved (toml_edit, no reserialisation).
pub fn alacritty_with_palette(existing: &str, p: &Palette) -> Result<String> {
    use toml_edit::{value, DocumentMut, Item, Table};

    let mut doc: DocumentMut = existing.parse().context("alacritty.toml is not valid TOML")?;
    let r = kde_roles(p);
    let ansi = ansi16(p);
    let names = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];

    let colors = doc
        .entry("colors")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .ok_or_else(|| anyhow!("alacritty.toml: `colors` is not a table"))?;
    colors.set_implicit(true);
    let mut set = |table: &str, entries: Vec<(&str, Rgb)>| -> Result<()> {
        let t = colors
            .entry(table)
            .or_insert(Item::Table(Table::new()))
            .as_table_mut()
            .ok_or_else(|| anyhow!("alacritty.toml: `colors.{table}` is not a table"))?;
        for (key, color) in entries {
            t[key] = value(color.hex());
        }
        Ok(())
    };
    set("primary", vec![("background", r.window), ("foreground", r.text)])?;
    set("cursor", vec![("text", r.window), ("cursor", r.text)])?;
    set("selection", vec![("text", r.selection_text), ("background", r.accent)])?;
    set("normal", names.iter().copied().zip(ansi[..8].iter().copied()).collect())?;
    set("bright", names.iter().copied().zip(ansi[8..].iter().copied()).collect())?;
    Ok(doc.to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// Applying
// ─────────────────────────────────────────────────────────────────────────────

/// Targets a palette can be applied to.
pub const TARGETS: [&str; 10] = ["kde", "kitty", "alacritty", "konsole", "ghostty", "nvim", "discord", "spicetify", "cava", "kvantum"];

/// Targets used when none are given. `kvantum` is opt-in: it switches the Qt
/// widget style, which the user may have chosen deliberately (e.g. Breeze).
pub const DEFAULT_TARGETS: [&str; 9] = ["kde", "kitty", "alacritty", "konsole", "ghostty", "nvim", "discord", "spicetify", "cava"];

/// Extra knobs for [`apply`].
#[derive(Debug, Default, Clone)]
pub struct ApplyOptions {
    /// Installed Kvantum theme to recolour (default by variant: KvArcDark/KvArc).
    pub kvantum_base: Option<String>,
}

fn no_apply() -> bool {
    std::env::var("KTM_THEME_NO_APPLY").as_deref() == Ok("1")
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

/// Remote-control sockets of running kitty instances started with
/// `listen_on unix:/tmp/kitty-theme-profile`. kitty appends `-<pid>` to the
/// path, so the bare name alone never matches.
pub fn kitty_sockets() -> Vec<String> {
    kitty_sockets_in(Path::new("/tmp"))
}

fn kitty_sockets_in(dir: &Path) -> Vec<String> {
    const BASE: &str = "kitty-theme-profile";
    let mut sockets: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    let is_ours = name == BASE
                        || name
                            .strip_prefix(BASE)
                            .and_then(|rest| rest.strip_prefix('-'))
                            .is_some_and(|pid| !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()));
                    is_ours.then(|| format!("unix:{}", dir.join(&name).display()))
                })
                .collect()
        })
        .unwrap_or_default();
    sockets.sort();
    sockets
}

/// Atomic write that keeps the permissions of an existing file.
fn write_atomic(path: &Path, text: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mode = std::fs::metadata(path).map(|m| m.permissions().mode()).ok();
    let tmp = path.with_extension("tmp-theme-profile");
    std::fs::write(&tmp, text)?;
    if let Some(mode) = mode {
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode))?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Copy a user config into `backups/palette-apply-<time>/` before editing it.
fn backup(path: &Path, stamp: &str) -> Result<()> {
    let dir = crate::paths::backups_dir().join(format!("palette-apply-{stamp}"));
    std::fs::create_dir_all(&dir)?;
    let name = path.file_name().ok_or_else(|| anyhow!("no file name"))?;
    std::fs::copy(path, dir.join(name))?;
    Ok(())
}

/// Generate and apply the palette to `targets`; returns one line per action.
/// Generated files carry `ThemeProfile*` names and terminal configs only get
/// their colour keys/blocks replaced (after a backup), so user-made assets are
/// never overwritten. A missing terminal config is skipped, not created. The
/// palette becomes the live palette (`palette.json`).
pub fn apply(p: &Palette, targets: &[String], options: &ApplyOptions) -> Result<Vec<String>> {
    p.validate()?;
    for target in targets {
        if !TARGETS.contains(&target.as_str()) {
            return Err(anyhow!("unknown target {target:?} (known: {})", TARGETS.join(", ")));
        }
    }
    let wants = |t: &str| targets.iter().any(|x| x == t);
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let mut report = Vec::new();

    if wants("kde") {
        let id = kde_scheme_id(p);
        let path = crate::paths::data_home().join("color-schemes").join(format!("{id}.colors"));
        write_atomic(&path, &kde_color_scheme(p))?;
        // An explicit palette accent must not be overridden by the wallpaper.
        run(
            "kwriteconfig6",
            &["--notify", "--file", "kdeglobals", "--group", "General", "--key", "accentColorFromWallpaper", "false"],
        );
        // Two calls on purpose: with `--accent-color`, plasma-apply-colorscheme
        // only sets the accent and silently ignores the scheme argument.
        let applied = run("plasma-apply-colorscheme", &[&id])
            && run("plasma-apply-colorscheme", &["--accent-color", &p.accent]);
        report.push(format!(
            "kde: {} ({}){}",
            path.display(),
            id,
            if applied { "" } else { " — plasma-apply-colorscheme failed" }
        ));
    }

    if wants("kitty") {
        let host = crate::paths::live_path(crate::appearance::KITTY_HOST);
        if host.is_file() {
            // The palette goes into the include file (theme-profile.conf), not
            // into the user's kitty.conf (which only gains the include line).
            crate::profile::split_live_hosts(&[crate::appearance::KITTY_HOST.to_string()])?;
            let host_text = std::fs::read_to_string(&host)?;
            write_atomic(&host, &crate::appearance::kitty_host(&host_text))?;
            let path = crate::paths::live_path(crate::appearance::KITTY_THEME);
            let existing = std::fs::read_to_string(&path).unwrap_or_default();
            if path.is_file() {
                backup(&path, &stamp)?;
            }
            write_atomic(&path, &kitty_conf_with_palette(&existing, p))?;
            // Live reload needs a kitty started with allow_remote_control and
            // listen_on unix:/tmp/kitty-theme-profile (see AGENTS.md).
            let config = host.to_string_lossy().to_string();
            let sockets = if which::which("kitty").is_ok() { kitty_sockets() } else { Vec::new() };
            let mut live = false;
            for socket in &sockets {
                live |= run("kitty", &["@", "--to", socket, "set-colors", "--all", "--configured", &config]);
            }
            report.push(format!(
                "kitty: {}{}",
                path.display(),
                if live { "" } else { " (no running kitty to reload)" }
            ));
        } else {
            report.push(format!("kitty: skipped, {} not found", host.display()));
        }
    }

    if wants("alacritty") {
        let host = crate::paths::live_path(crate::appearance::ALACRITTY_HOST);
        if host.is_file() {
            // Colours go into the imported theme-profile.toml; alacritty.toml
            // only gains the import (Alacritty reloads both by itself).
            crate::profile::split_live_hosts(&[crate::appearance::ALACRITTY_HOST.to_string()])?;
            let host_text = std::fs::read_to_string(&host)?;
            write_atomic(&host, &crate::appearance::alacritty_host(&host_text)?)?;
            let path = crate::paths::live_path(crate::appearance::ALACRITTY_THEME);
            let existing = std::fs::read_to_string(&path).unwrap_or_default();
            let updated = alacritty_with_palette(&existing, p)?;
            if path.is_file() {
                backup(&path, &stamp)?;
            }
            write_atomic(&path, &updated)?;
            report.push(format!("alacritty: {}", path.display()));
        } else {
            report.push(format!("alacritty: skipped, {} not found", host.display()));
        }
    }

    if wants("konsole") {
        if which::which("konsole").is_ok() || crate::paths::data_home().join("konsole").exists() {
            let scheme = crate::konsole::apply_palette(p)?;
            report.push(format!("konsole: {} (new tabs/windows; open ones keep their scheme)", scheme.display()));
        } else {
            report.push("konsole: skipped, not installed".to_string());
        }
    }

    if wants("nvim") {
        match crate::nvim::apply_palette(p)? {
            Some(path) => report.push(format!("nvim: {} (new Neovim sessions; `:colorscheme theme-profile` reloads)", path.display())),
            None => report.push("nvim: skipped, not installed".to_string()),
        }
    }

    if wants("discord") {
        let files = crate::discord::apply_palette(p)?;
        if files.is_empty() {
            report.push("discord: skipped, no Vencord/Vesktop (stock Discord has no themes)".to_string());
        }
        for f in files {
            report.push(format!("discord: {} (enabled; Vencord reloads themes itself)", f.display()));
        }
    }

    if wants("spicetify") {
        match crate::spicetify::apply_palette(p)? {
            Some(path) => report.push(format!("spicetify: {} (selected; spicetify apply)", path.display())),
            None => report.push("spicetify: skipped, no ~/.config/spicetify/config-xpui.ini".to_string()),
        }
    }

    if wants("ghostty") {
        match crate::extras::ghostty_apply_palette(p)? {
            Some(path) => report.push(format!("ghostty: {} (included by config; new windows)", path.display())),
            None => report.push("ghostty: skipped, no ~/.config/ghostty/config".to_string()),
        }
    }

    if wants("cava") {
        match crate::extras::cava_apply_palette(p)? {
            Some(path) => report.push(format!("cava: {} (gradient; running cava reloaded)", path.display())),
            None => report.push("cava: skipped, no ~/.config/cava/config".to_string()),
        }
    }

    if wants("kvantum") {
        let base = options
            .kvantum_base
            .clone()
            .unwrap_or_else(|| crate::kvantum::default_base(p).to_string());
        let (id, dir) = crate::kvantum::build(p, &base)?;
        let set = run("kvantummanager", &["--set", &id])
            && run("kwriteconfig6", &["--notify", "--file", "kdeglobals", "--group", "KDE", "--key", "widgetStyle", "kvantum"]);
        report.push(format!(
            "kvantum: {} (recoloured from {base}){}",
            dir.display(),
            if set { "" } else { " — kvantummanager failed" }
        ));
    }

    save(p, &live_palette_path())?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERFOREST: &str = include_str!("../data/base16/everforest.yaml");
    const GRUVBOX_LIGHT: &str = include_str!("../data/base16/gruvbox-light-medium.yaml");

    fn group<'a>(scheme: &'a str, name: &str) -> &'a str {
        let header = format!("[{name}]\n");
        let start = scheme.find(&header).unwrap_or_else(|| panic!("missing {name}")) + header.len();
        let end = scheme[start..].find("\n[").map_or(scheme.len(), |i| start + i);
        &scheme[start..end]
    }

    #[test]
    fn parses_tinted_theming_yaml_with_comments() {
        let p = parse_base16(EVERFOREST, "test").unwrap();
        assert_eq!(p.name, "Everforest");
        assert_eq!(p.variant, Variant::Dark);
        assert_eq!(p.base[0], "#2d353b");
        assert_eq!(p.base[0x0B], "#a7c080");
        assert_eq!(p.accent, "#7fbbb3", "default accent is base0D");
    }

    #[test]
    fn parses_legacy_format_and_guesses_variant() {
        let mut text = String::from("scheme: \"Old\"\nauthor: \"x\"\n");
        for (i, slot) in SLOTS.iter().enumerate() {
            let v = if i == 0 { "fafafa".to_string() } else { format!("{:02x}{:02x}{:02x}", i, i, i) };
            text.push_str(&format!("{slot}: \"{v}\"\n"));
        }
        text.push_str("accent: '#a7c080'\n");
        let p = parse_base16(&text, "legacy").unwrap();
        assert_eq!(p.name, "Old");
        assert_eq!(p.variant, Variant::Light);
        assert_eq!(p.accent, "#a7c080");
    }

    #[test]
    fn rejects_incomplete_or_bad_schemes() {
        assert!(parse_base16("name: x\nbase00: \"#000000\"\n", "t").is_err());
        let bad = EVERFOREST.replace("#a7c080", "#zzzzzz");
        assert!(parse_base16(&bad, "t").is_err());
    }

    #[test]
    fn json_roundtrip_and_set() {
        let mut p = parse_base16(EVERFOREST, "t").unwrap();
        p.set("accent", "#A7C080").unwrap();
        p.set("BASE0f", "#123456").unwrap();
        assert!(p.set("base10", "#000000").is_err());
        let json = serde_json::to_string(&p).unwrap();
        let back = parse_base16(&json, "ignored").unwrap();
        assert_eq!(back, p);
        assert_eq!(back.accent, "#a7c080");
    }

    #[test]
    fn contrast_matches_wcag() {
        assert!((contrast(Rgb(0, 0, 0), Rgb(255, 255, 255)) - 21.0).abs() < 0.01);
        assert!((contrast(Rgb(10, 10, 10), Rgb(10, 10, 10)) - 1.0).abs() < 1e-9);
        let p = parse_base16(EVERFOREST, "t").unwrap();
        assert!(p.contrast_warnings().is_empty(), "{:?}", p.contrast_warnings());
    }

    #[test]
    fn kde_scheme_dark_everforest() {
        let mut p = parse_base16(EVERFOREST, "t").unwrap();
        p.set("accent", "#a7c080").unwrap();
        let s = kde_color_scheme(&p);
        for g in [
            "ColorEffects:Disabled", "ColorEffects:Inactive", "Colors:Button",
            "Colors:Complementary", "Colors:Header", "Colors:Header][Inactive",
            "Colors:Selection", "Colors:Tooltip", "Colors:View", "Colors:Window",
            "General", "KDE", "WM",
        ] {
            group(&s, g);
        }
        let window = group(&s, "Colors:Window");
        assert!(window.contains("BackgroundNormal=45,53,59\n"));
        assert!(window.contains("ForegroundNormal=211,198,170\n"));
        assert!(window.contains("DecorationFocus=167,192,128\n"));
        assert!(window.contains("ForegroundNegative=230,126,128\n"));
        assert_eq!(window.lines().count(), 12);
        // View is deeper (darker) than the window on a dark scheme.
        let view_bg = group(&s, "Colors:View").lines().find(|l| l.starts_with("BackgroundNormal=")).unwrap();
        let rgb: Vec<u8> = view_bg[17..].split(',').map(|v| v.parse().unwrap()).collect();
        assert!(Rgb(rgb[0], rgb[1], rgb[2]).luminance() < Rgb(45, 53, 59).luminance());
        // Selection: accent background, readable text.
        let sel = group(&s, "Colors:Selection");
        assert!(sel.contains("BackgroundNormal=167,192,128\n"));
        assert!(sel.contains("ForegroundNormal=45,53,59\n"));
        let general = group(&s, "General");
        assert!(general.contains("ColorScheme=ThemeProfileEverforest"));
        assert!(general.contains("Name=Everforest"));
    }

    #[test]
    fn kde_scheme_light_gruvbox() {
        let p = parse_base16(GRUVBOX_LIGHT, "t").unwrap();
        assert_eq!(p.variant, Variant::Light);
        let s = kde_color_scheme(&p);
        let window = group(&s, "Colors:Window");
        assert!(window.contains("BackgroundNormal=251,241,199\n"));
        assert!(window.contains("ForegroundNormal=80,73,69\n"));
        let view_bg = group(&s, "Colors:View").lines().find(|l| l.starts_with("BackgroundNormal=")).unwrap();
        let rgb: Vec<u8> = view_bg[17..].split(',').map(|v| v.parse().unwrap()).collect();
        assert!(Rgb(rgb[0], rgb[1], rgb[2]).luminance() > Rgb(251, 241, 199).luminance());
        assert_eq!(kde_scheme_id(&p), "ThemeProfileGruvboxLightMedium");
        assert!(p.contrast_warnings().is_empty(), "{:?}", p.contrast_warnings());
    }

    #[test]
    fn kitty_block_is_replaced_not_duplicated() {
        let mut p = parse_base16(EVERFOREST, "t").unwrap();
        p.set("accent", "#a7c080").unwrap();
        let user = "font_size 11\nbackground #000000\n";
        let once = kitty_conf_with_palette(user, &p);
        assert!(once.starts_with("font_size 11\nbackground #000000\n\n# BEGIN theme-profile palette"));
        assert!(once.contains("\nbackground #2d353b\n"));
        assert!(once.contains("\nselection_background #a7c080\n"));
        assert!(once.contains("\ncolor0 #475258\n"), "color0 = base02, visible on the background");
        assert!(once.contains("\ncolor15 #fdf6e3\n"));
        let twice = kitty_conf_with_palette(&once, &p);
        assert_eq!(once, twice, "re-applying is idempotent");
        assert_eq!(twice.matches("# BEGIN theme-profile palette").count(), 1);
        let light = parse_base16(GRUVBOX_LIGHT, "t").unwrap();
        let switched = kitty_conf_with_palette(&twice, &light);
        assert!(switched.contains("\nbackground #fbf1c7\n"));
        assert!(!switched.contains("#2d353b"));
        assert!(switched.starts_with("font_size 11\n"));
    }

    #[test]
    fn alacritty_keeps_user_settings_and_comments() {
        let p = parse_base16(EVERFOREST, "t").unwrap();
        let user = "[window]\nopacity = 0.5 # keep me\n\n[colors.primary]\n#background = \"#1e1b2e\"\n\n[[keyboard.bindings]]\nkey = \"V\"\n";
        let out = alacritty_with_palette(user, &p).unwrap();
        assert!(out.contains("opacity = 0.5 # keep me"));
        assert!(out.contains("#background = \"#1e1b2e\""));
        assert!(out.contains("key = \"V\""));
        let doc: toml::Value = toml::from_str(&out).unwrap();
        assert_eq!(doc["colors"]["primary"]["background"].as_str(), Some("#2d353b"));
        assert_eq!(doc["colors"]["normal"]["red"].as_str(), Some("#e67e80"));
        assert_eq!(doc["colors"]["bright"]["white"].as_str(), Some("#fdf6e3"));
        assert_eq!(doc["colors"]["selection"]["background"].as_str(), Some("#7fbbb3"));
        assert_eq!(alacritty_with_palette(&out, &p).unwrap(), out, "idempotent");
        assert!(alacritty_with_palette("colors = 3\n", &p).is_err());
        assert!(alacritty_with_palette("[broken\n", &p).is_err());
    }

    #[test]
    fn kitty_sockets_include_pid_suffixed_ones() {
        let tmp = tempfile::tempdir().unwrap();
        for name in ["kitty-theme-profile-5466", "kitty-theme-profile", "kitty-theme-profile-x1", "kitty-other-1"] {
            std::fs::write(tmp.path().join(name), "").unwrap();
        }
        let found = kitty_sockets_in(tmp.path());
        let d = tmp.path().display();
        assert_eq!(found, vec![format!("unix:{d}/kitty-theme-profile"), format!("unix:{d}/kitty-theme-profile-5466")]);
    }
}
