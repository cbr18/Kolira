//! OKLab colour space (Björn Ottosson): perceptual lightness/chroma/hue used
//! by the Kvantum recolouring and the wallpaper palette extraction.

use crate::palette::Rgb;

#[derive(Debug, Clone, Copy)]
pub struct Lab {
    pub l: f64,
    pub a: f64,
    pub b: f64,
}

impl Lab {
    pub fn chroma(self) -> f64 {
        self.a.hypot(self.b)
    }
    pub fn hue(self) -> f64 {
        self.b.atan2(self.a).to_degrees()
    }
}

fn to_linear(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn from_linear(c: f64) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

pub fn oklab(c: Rgb) -> Lab {
    let (r, g, b) = (to_linear(c.0), to_linear(c.1), to_linear(c.2));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    Lab {
        l: 0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        a: 1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        b: 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    }
}

pub fn from_oklab(c: Lab) -> Rgb {
    let l = (c.l + 0.396_337_777_4 * c.a + 0.215_803_757_3 * c.b).powi(3);
    let m = (c.l - 0.105_561_345_8 * c.a - 0.063_854_172_8 * c.b).powi(3);
    let s = (c.l - 0.089_484_177_5 * c.a - 1.291_485_548 * c.b).powi(3);
    Rgb(
        from_linear(4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s),
        from_linear(-1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s),
        from_linear(-0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s),
    )
}

pub fn hue_distance(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

impl Lab {
    /// Build from lightness, chroma and hue (degrees).
    pub fn lch(l: f64, chroma: f64, hue_deg: f64) -> Lab {
        let h = hue_deg.to_radians();
        Lab {
            l,
            a: chroma * h.cos(),
            b: chroma * h.sin(),
        }
    }
}
