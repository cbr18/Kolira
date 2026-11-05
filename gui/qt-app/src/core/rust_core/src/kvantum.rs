//! Kvantum theme in the palette's colours, recoloured on the fly.
//!
//! Kvantum themes are GPL artwork, so the kit ships none: at apply time an
//! *installed* base theme (default `KvArcDark` / `KvArc` by palette variant)
//! is copied to `~/.config/Kvantum/ThemeProfile<Name>/` and every colour in its
//! SVG and kvconfig is remapped in OKLab:
//!
//! * near-neutral colours keep their position on the base theme's
//!   background → text lightness scale, re-expressed on the palette's scale
//!   (extrapolated beyond both ends, so shadows/highlights keep their depth);
//! * colours close in hue to the base theme's highlight become the palette
//!   accent with the same lightness offset and relative chroma;
//! * other saturated colours (semantic red/green…) are left alone.
//!
//! Alpha (`#rrggbbaa`) is preserved; `[GeneralColors]` keys with a clear role
//! are then set explicitly from the KDE roles.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::oklab::{from_oklab, hue_distance, oklab, Lab};
use crate::palette::{kde_roles, kde_scheme_id, Palette, Rgb, Variant};

/// Chroma below which a colour counts as neutral (OKLab units).
const NEUTRAL_CHROMA: f64 = 0.035;
/// Max hue distance (degrees) for a colour to follow the base highlight.
const ACCENT_HUE_RANGE: f64 = 45.0;

/// The recolouring rule, built from the base theme's anchors and the palette.
pub struct Recolor {
    base_bg: Lab,
    base_fg: Lab,
    base_hl: Lab,
    bg: Lab,
    fg: Lab,
    accent: Lab,
}

impl Recolor {
    pub fn new(base_bg: Rgb, base_fg: Rgb, base_hl: Rgb, p: &Palette) -> Recolor {
        let roles = kde_roles(p);
        Recolor {
            base_bg: oklab(base_bg),
            base_fg: oklab(base_fg),
            base_hl: oklab(base_hl),
            bg: oklab(roles.window),
            fg: oklab(roles.text),
            accent: oklab(roles.accent),
        }
    }

    pub fn map(&self, c: Rgb) -> Rgb {
        let lab = oklab(c);
        let chroma = lab.chroma();
        if chroma >= NEUTRAL_CHROMA && self.base_hl.chroma() >= NEUTRAL_CHROMA {
            if hue_distance(lab.hue(), self.base_hl.hue()) <= ACCENT_HUE_RANGE {
                let l = (self.accent.l + (lab.l - self.base_hl.l)).clamp(0.0, 1.0);
                let ratio = (chroma / self.base_hl.chroma()).clamp(0.0, 1.5);
                let target_chroma = self.accent.chroma() * ratio;
                let hue = self.accent.b.atan2(self.accent.a);
                return from_oklab(Lab { l, a: target_chroma * hue.cos(), b: target_chroma * hue.sin() });
            }
            return c; // semantic colour (red close button, green OK…)
        }
        let span = self.base_fg.l - self.base_bg.l;
        let t = if span.abs() < 1e-6 { 0.0 } else { (lab.l - self.base_bg.l) / span };
        let w = t.clamp(0.0, 1.0);
        from_oklab(Lab {
            l: (self.bg.l + t * (self.fg.l - self.bg.l)).clamp(0.0, 1.0),
            a: self.bg.a + w * (self.fg.a - self.bg.a),
            b: self.bg.b + w * (self.fg.b - self.bg.b),
        })
    }

    /// Replace every `#rgb`, `#rrggbb` and `#rrggbbaa` token (alpha kept).
    /// Tokens glued to a word character (`url(#defs5)`) are not colours.
    pub fn map_text(&self, text: &str) -> String {
        let bytes = text.as_bytes();
        let mut out = String::with_capacity(text.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'#' {
                let start = i + 1;
                let mut end = start;
                while end < bytes.len() && bytes[end].is_ascii_hexdigit() {
                    end += 1;
                }
                let glued = end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_');
                let len = end - start;
                if !glued && matches!(len, 3 | 6 | 8) {
                    let hex = &text[start..end];
                    let (rgb, alpha) = match len {
                        3 => {
                            let e: String = hex.chars().flat_map(|c| [c, c]).collect();
                            (e, "")
                        }
                        6 => (hex.to_string(), ""),
                        _ => (hex[..6].to_string(), &hex[6..]),
                    };
                    if let Ok(c) = Rgb::parse(&rgb) {
                        out.push_str(&self.map(c).hex());
                        out.push_str(&alpha.to_ascii_lowercase());
                        i = end;
                        continue;
                    }
                }
            }
            let ch = text[i..].chars().next().expect("in bounds");
            out.push(ch);
            i += ch.len_utf8();
        }
        out
    }
}

