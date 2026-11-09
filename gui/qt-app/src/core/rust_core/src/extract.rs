//! Palette from a wallpaper.
//!
//! Two front-ends, one rule: the CLI clusters the image itself
//! ([`swatches_from_image`], k-means in OKLab), the GUI passes Plasma's own
//! `Kirigami.ImageColors` swatches (`palette from-colors`). Both feed
//! [`palette_from_swatches`], which turns weighted colours into base16:
//!
//! * background ramp `base00…base07`: hue of the dominant colour, low chroma,
//!   fixed lightness steps (dark or light by the image's mean lightness);
//! * accent: the most prominent colourful cluster (weight × chroma) outside
//!   the background family, brought to a readable lightness;
//! * `base08…base0F`: canonical hues (red, orange, yellow, green, cyan, blue,
//!   magenta, brown) snapped to a nearby image hue when there is one, so the
//!   semantic colours stay recognisable but harmonise with the picture;
//! * text (`base05`) is pushed until it reaches 7:1 contrast on `base00`.

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use crate::oklab::{from_oklab, hue_distance, oklab, Lab};
use crate::palette::{contrast, Palette, Rgb, Variant, SLOTS};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Swatch {
    pub color: Rgb,
    /// Share of the image (any positive scale; normalised internally).
    pub weight: f64,
}

/// Minimum OKLab chroma for a colour to count as "colourful".
const COLOURFUL: f64 = 0.04;
const SNAP_RANGE: f64 = 25.0;
/// OKLab distance under which a cluster belongs to the background family.
const BACKGROUND_FAMILY: f64 = 0.1;
/// Canonical hues (degrees, OKLab) for base08…base0F and a lightness offset.
const SEMANTIC: [(f64, f64); 8] = [
    (25.0, 0.0),   // base08 red
    (55.0, 0.0),   // base09 orange
    (95.0, 0.03),  // base0A yellow
    (140.0, 0.0),  // base0B green
    (195.0, 0.0),  // base0C cyan
    (255.0, 0.0),  // base0D blue
    (330.0, 0.0),  // base0E magenta
    (60.0, -0.15), // base0F brown
];

/// k-means (k ≤ 8) on a ≤96 px thumbnail; deterministic (quantile seeding).
pub fn swatches_from_image(path: &Path) -> Result<Vec<Swatch>> {
    let image = image::open(path).with_context(|| format!("cannot decode {}", path.display()))?;
    let thumb = image.thumbnail(96, 96).to_rgb8();
    let points: Vec<Lab> = thumb
        .pixels()
        .map(|p| oklab(Rgb(p[0], p[1], p[2])))
        .collect();
    if points.is_empty() {
        return Err(anyhow!("{} has no pixels", path.display()));
    }
    Ok(kmeans(&points, 8)
        .into_iter()
        .map(|(lab, weight)| Swatch {
            color: from_oklab(lab),
            weight,
        })
        .collect())
}

fn dist2(a: Lab, b: Lab) -> f64 {
    (a.l - b.l).powi(2) + (a.a - b.a).powi(2) + (a.b - b.b).powi(2)
}

fn kmeans(points: &[Lab], k: usize) -> Vec<(Lab, f64)> {
    let k = k.min(points.len()).max(1);
    // Seed with lightness quantiles: deterministic and spread out.
    let mut sorted = points.to_vec();
    sorted.sort_by(|a, b| a.l.total_cmp(&b.l));
    let mut centers: Vec<Lab> = (0..k)
        .map(|i| sorted[(i * 2 + 1) * sorted.len() / (2 * k)])
        .collect();
    let mut assign = vec![0usize; points.len()];
    for _ in 0..16 {
        for (i, p) in points.iter().enumerate() {
            assign[i] = (0..k)
                .min_by(|&a, &b| dist2(*p, centers[a]).total_cmp(&dist2(*p, centers[b])))
                .expect("k ≥ 1");
        }
        let mut sums = vec![(0.0, 0.0, 0.0, 0usize); k];
        for (p, &c) in points.iter().zip(&assign) {
            sums[c].0 += p.l;
            sums[c].1 += p.a;
            sums[c].2 += p.b;
            sums[c].3 += 1;
        }
        for (c, (l, a, b, n)) in sums.iter().enumerate() {
            if *n > 0 {
                let n = *n as f64;
                centers[c] = Lab {
                    l: l / n,
                    a: a / n,
                    b: b / n,
                };
            }
        }
    }
    let total = points.len() as f64;
    let mut out: Vec<(Lab, f64)> = (0..k)
        .map(|c| {
            (
                centers[c],
                assign.iter().filter(|&&a| a == c).count() as f64 / total,
            )
        })
        .filter(|(_, w)| *w > 0.0)
        .collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out
}

