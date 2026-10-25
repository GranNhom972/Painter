//! RGBA colours (straight, non-premultiplied alpha) and blending.

use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl fmt::Debug for Rgba {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl Rgba {
    pub const TRANSPARENT: Rgba = Rgba::new(0, 0, 0, 0);
    pub const BLACK: Rgba = Rgba::rgb(0, 0, 0);
    pub const WHITE: Rgba = Rgba::rgb(255, 255, 255);

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Rgba { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Rgba { r, g, b, a: 255 }
    }

    pub const fn to_array(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }

    pub const fn from_array(p: [u8; 4]) -> Self {
        Rgba::new(p[0], p[1], p[2], p[3])
    }

    /// Parses `#rgb`, `#rrggbb` or `#rrggbbaa` (the `#` is optional).
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('#');
        let hex = |i: usize, n: usize| u8::from_str_radix(s.get(i..i + n)?, 16).ok();
        match s.len() {
            3 => {
                let c = |i| hex(i, 1).map(|v| v * 17);
                Some(Rgba::rgb(c(0)?, c(1)?, c(2)?))
            }
            6 => Some(Rgba::rgb(hex(0, 2)?, hex(2, 2)?, hex(4, 2)?)),
            8 => Some(Rgba::new(hex(0, 2)?, hex(2, 2)?, hex(4, 2)?, hex(6, 2)?)),
            _ => None,
        }
    }

    /// `#rrggbb` for opaque colours, `#rrggbbaa` otherwise.
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    pub fn with_alpha(self, a: u8) -> Self {
        Rgba { a, ..self }
    }

    /// Multiplies the alpha channel by a coverage in 0..=255.
    pub fn scale_alpha(self, coverage: u8) -> Self {
        let a = (self.a as u32 * coverage as u32 + 127) / 255;
        Rgba { a: a as u8, ..self }
    }

    /// Largest per-channel difference (alpha included); used for fill tolerance.
    pub fn distance(self, o: Rgba) -> u8 {
        let d = |a: u8, b: u8| a.abs_diff(b);
        d(self.r, o.r).max(d(self.g, o.g)).max(d(self.b, o.b)).max(d(self.a, o.a))
    }

    /// `self` composited over `dst`.
    pub fn over(self, dst: Rgba) -> Rgba {
        Rgba::from_array(blend_over(dst.to_array(), self.to_array()))
    }

    /// Hue in degrees [0, 360), saturation and value in [0, 1].
    pub fn to_hsv(self) -> (f32, f32, f32) {
        let r = self.r as f32 / 255.0;
        let g = self.g as f32 / 255.0;
        let b = self.b as f32 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;
        let h = if d == 0.0 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / d).rem_euclid(6.0))
        } else if max == g {
            60.0 * ((b - r) / d + 2.0)
        } else {
            60.0 * ((r - g) / d + 4.0)
        };
        let s = if max == 0.0 { 0.0 } else { d / max };
        (h, s, max)
    }

    pub fn from_hsv(h: f32, s: f32, v: f32) -> Self {
        let h = h.rem_euclid(360.0);
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;
        let (r, g, b) = match (h / 60.0) as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let q = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
        Rgba::rgb(q(r), q(g), q(b))
    }
}

/// Porter-Duff "source over" on straight-alpha pixels.
#[inline]
pub fn blend_over(dst: [u8; 4], src: [u8; 4]) -> [u8; 4] {
    let sa = src[3] as u32;
    if sa == 0 {
        return dst;
    }
    if sa == 255 {
        return src;
    }
    let da = dst[3] as u32;
    let inv = 255 - sa;
    let out_a = sa * 255 + da * inv; // alpha * 255
    if out_a == 0 {
        return [0, 0, 0, 0];
    }
    let ch = |i: usize| ((src[i] as u32 * sa * 255 + dst[i] as u32 * da * inv + out_a / 2) / out_a) as u8;
    [ch(0), ch(1), ch(2), ((out_a + 127) / 255) as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        assert_eq!(Rgba::from_hex("#ff8000"), Some(Rgba::rgb(255, 128, 0)));
        assert_eq!(Rgba::from_hex("ff800080"), Some(Rgba::new(255, 128, 0, 128)));
        assert_eq!(Rgba::from_hex("#fff"), Some(Rgba::WHITE));
        assert_eq!(Rgba::from_hex("zzz"), None);
        assert_eq!(Rgba::from_hex("#12345"), None);
        assert_eq!(Rgba::rgb(1, 2, 3).to_hex(), "#010203");
        assert_eq!(Rgba::new(1, 2, 3, 4).to_hex(), "#01020304");
    }

    #[test]
    fn blending() {
        let red = Rgba::rgb(255, 0, 0);
        assert_eq!(red.over(Rgba::WHITE), red);
        assert_eq!(Rgba::TRANSPARENT.over(Rgba::WHITE), Rgba::WHITE);
        let half = red.with_alpha(128).over(Rgba::WHITE);
        assert_eq!(half.a, 255);
        assert!(half.r == 255 && (126..=128).contains(&half.g));
        // Blending onto a transparent pixel keeps the source colour.
        let p = red.with_alpha(100).over(Rgba::TRANSPARENT);
        assert_eq!(p, red.with_alpha(100));
    }

    #[test]
    fn hsv_roundtrip() {
        for c in [Rgba::rgb(255, 0, 0), Rgba::rgb(34, 177, 76), Rgba::rgb(63, 72, 204), Rgba::rgb(200, 191, 231), Rgba::BLACK, Rgba::WHITE] {
            let (h, s, v) = c.to_hsv();
            let back = Rgba::from_hsv(h, s, v);
            assert!(c.distance(back) <= 1, "{c:?} -> {back:?}");
        }
    }

    #[test]
    fn distance_and_alpha() {
        assert_eq!(Rgba::rgb(10, 20, 30).distance(Rgba::rgb(15, 20, 25)), 5);
        assert_eq!(Rgba::WHITE.scale_alpha(128).a, 128);
        assert_eq!(Rgba::WHITE.with_alpha(128).scale_alpha(255).a, 128);
    }
}