fn named(value: &str) -> Option<Rgb> {
    match value.trim().to_ascii_lowercase().as_str() {
        "black" => Some(Rgb(0, 0, 0)),
        "white" => Some(Rgb(255, 255, 255)),
        _ => Rgb::parse(value.trim().get(..7).unwrap_or("")).ok(),
    }
}

fn raw_general(kvconfig: &str, key: &str) -> Option<String> {
    let mut in_colors = false;
    for line in kvconfig.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_colors = line == "[GeneralColors]";
            continue;
        }
        if in_colors {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

fn general_color(kvconfig: &str, key: &str) -> Option<Rgb> {
    raw_general(kvconfig, key).and_then(|v| named(&v))
}

/// Recolour a kvconfig: map every colour, then set role keys explicitly.
pub fn recolor_kvconfig(kvconfig: &str, recolor: &Recolor, p: &Palette) -> String {
    let r = kde_roles(p);
    // Tooltip text follows the palette text: recolouring a light base into a
    // dark palette (or back) inverts lightness, and a base's white-on-dark
    // tooltip text would land dark-on-dark.
    // Themes without tooltip artwork (KvRoughGlass) otherwise get a black
    // tooltip base; set both so the pair always matches the palette.
    let roles: [(&str, Rgb); 12] = [
        ("tooltip.base.color", r.window_alt),
        ("tooltip.text.color", r.text),
        ("window.color", r.window),
        ("base.color", r.view),
        ("alt.base.color", r.view_alt),
        ("button.color", r.button),
        ("highlight.color", r.accent),
        ("inactive.highlight.color", r.accent.mix(r.window, 0.4)),
        ("highlight.text.color", r.selection_text),
        ("progress.indicator.text.color", r.selection_text),
        ("link.color", r.link),
        ("link.visited.color", r.visited),
    ];
    // Text drawn on the accent (pressed/toggled buttons) uses the very value
    // of the base's `highlight.text.color`; give it the palette's on-accent
    // text, otherwise e.g. light text lands on a light accent.
    let on_accent = raw_general(kvconfig, "highlight.text.color");
    let mut in_colors = false;
    let mut out = String::with_capacity(kvconfig.len());
    for line in kvconfig.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_colors = trimmed == "[GeneralColors]";
        }
        let on_accent_key = trimmed.split_once('=').filter(|(k, v)| {
            matches!(k.trim(), "text.press.color" | "text.toggle.color")
                && on_accent.as_deref() == Some(v.trim())
        });
        let mapped = if let Some((key, _)) = on_accent_key {
            format!("{}={}", key.trim(), r.selection_text.hex())
        } else if in_colors {
            if let Some((key, value)) = trimmed.split_once('=') {
                let key = key.trim();
                if let Some((_, role)) = roles.iter().find(|(k, _)| *k == key) {
                    format!("{key}={}", role.hex())
                } else if let Some(c) = named(value).filter(|_| !value.trim().starts_with('#')) {
                    format!("{key}={}", recolor.map(c).hex())
                } else {
                    recolor.map_text(line)
                }
            } else {
                line.to_string()
            }
        } else {
            recolor.map_text(line)
        };
        out.push_str(&mapped);
        out.push('\n');
    }
    // Roles the base does not set at all get added (e.g. tooltip.base.color).
    for (key, role) in roles {
        if raw_general(&out, key).is_none() {
            out = crate::field::set_ini_text(&out, "GeneralColors", key, &role.hex());
        }
    }
    out
}