/// Turn weighted colours into a base16 palette (see module docs).
pub fn palette_from_swatches(
    swatches: &[Swatch],
    variant: Option<Variant>,
    name: &str,
    source: &str,
) -> Result<Palette> {
    let swatches: Vec<(Lab, f64)> = swatches
        .iter()
        .filter(|s| s.weight.is_finite() && s.weight > 0.0)
        .map(|s| (oklab(s.color), s.weight))
        .collect();
    if swatches.is_empty() {
        return Err(anyhow!("no colours to build a palette from"));
    }
    let total: f64 = swatches.iter().map(|(_, w)| w).sum();
    let mean_l = swatches.iter().map(|(c, w)| c.l * w).sum::<f64>() / total;
    let variant = variant.unwrap_or(if mean_l < 0.62 {
        Variant::Dark
    } else {
        Variant::Light
    });
    let dark = variant == Variant::Dark;

    let dominant = swatches
        .iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .expect("non-empty")
        .0;
    let bg_hue = dominant.hue();
    let bg_chroma = (dominant.chroma() * 0.35).min(0.035);

    let ramp_l: [f64; 8] = if dark {
        [0.24, 0.29, 0.36, 0.56, 0.66, 0.86, 0.92, 0.97]
    } else {
        [0.97, 0.92, 0.86, 0.66, 0.56, 0.36, 0.29, 0.24]
    };
    let ramp_c: [f64; 8] = [1.0, 1.0, 0.9, 0.6, 0.5, 0.35, 0.25, 0.15];
    let mut base: Vec<Rgb> = (0..8)
        .map(|i| from_oklab(Lab::lch(ramp_l[i], bg_chroma * ramp_c[i], bg_hue)))
        .collect();

    // Text: at least 7:1 on the background.
    let mut text_l = ramp_l[5];
    while contrast(base[5], base[0]) < 7.0 && (0.0..=1.0).contains(&text_l) {
        text_l += if dark { 0.02 } else { -0.02 };
        base[5] = from_oklab(Lab::lch(
            text_l.clamp(0.0, 1.0),
            bg_chroma * ramp_c[5],
            bg_hue,
        ));
    }

    let mut colourful: Vec<(Lab, f64)> = swatches
        .iter()
        .copied()
        .filter(|(c, _)| c.chroma() >= COLOURFUL)
        .collect();
    colourful.sort_by(|a, b| (b.1 * b.0.chroma()).total_cmp(&(a.1 * a.0.chroma())));
    let mut chromas: Vec<f64> = colourful.iter().map(|(c, _)| c.chroma()).collect();
    chromas.sort_by(f64::total_cmp);
    let accent_chroma = chromas
        .get(chromas.len() / 2)
        .copied()
        .unwrap_or(0.11)
        .clamp(0.08, 0.14);
    let accent_l = if dark { 0.74 } else { 0.52 };

    for (hue, l_offset) in SEMANTIC {
        let snapped = colourful
            .iter()
            .map(|(c, _)| c.hue())
            .filter(|h| hue_distance(*h, hue) <= SNAP_RANGE)
            .min_by(|a, b| hue_distance(*a, hue).total_cmp(&hue_distance(*b, hue)))
            .unwrap_or(hue);
        base.push(from_oklab(Lab::lch(
            (accent_l + l_offset).clamp(0.0, 1.0),
            accent_chroma,
            snapped,
        )));
    }

    // Accent candidates exclude the background family (clusters close to the
    // dominant colour): a darker shade of the backdrop is not a highlight.
    let accent_pick = colourful
        .iter()
        .find(|(c, _)| dist2(*c, dominant).sqrt() >= BACKGROUND_FAMILY)
        .or(colourful.first());
    let accent = match accent_pick {
        Some((c, _)) => from_oklab(Lab::lch(
            if dark { 0.76 } else { 0.55 },
            c.chroma().clamp(0.08, 0.15),
            c.hue(),
        )),
        None => base[0x0D],
    };

    let accent = readable_accent(accent, &base);

    let palette = Palette {
        name: if name.trim().is_empty() {
            "Wallpaper".to_string()
        } else {
            name.trim().to_string()
        },
        author: "theme-profile (generated from wallpaper)".to_string(),
        variant,
        base: base.iter().map(|c| c.hex()).collect(),
        accent: accent.hex(),
        source: source.to_string(),
    };
    debug_assert_eq!(palette.base.len(), SLOTS.len());
    palette.validate()?;
    Ok(palette)
}