/// First line of a generated kvconfig: which theme it was recoloured from.
/// Themes that ship with Kvantum itself are GPL-3.0-or-later, so their
/// recolours may be redistributed; any other base keeps its own licence.
pub const PROVENANCE: &str = "; theme-profile: recoloured from ";
const SHIPPED_TAG: &str = " (ships with Kvantum)";
const SHIPPED_ROOTS: &[&str] = &["/usr/share/Kvantum", "/usr/local/share/Kvantum"];

/// (base theme, ships with Kvantum) of a generated kvconfig.
pub fn provenance(kvconfig: &str) -> Option<(String, bool)> {
    let rest = kvconfig.lines().next()?.strip_prefix(PROVENANCE)?;
    Some(match rest.strip_suffix(SHIPPED_TAG) {
        Some(base) => (base.to_string(), true),
        None => (rest.to_string(), false),
    })
}

/// Kvantum draws tooltips of a theme without a `[ToolTip]` section from its
/// built-in default theme (a glossy black panel). Give such a theme the look
/// of its own popup menus instead, when its SVG has them.
fn with_tooltip_panel(kvconfig: &str, svg: &str) -> String {
    if kvconfig.lines().any(|l| l.trim() == "[ToolTip]") || !svg.contains("id=\"menu-normal\"") {
        return kvconfig.to_string();
    }
    let sep = if kvconfig.ends_with("\n\n") { "" } else if kvconfig.ends_with('\n') { "\n" } else { "\n\n" };
    format!(
        "{kvconfig}{sep}[ToolTip]\nframe=true\nframe.element=menu\ninterior=true\ninterior.element=menu\n\
         frame.top=3\nframe.bottom=3\nframe.left=3\nframe.right=3\n"
    )
}

/// Default base theme for a palette variant.
pub fn default_base(p: &Palette) -> &'static str {
    match p.variant {
        Variant::Dark => "KvArcDark",
        Variant::Light => "KvArc",
    }
}

/// Directory of an installed Kvantum theme (user themes win over system ones).
pub fn find_theme(name: &str) -> Option<PathBuf> {
    find_theme_in(name, &search_roots())
}

fn search_roots() -> Vec<PathBuf> {
    vec![
        user_root(),
        PathBuf::from("/usr/share/Kvantum"),
        PathBuf::from("/usr/local/share/Kvantum"),
    ]
}

/// Where Kvantum looks for user themes (and where generated ones go).
fn user_root() -> PathBuf {
    crate::paths::config_home().join("Kvantum")
}

fn find_theme_in(name: &str, roots: &[PathBuf]) -> Option<PathBuf> {
    if name.is_empty() || name.contains('/') || name.starts_with('.') {
        return None;
    }
    roots
        .iter()
        .map(|root| root.join(name))
        .find(|d| d.join(format!("{name}.kvconfig")).is_file())
}

/// Build `~/.config/Kvantum/ThemeProfile<Name>/` from `base` (an installed
/// theme name) and return (theme id, directory).
pub fn build(p: &Palette, base: &str) -> Result<(String, PathBuf)> {
    build_in(p, base, &search_roots(), &user_root())
}

fn build_in(p: &Palette, base: &str, roots: &[PathBuf], out_root: &Path) -> Result<(String, PathBuf)> {
    let base_dir =
        find_theme_in(base, roots).ok_or_else(|| anyhow!("Kvantum theme {base:?} is not installed"))?;
    let base_kv = std::fs::read_to_string(base_dir.join(format!("{base}.kvconfig")))
        .with_context(|| format!("cannot read {base}.kvconfig"))?;
    let anchor = |key: &str| {
        general_color(&base_kv, key).ok_or_else(|| anyhow!("{base}.kvconfig has no [GeneralColors] {key}"))
    };
    let bg = anchor("window.color")?;
    let fg = general_color(&base_kv, "window.text.color").map_or_else(|| anchor("text.color"), Ok)?;
    let hl = anchor("highlight.color")?;
    let recolor = Recolor::new(bg, fg, hl, p);

    let id = kde_scheme_id(p);
    let target = out_root.join(&id);
    let staging = target.with_extension("tmp-theme-profile");
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    std::fs::create_dir_all(&staging)?;
    for entry in std::fs::read_dir(&base_dir)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let file = path.file_name().expect("file").to_string_lossy().to_string();
        let renamed = match file.strip_prefix(base) {
            Some(rest) if rest.starts_with('.') => format!("{id}{rest}"),
            _ => file.clone(),
        };
        let dest = staging.join(&renamed);
        if file.ends_with(".kvconfig") {
            let text = std::fs::read_to_string(&path)?;
            let shipped = SHIPPED_ROOTS.iter().any(|root| base_dir.starts_with(root));
            let note = format!("{PROVENANCE}{base}{}\n", if shipped { SHIPPED_TAG } else { "" });
            let svg = std::fs::read_to_string(base_dir.join(format!("{base}.svg"))).unwrap_or_default();
            let kv = with_tooltip_panel(&recolor_kvconfig(&text, &recolor, p), &svg);
            std::fs::write(&dest, note + &kv)?;
        } else if file.ends_with(".svg") {
            let text = std::fs::read_to_string(&path)?;
            std::fs::write(&dest, recolor.map_text(&text))?;
        } else {
            std::fs::copy(&path, &dest)?;
        }
    }
    if target.exists() {
        std::fs::remove_dir_all(&target)?;
    }
    std::fs::rename(&staging, &target)?;
    Ok((id, target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::parse_base16;

    const EVERFOREST: &str = include_str!("../data/base16/everforest.yaml");

    fn everforest() -> Palette {
        let mut p = parse_base16(EVERFOREST, "t").unwrap();
        p.set("accent", "#a7c080").unwrap();
        p
    }

    fn arc_dark(p: &Palette) -> Recolor {
        Recolor::new(Rgb::parse("#383c4a").unwrap(), Rgb(255, 255, 255), Rgb::parse("#5294e2").unwrap(), p)
    }

    #[test]
    fn oklab_roundtrip() {
        for c in [Rgb(0, 0, 0), Rgb(255, 255, 255), Rgb(45, 53, 59), Rgb(167, 192, 128), Rgb(230, 126, 128)] {
            let back = from_oklab(oklab(c));
            assert!((i16::from(back.0) - i16::from(c.0)).abs() <= 1, "{c:?} → {back:?}");
            assert!((i16::from(back.2) - i16::from(c.2)).abs() <= 1, "{c:?} → {back:?}");
        }
    }

    #[test]
    fn anchors_map_to_palette_roles() {
        let p = everforest();
        let r = arc_dark(&p);
        let roles = kde_roles(&p);
        let close = |a: Rgb, b: Rgb| (oklab(a).l - oklab(b).l).abs() < 0.01;
        assert!(close(r.map(Rgb::parse("#383c4a").unwrap()), roles.window));
        assert!(close(r.map(Rgb(255, 255, 255)), roles.text));
        assert_eq!(r.map(Rgb::parse("#5294e2").unwrap()).hex(), "#a7c080", "base highlight → accent");
        // Darker than the base window stays darker than our window (depth kept).
        assert!(oklab(r.map(Rgb::parse("#2f343f").unwrap())).l < oklab(roles.window).l);
        // A slightly different blue follows the accent hue.
        let near = oklab(r.map(Rgb::parse("#4a90d9").unwrap()));
        assert!(hue_distance(near.hue(), oklab(roles.accent).hue()) < 5.0);
        // Semantic red is kept.
        assert_eq!(r.map(Rgb::parse("#cc575d").unwrap()).hex(), "#cc575d");
    }

    #[test]
    fn map_text_keeps_alpha_ids_and_short_forms() {
        let p = everforest();
        let r = arc_dark(&p);
        let svg = r##"<g style="fill:#5294e2;stroke:#fff"><use xlink:href="#defs5"/><x fill="#ffffffc8"/></g>"##;
        let out = r.map_text(svg);
        assert!(out.contains("fill:#a7c080;"));
        assert!(out.contains("#defs5"), "ids are not colours");
        assert!(out.contains(&format!("fill=\"{}c8\"", r.map(Rgb(255, 255, 255)).hex())));
        assert!(!out.contains("#fff;") && !out.contains("#fff\""), "short form mapped: {out}");
    }

    #[test]
    fn kvconfig_roles_are_explicit() {
        let p = everforest();
        let r = arc_dark(&p);
        let kv = "[%General]\nauthor=x\n\n[GeneralColors]\nwindow.color=#383c4a\nbase.color=#404552\nhighlight.color=#5294e2\ntext.color=#ffffffc8\ndark.color=black\nhighlight.text.color=white\ntooltip.text.color=white\n\n[Hacks]\nx=1\n";
        let out = recolor_kvconfig(kv, &r, &p);
        let roles = kde_roles(&p);
        assert!(out.contains(&format!("window.color={}\n", roles.window.hex())));
        assert!(out.contains(&format!("base.color={}\n", roles.view.hex())));
        assert!(out.contains("highlight.color=#a7c080\n"));
        assert!(out.contains(&format!("highlight.text.color={}\n", roles.selection_text.hex())));
        assert!(out.contains(&format!("tooltip.text.color={}\n", roles.text.hex())));
        assert!(out.contains(&format!("tooltip.base.color={}\n", roles.window_alt.hex())), "missing role keys are added: {out}");
        assert!(out.contains("text.color=#") && out.contains("c8\n"), "alpha kept: {out}");
        assert!(!out.contains("dark.color=black"), "named colours mapped");
        assert!(out.contains("[%General]\nauthor=x\n") && out.contains("[Hacks]\nx=1\n"));

        // Pressed/toggled text that equals the base highlight text → on-accent text.
        let kv = "[GeneralColors]\nhighlight.text.color=white\n\n[PanelButtonCommand]\ntext.toggle.color=white\ntext.press.color=#ffffff96\n";
        let out = recolor_kvconfig(kv, &r, &p);
        assert!(out.contains(&format!("text.toggle.color={}\n", roles.selection_text.hex())));
        assert!(!out.contains(&format!("text.press.color={}\n", roles.selection_text.hex())), "other contexts keep the mapping");
    }

    #[test]
    fn build_recolours_an_installed_theme() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("Kvantum/FakeBase");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("FakeBase.kvconfig"),
            "[GeneralColors]\nwindow.color=#383c4a\nwindow.text.color=#ffffff\nhighlight.color=#5294e2\n").unwrap();
        std::fs::write(base.join("FakeBase.svg"), "<rect style=\"fill:#383c4a\"/><rect fill=\"#5294e2\"/>").unwrap();
        std::fs::write(base.join("preview.png"), b"png").unwrap();
        let roots = vec![tmp.path().join("Kvantum")];
        let p = everforest();
        let (id, dir) = build_in(&p, "FakeBase", &roots, &roots[0]).unwrap();
        assert_eq!(id, "ThemeProfileEverforest");
        let svg = std::fs::read_to_string(dir.join("ThemeProfileEverforest.svg")).unwrap();
        assert!(svg.contains("#a7c080") && !svg.contains("#5294e2"));
        let kv = std::fs::read_to_string(dir.join("ThemeProfileEverforest.kvconfig")).unwrap();
        assert_eq!(provenance(&kv), Some(("FakeBase".to_string(), false)), "a base outside Kvantum's own themes is not marked GPL");
        assert_eq!(provenance("; theme-profile: recoloured from KvRoughGlass (ships with Kvantum)\n[x]\n"), Some(("KvRoughGlass".to_string(), true)));
        assert_eq!(provenance("[%General]\n"), None);
        assert!(dir.join("preview.png").is_file());
        assert!(!kv.contains("[ToolTip]"), "no menu artwork, no tooltip section");
        let with = with_tooltip_panel("[GeneralColors]\nx=1\n", "<g id=\"menu-normal\"/>");
        assert!(with.ends_with("[ToolTip]\nframe=true\nframe.element=menu\ninterior=true\ninterior.element=menu\nframe.top=3\nframe.bottom=3\nframe.left=3\nframe.right=3\n"), "{with}");
        assert_eq!(with_tooltip_panel("[ToolTip]\nframe=true\n", "<g id=\"menu-normal\"/>"), "[ToolTip]\nframe=true\n");
        assert!(build_in(&p, "../etc", &roots, &roots[0]).is_err());
        // Rebuilding replaces the previous output cleanly.
        build_in(&p, "FakeBase", &roots, &roots[0]).unwrap();
        assert!(dir.join("ThemeProfileEverforest.svg").is_file());
    }
}