/// Nudge the accent's lightness (smallest change first, either direction)
/// until some text slot reads on it at WCAG AA — selected text sits there.
fn readable_accent(accent: Rgb, base: &[Rgb]) -> Rgb {
    let best = |c: Rgb| {
        [base[0x00], base[0x07], base[0x05]]
            .iter()
            .map(|t| contrast(*t, c))
            .fold(0.0, f64::max)
    };
    if best(accent) >= crate::palette::MIN_TEXT_CONTRAST {
        return accent;
    }
    let lab = oklab(accent);
    for step in 1..=15 {
        for sign in [-1.0, 1.0] {
            let l = (lab.l + sign * 0.02 * f64::from(step)).clamp(0.0, 1.0);
            let candidate = from_oklab(Lab { l, ..lab });
            if best(candidate) >= crate::palette::MIN_TEXT_CONTRAST {
                return candidate;
            }
        }
    }
    accent
}

/// Palette straight from an image file (CLI path).
pub fn palette_from_image(
    path: &Path,
    variant: Option<Variant>,
    name: Option<&str>,
) -> Result<Palette> {
    let swatches = swatches_from_image(path)?;
    let default_name = path
        .file_stem()
        .map(|s| {
            format!(
                "Wallpaper {}",
                s.to_string_lossy().chars().take(24).collect::<String>()
            )
        })
        .unwrap_or_else(|| "Wallpaper".to_string());
    palette_from_swatches(
        &swatches,
        variant,
        name.unwrap_or(&default_name),
        &format!("wallpaper:{}", path.display()),
    )
}

/// JSON accepted by `palette from-colors` (shape of `Kirigami.ImageColors.palette`).
#[derive(Debug, Deserialize)]
pub struct ColorsInput {
    pub colors: Vec<ColorInput>,
    #[serde(default)]
    pub variant: Option<Variant>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ColorInput {
    pub color: String,
    #[serde(default = "one")]
    pub ratio: f64,
}

fn one() -> f64 {
    1.0
}

pub fn palette_from_colors_json(json: &str) -> Result<Palette> {
    let input: ColorsInput = serde_json::from_str(json).context("invalid colours JSON")?;
    let swatches: Vec<Swatch> = input
        .colors
        .iter()
        .map(|c| {
            Ok(Swatch {
                color: Rgb::parse(c.color.get(..7).unwrap_or(&c.color))?,
                weight: c.ratio,
            })
        })
        .collect::<Result<_>>()?;
    palette_from_swatches(
        &swatches,
        input.variant,
        input.name.as_deref().unwrap_or("Wallpaper"),
        input.source.as_deref().unwrap_or("imagecolors"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::MIN_TEXT_CONTRAST;

    fn sw(hex: &str, weight: f64) -> Swatch {
        Swatch {
            color: Rgb::parse(hex).unwrap(),
            weight,
        }
    }

    fn hue_of(hex: &str) -> f64 {
        oklab(Rgb::parse(hex).unwrap()).hue()
    }

    #[test]
    fn dark_forest_image_gives_dark_green_tinted_palette() {
        let swatches = [
            sw("#1f3326", 0.55),
            sw("#2e4a35", 0.25),
            sw("#8fb573", 0.12),
            sw("#d0894a", 0.08),
        ];
        let p = palette_from_swatches(&swatches, None, "Forest", "test").unwrap();
        assert_eq!(p.variant, Variant::Dark);
        // Background keeps the dominant (green) hue, muted.
        assert!(hue_distance(hue_of(&p.base[0]), hue_of("#1f3326")) < 20.0);
        assert!(oklab(Rgb::parse(&p.base[0]).unwrap()).chroma() <= 0.036);
        // Accent: most prominent colourful cluster (green), readable.
        assert!(hue_distance(hue_of(&p.accent), hue_of("#8fb573")) < 10.0);
        // Orange slot snaps to the image's orange.
        assert!(hue_distance(hue_of(&p.base[0x09]), hue_of("#d0894a")) < 3.0);
        // Red stays red-ish (no image hue nearby → canonical).
        assert!(hue_distance(hue_of(&p.base[0x08]), 25.0) < 3.0);
        assert!(contrast(p.slot(5), p.slot(0)) >= 7.0);
        assert!(
            p.contrast_warnings().is_empty(),
            "{:?}",
            p.contrast_warnings()
        );
    }

    #[test]
    fn bright_image_gives_light_palette_and_grey_image_still_works() {
        let p = palette_from_swatches(&[sw("#f2efe6", 0.7), sw("#c9c2b0", 0.3)], None, "", "t")
            .unwrap();
        assert_eq!(p.variant, Variant::Light);
        assert_eq!(p.name, "Wallpaper");
        assert!(contrast(p.slot(5), p.slot(0)) >= MIN_TEXT_CONTRAST);
        // No colourful cluster → accent falls back to the blue slot.
        assert_eq!(p.accent, p.base[0x0D]);
        let forced =
            palette_from_swatches(&[sw("#f2efe6", 1.0)], Some(Variant::Dark), "x", "t").unwrap();
        assert_eq!(forced.variant, Variant::Dark);
        assert!(palette_from_swatches(&[], None, "x", "t").is_err());
    }

    #[test]
    fn accent_is_nudged_until_selected_text_reads() {
        // Watercolour-like light image with an olive accent (was 4.4:1).
        let swatches = [
            sw("#efeadb", 0.5),
            sw("#d8d3bd", 0.2),
            sw("#8a8a45", 0.15),
            sw("#5f8f99", 0.15),
        ];
        let p = palette_from_swatches(&swatches, None, "Light", "t").unwrap();
        assert_eq!(p.variant, Variant::Light);
        assert!(
            p.contrast_warnings().is_empty(),
            "{:?}",
            p.contrast_warnings()
        );
    }

    #[test]
    fn colors_json_accepts_imagecolors_shape() {
        let json = r##"{"colors":[{"color":"#1f3326","ratio":0.6},{"color":"#8fb573ff","ratio":0.2},{"color":"#d0894a"}],
                       "name":"From Plasma","source":"imagecolors:/w.jpg"}"##;
        let p = palette_from_colors_json(json).unwrap();
        assert_eq!(p.name, "From Plasma");
        assert_eq!(p.source, "imagecolors:/w.jpg");
        assert!(palette_from_colors_json(r#"{"colors":[{"color":"nope"}]}"#).is_err());
    }

    #[test]
    fn image_file_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("forest.png");
        let img = image::RgbImage::from_fn(64, 64, |x, _| {
            if x < 44 {
                image::Rgb([31, 51, 38])
            } else if x < 56 {
                image::Rgb([143, 181, 115])
            } else {
                image::Rgb([208, 137, 74])
            }
        });
        img.save(&path).unwrap();
        let swatches = swatches_from_image(&path).unwrap();
        assert!((swatches.iter().map(|s| s.weight).sum::<f64>() - 1.0).abs() < 1e-9);
        let p = palette_from_image(&path, None, None).unwrap();
        assert_eq!(p.variant, Variant::Dark);
        assert_eq!(p.name, "Wallpaper forest");
        assert!(p.source.starts_with("wallpaper:"));
        assert!(hue_distance(hue_of(&p.accent), hue_of("#8fb573")) < 15.0);
        assert!(swatches_from_image(&tmp.path().join("missing.png")).is_err());
    }
}
