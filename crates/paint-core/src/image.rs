//! RGBA8 raster images, coverage masks and whole-image transforms.

use crate::color::{blend_over, Rgba};
use crate::geom::IRect;
use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl fmt::Debug for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Image({}x{})", self.width, self.height)
    }
}

/// How [`Image::draw`] composites a source image.
#[derive(Clone, Copy, Default)]
pub struct DrawOptions<'a> {
    /// Coverage mask, same size as the source; 0 skips the pixel.
    pub mask: Option<&'a Mask>,
    /// Source pixels of exactly this colour are skipped ("transparent selection").
    pub key: Option<Rgba>,
    /// Copy pixels instead of alpha-blending them.
    pub replace: bool,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Self {
        Image { width, height, data: vec![0; width as usize * height as usize * 4] }
    }

    pub fn filled(width: u32, height: u32, c: Rgba) -> Self {
        let mut img = Image::new(width, height);
        img.fill(c);
        img
    }

    pub fn from_rgba(width: u32, height: u32, data: Vec<u8>) -> Result<Self, String> {
        if data.len() != width as usize * height as usize * 4 {
            return Err(format!("expected {} bytes for {width}x{height}, got {}", width as usize * height as usize * 4, data.len()));
        }
        Ok(Image { width, height, data })
    }

    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn bounds(&self) -> IRect {
        IRect::new(0, 0, self.width as i32, self.height as i32)
    }
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
    pub fn into_raw(self) -> Vec<u8> {
        self.data
    }
    pub fn byte_size(&self) -> usize {
        self.data.len()
    }

    #[inline]
    fn idx(&self, x: i32, y: i32) -> usize {
        (y as usize * self.width as usize + x as usize) * 4
    }

    #[inline]
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height
    }

    /// Pixel at (x, y); transparent outside the image.
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Rgba {
        if !self.in_bounds(x, y) {
            return Rgba::TRANSPARENT;
        }
        let i = self.idx(x, y);
        Rgba::new(self.data[i], self.data[i + 1], self.data[i + 2], self.data[i + 3])
    }

    #[inline]
    pub fn get_raw(&self, x: i32, y: i32) -> [u8; 4] {
        let i = self.idx(x, y);
        [self.data[i], self.data[i + 1], self.data[i + 2], self.data[i + 3]]
    }

    #[inline]
    pub fn set(&mut self, x: i32, y: i32, c: Rgba) {
        if self.in_bounds(x, y) {
            let i = self.idx(x, y);
            self.data[i..i + 4].copy_from_slice(&c.to_array());
        }
    }

    #[inline]
    pub fn set_raw(&mut self, x: i32, y: i32, p: [u8; 4]) {
        let i = self.idx(x, y);
        self.data[i..i + 4].copy_from_slice(&p);
    }

    #[inline]
    pub fn blend(&mut self, x: i32, y: i32, c: Rgba) {
        if self.in_bounds(x, y) {
            let i = self.idx(x, y);
            let dst = [self.data[i], self.data[i + 1], self.data[i + 2], self.data[i + 3]];
            self.data[i..i + 4].copy_from_slice(&blend_over(dst, c.to_array()));
        }
    }

    pub fn fill(&mut self, c: Rgba) {
        let p = c.to_array();
        for px in self.data.as_chunks_mut::<4>().0 {
            px.copy_from_slice(&p);
        }
    }

    /// Replaces every pixel of `r` (clipped) with `c`.
    pub fn fill_rect(&mut self, r: IRect, c: Rgba) {
        let r = r.clamp_to(self.width, self.height);
        let p = c.to_array();
        for y in r.y..r.bottom() {
            let start = self.idx(r.x, y);
            for px in self.data[start..start + r.w as usize * 4].as_chunks_mut::<4>().0 {
                px.copy_from_slice(&p);
            }
        }
    }

    /// Replaces the pixels where `mask` (placed at `x`, `y`) is set.
    pub fn fill_masked(&mut self, x: i32, y: i32, mask: &Mask, c: Rgba) {
        for my in 0..mask.height() as i32 {
            for mx in 0..mask.width() as i32 {
                if mask.get(mx, my) >= 128 {
                    self.set(x + mx, y + my, c);
                }
            }
        }
    }

    /// Copy of the pixels inside `r`; parts outside the image are transparent.
    pub fn sub_image(&self, r: IRect) -> Image {
        let mut out = Image::new(r.w.max(0) as u32, r.h.max(0) as u32);
        let clip = r.clamp_to(self.width, self.height);
        for y in clip.y..clip.bottom() {
            let src = self.idx(clip.x, y);
            let dst = out.idx(clip.x - r.x, y - r.y);
            let n = clip.w as usize * 4;
            out.data[dst..dst + n].copy_from_slice(&self.data[src..src + n]);
        }
        out
    }

    /// Composites `src` with its top-left corner at (`dx`, `dy`).
    pub fn draw(&mut self, src: &Image, dx: i32, dy: i32, opts: &DrawOptions) {
        let target = IRect::new(dx, dy, src.width as i32, src.height as i32).clamp_to(self.width, self.height);
        for y in target.y..target.bottom() {
            for x in target.x..target.right() {
                let (sx, sy) = (x - dx, y - dy);
                let mut p = src.get_raw(sx, sy);
                if let Some(m) = opts.mask {
                    let cov = m.get(sx, sy);
                    if cov == 0 {
                        continue;
                    }
                    if cov < 255 {
                        p[3] = ((p[3] as u32 * cov as u32 + 127) / 255) as u8;
                    }
                }
                if let Some(k) = opts.key {
                    if Rgba::from_array(p) == k {
                        continue;
                    }
                }
                if opts.replace {
                    self.set_raw(x, y, p);
                } else {
                    let d = self.get_raw(x, y);
                    self.set_raw(x, y, blend_over(d, p));
                }
            }
        }
    }

    /// True when every pixel equals `c`.
    pub fn is_uniform(&self, c: Rgba) -> bool {
        let p = c.to_array();
        self.data.as_chunks::<4>().0.iter().all(|px| *px == p)
    }

    pub fn flip_horizontal(&self) -> Image {
        let mut out = Image::new(self.width, self.height);
        for y in 0..self.height as i32 {
            for x in 0..self.width as i32 {
                out.set_raw(self.width as i32 - 1 - x, y, self.get_raw(x, y));
            }
        }
        out
    }

    pub fn flip_vertical(&self) -> Image {
        let mut out = Image::new(self.width, self.height);
        let row = self.width as usize * 4;
        for y in 0..self.height as usize {
            let dst = (self.height as usize - 1 - y) * row;
            out.data[dst..dst + row].copy_from_slice(&self.data[y * row..(y + 1) * row]);
        }
        out
    }

    /// Rotation by 90° clockwise.
    pub fn rotate_cw(&self) -> Image {
        let mut out = Image::new(self.height, self.width);
        for y in 0..self.height as i32 {
            for x in 0..self.width as i32 {
                out.set_raw(self.height as i32 - 1 - y, x, self.get_raw(x, y));
            }
        }
        out
    }

    /// Rotation by 90° counter-clockwise.
    pub fn rotate_ccw(&self) -> Image {
        let mut out = Image::new(self.height, self.width);
        for y in 0..self.height as i32 {
            for x in 0..self.width as i32 {
                out.set_raw(y, self.width as i32 - 1 - x, self.get_raw(x, y));
            }
        }
        out
    }

    pub fn rotate_180(&self) -> Image {
        let mut out = Image::new(self.width, self.height);
        let n = self.data.len() / 4;
        for i in 0..n {
            let j = n - 1 - i;
            out.data[j * 4..j * 4 + 4].copy_from_slice(&self.data[i * 4..i * 4 + 4]);
        }
        out
    }

    /// Same pixels on a canvas of a new size, anchored top-left; new area filled with `bg`.
    pub fn with_canvas_size(&self, w: u32, h: u32, bg: Rgba) -> Image {
        self.with_canvas_at(w, h, 0, 0, bg)
    }

    /// Same pixels placed at (`dx`, `dy`) on a canvas of a new size; new area filled with `bg`.
    pub fn with_canvas_at(&self, w: u32, h: u32, dx: i32, dy: i32, bg: Rgba) -> Image {
        let mut out = Image::filled(w, h, bg);
        out.draw(self, dx, dy, &DrawOptions { replace: true, ..Default::default() });
        out
    }

    /// Resampled copy (triangle filter, premultiplied alpha; acts as a box filter when shrinking).
    pub fn resized(&self, w: u32, h: u32) -> Image {
        let (w, h) = (w.max(1), h.max(1));
        if w == self.width && h == self.height {
            return self.clone();
        }
        if self.width == 0 || self.height == 0 {
            return Image::new(w, h);
        }
        // Premultiplied float buffer.
        let mut pm: Vec<[f32; 4]> = self
            .data
            .as_chunks::<4>().0.iter()
            .map(|p| {
                let a = p[3] as f32 / 255.0;
                [p[0] as f32 * a, p[1] as f32 * a, p[2] as f32 * a, p[3] as f32]
            })
            .collect();
        let horiz = resample_axis(&pm, self.width as usize, self.height as usize, w as usize, true);
        pm = resample_axis(&horiz, w as usize, self.height as usize, h as usize, false);
        let mut out = Image::new(w, h);
        for (i, p) in pm.iter().enumerate() {
            let a = p[3].clamp(0.0, 255.0);
            let px = if a < 0.5 {
                [0, 0, 0, 0]
            } else {
                // Un-premultiply: colour = premultiplied / (alpha / 255).
                let c = |v: f32| (v * 255.0 / a).round().clamp(0.0, 255.0) as u8;
                [c(p[0]), c(p[1]), c(p[2]), a.round() as u8]
            };
            out.data[i * 4..i * 4 + 4].copy_from_slice(&px);
        }
        out
    }

    /// Shears the image by `deg_x` (horizontal) then `deg_y` (vertical) degrees.
    /// The canvas grows to fit and new pixels are `bg`.
    pub fn skewed(&self, deg_x: f32, deg_y: f32, bg: Rgba) -> Image {
        let mut img = self.clone();
        if deg_x.abs() > 0.01 {
            img = img.skew_axis(deg_x, true, bg);
        }
        if deg_y.abs() > 0.01 {
            img = img.skew_axis(deg_y, false, bg);
        }
        img
    }

    fn skew_axis(&self, deg: f32, horizontal: bool, bg: Rgba) -> Image {
        let t = deg.clamp(-89.0, 89.0).to_radians().tan();
        let (w, h) = (self.width as i32, self.height as i32);
        // `along` is the axis that shifts, `across` the one that drives the shift.
        let (along, across) = if horizontal { (w, h) } else { (h, w) };
        let extra = (t.abs() * across as f32).ceil() as i32;
        let base = if t < 0.0 { extra as f32 } else { 0.0 };
        let (ow, oh) = if horizontal { (w + extra, h) } else { (w, h + extra) };
        let mut out = Image::filled(ow as u32, oh as u32, bg);
        for c in 0..across {
            let shift = base + t * (c as f32 + 0.5);
            for o in 0..along + extra {
                let src = o as f32 + 0.5 - shift - 0.5;
                let s0 = src.floor() as i32;
                let f = src - s0 as f32;
                let fetch = |s: i32| -> Option<[u8; 4]> {
                    if s < 0 || s >= along {
                        None
                    } else if horizontal {
                        Some(self.get_raw(s, c))
                    } else {
                        Some(self.get_raw(c, s))
                    }
                };
                let (a, b) = (fetch(s0), fetch(s0 + 1));
                let p = match (a, b) {
                    (None, None) => continue,
                    (Some(a), None) => {
                        if f > 0.5 {
                            continue;
                        }
                        a
                    }
                    (None, Some(b)) => {
                        if f < 0.5 {
                            continue;
                        }
                        b
                    }
                    (Some(a), Some(b)) => {
                        let mut p = [0u8; 4];
                        for i in 0..4 {
                            p[i] = (a[i] as f32 * (1.0 - f) + b[i] as f32 * f).round() as u8;
                        }
                        p
                    }
                };
                if horizontal {
                    out.set_raw(o, c, p);
                } else {
                    out.set_raw(c, o, p);
                }
            }
        }
        out
    }

    /// Downscaled copy whose longest side is at most `max` pixels.
    pub fn thumbnail(&self, max: u32) -> Image {
        let longest = self.width.max(self.height).max(1);
        if longest <= max {
            return self.clone();
        }
        let k = max as f32 / longest as f32;
        self.resized(((self.width as f32 * k).round() as u32).max(1), ((self.height as f32 * k).round() as u32).max(1))
    }
}

/// Geometric operations shared by whole-image and selection transforms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Transform {
    RotateCw,
    RotateCcw,
    Rotate180,
    FlipHorizontal,
    FlipVertical,
    Resize(u32, u32),
    Skew(f32, f32),
}

impl Transform {
    /// Applies the transform; `bg` fills the area uncovered by a skew.
    pub fn apply(&self, img: &Image, bg: Rgba) -> Image {
        match *self {
            Transform::RotateCw => img.rotate_cw(),
            Transform::RotateCcw => img.rotate_ccw(),
            Transform::Rotate180 => img.rotate_180(),
            Transform::FlipHorizontal => img.flip_horizontal(),
            Transform::FlipVertical => img.flip_vertical(),
            Transform::Resize(w, h) => img.resized(w, h),
            Transform::Skew(x, y) => img.skewed(x, y, bg),
        }
    }

    /// Maps a point given relative to the image box (0..1); `None` for non-affine-in-box transforms.
    pub fn map_relative(&self, p: crate::geom::Pt) -> Option<crate::geom::Pt> {
        use crate::geom::Pt;
        Some(match self {
            Transform::RotateCw => Pt::new(1.0 - p.y, p.x),
            Transform::RotateCcw => Pt::new(p.y, 1.0 - p.x),
            Transform::Rotate180 => Pt::new(1.0 - p.x, 1.0 - p.y),
            Transform::FlipHorizontal => Pt::new(1.0 - p.x, p.y),
            Transform::FlipVertical => Pt::new(p.x, 1.0 - p.y),
            Transform::Resize(..) => p,
            Transform::Skew(..) => return None,
        })
    }

    /// Whether the transform swaps width and height.
    pub fn swaps_axes(&self) -> bool {
        matches!(self, Transform::RotateCw | Transform::RotateCcw)
    }
}

/// One-dimensional resampling pass over a premultiplied buffer.
fn resample_axis(src: &[[f32; 4]], sw: usize, sh: usize, n: usize, horizontal: bool) -> Vec<[f32; 4]> {
    let len = if horizontal { sw } else { sh };
    let scale = n as f32 / len as f32;
    let support = if scale < 1.0 { 1.0 / scale } else { 1.0 };
    // Precompute weights per output coordinate.
    let weights: Vec<(usize, Vec<f32>)> = (0..n)
        .map(|o| {
            let center = (o as f32 + 0.5) / scale;
            let lo = ((center - support).floor().max(0.0)) as usize;
            let hi = ((center + support).ceil() as usize).min(len);
            let mut ws: Vec<f32> = (lo..hi)
                .map(|i| {
                    let d = ((i as f32 + 0.5) - center).abs() / support;
                    (1.0 - d).max(0.0)
                })
                .collect();
            let sum: f32 = ws.iter().sum();
            if sum > 0.0 {
                ws.iter_mut().for_each(|w| *w /= sum);
            } else {
                // Degenerate: nearest sample.
                let i = (center as usize).min(len - 1);
                return (i, vec![1.0]);
            }
            (lo, ws)
        })
        .collect();
    let (ow, oh) = if horizontal { (n, sh) } else { (sw, n) };
    let mut out = vec![[0f32; 4]; ow * oh];
    for y in 0..oh {
        for x in 0..ow {
            let (o, fixed) = if horizontal { (x, y) } else { (y, x) };
            let (lo, ws) = &weights[o];
            let mut acc = [0f32; 4];
            for (k, w) in ws.iter().enumerate() {
                let i = lo + k;
                let p = if horizontal { src[fixed * sw + i] } else { src[i * sw + fixed] };
                for c in 0..4 {
                    acc[c] += p[c] * w;
                }
            }
            out[y * ow + x] = acc;
        }
    }
    out
}

/// 8-bit coverage mask.
#[derive(Clone, PartialEq, Eq)]
pub struct Mask {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl fmt::Debug for Mask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Mask({}x{})", self.width, self.height)
    }
}

impl Mask {
    pub fn new(width: u32, height: u32) -> Self {
        Mask { width, height, data: vec![0; width as usize * height as usize] }
    }
    pub fn full(width: u32, height: u32) -> Self {
        Mask { width, height, data: vec![255; width as usize * height as usize] }
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x as u32 >= self.width || y as u32 >= self.height {
            0
        } else {
            self.data[y as usize * self.width as usize + x as usize]
        }
    }
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, v: u8) {
        if x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height {
            self.data[y as usize * self.width as usize + x as usize] = v;
        }
    }
    /// Keeps the strongest coverage (union of strokes without build-up).
    #[inline]
    pub fn max_at(&mut self, x: i32, y: i32, v: u8) {
        if x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height {
            let i = y as usize * self.width as usize + x as usize;
            if v > self.data[i] {
                self.data[i] = v;
            }
        }
    }
    pub fn clear(&mut self) {
        self.data.iter_mut().for_each(|v| *v = 0);
    }
    pub fn invert(&mut self) {
        self.data.iter_mut().for_each(|v| *v = 255 - *v);
    }
    /// Hard-edges the mask: values >= `t` become 255, others 0.
    pub fn threshold(&mut self, t: u8) {
        self.data.iter_mut().for_each(|v| *v = if *v >= t { 255 } else { 0 });
    }
    pub fn count_set(&self) -> usize {
        self.data.iter().filter(|v| **v > 0).count()
    }
    /// Bounding box of non-zero coverage.
    pub fn bbox(&self) -> Option<IRect> {
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for y in 0..self.height as i32 {
            let row = &self.data[y as usize * self.width as usize..(y as usize + 1) * self.width as usize];
            if let Some(first) = row.iter().position(|v| *v > 0) {
                let last = row.iter().rposition(|v| *v > 0).unwrap();
                x0 = x0.min(first as i32);
                x1 = x1.max(last as i32);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
        (x0 <= x1).then(|| IRect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1))
    }
    pub fn sub_mask(&self, r: IRect) -> Mask {
        let mut out = Mask::new(r.w.max(0) as u32, r.h.max(0) as u32);
        for y in 0..r.h {
            for x in 0..r.w {
                out.set(x, y, self.get(r.x + x, r.y + y));
            }
        }
        out
    }
    /// Nearest-neighbour resize (selection masks stay hard-edged).
    pub fn resized(&self, w: u32, h: u32) -> Mask {
        let mut out = Mask::new(w, h);
        if self.width == 0 || self.height == 0 {
            return out;
        }
        for y in 0..h {
            let sy = ((y as f32 + 0.5) * self.height as f32 / h as f32) as i32;
            for x in 0..w {
                let sx = ((x as f32 + 0.5) * self.width as f32 / w as f32) as i32;
                out.set(x as i32, y as i32, self.get(sx, sy));
            }
        }
        out
    }
    /// Applies the same geometric transform as the image helpers.
    pub fn map_image(&self, f: impl Fn(&Image) -> Image) -> Mask {
        let mut img = Image::new(self.width, self.height);
        for (i, v) in self.data.iter().enumerate() {
            img.data[i * 4 + 3] = *v;
        }
        let t = f(&img);
        Mask { width: t.width, height: t.height, data: t.data.as_chunks::<4>().0.iter().map(|p| p[3]).collect() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: u32, h: u32) -> Image {
        let mut img = Image::new(w, h);
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                img.set(x, y, Rgba::rgb(x as u8, y as u8, 7));
            }
        }
        img
    }

    #[test]
    fn get_set_and_bounds() {
        let mut img = Image::new(4, 3);
        img.set(1, 2, Rgba::rgb(9, 8, 7));
        assert_eq!(img.get(1, 2), Rgba::rgb(9, 8, 7));
        assert_eq!(img.get(-1, 0), Rgba::TRANSPARENT);
        img.set(10, 10, Rgba::WHITE); // ignored
        assert!(Image::from_rgba(2, 2, vec![0; 3]).is_err());
        assert_eq!(img.bounds(), IRect::new(0, 0, 4, 3));
    }

    #[test]
    fn transforms() {
        let img = gradient(3, 2);
        let cw = img.rotate_cw();
        assert_eq!((cw.width(), cw.height()), (2, 3));
        assert_eq!(cw.get(1, 0), img.get(0, 0));
        assert_eq!(cw.get(0, 2), img.get(2, 1));
        assert_eq!(cw.rotate_ccw(), img);
        assert_eq!(img.rotate_180(), img.rotate_cw().rotate_cw());
        assert_eq!(img.flip_horizontal().get(0, 0), img.get(2, 0));
        assert_eq!(img.flip_vertical().get(0, 0), img.get(0, 1));
        assert_eq!(img.flip_horizontal().flip_horizontal(), img);
    }

    #[test]
    fn sub_image_and_draw() {
        let img = gradient(5, 5);
        let s = img.sub_image(IRect::new(3, 3, 4, 4));
        assert_eq!(s.get(0, 0), img.get(3, 3));
        assert_eq!(s.get(3, 3), Rgba::TRANSPARENT);
        let mut dst = Image::filled(5, 5, Rgba::WHITE);
        let mut mask = Mask::new(4, 4);
        mask.set(0, 0, 255);
        dst.draw(&s, 1, 1, &DrawOptions { mask: Some(&mask), ..Default::default() });
        assert_eq!(dst.get(1, 1), img.get(3, 3));
        assert_eq!(dst.get(2, 2), Rgba::WHITE);
        // Colour key skips matching pixels.
        let mut dst2 = Image::filled(2, 1, Rgba::BLACK);
        let mut src = Image::filled(2, 1, Rgba::WHITE);
        src.set(1, 0, Rgba::rgb(255, 0, 0));
        dst2.draw(&src, 0, 0, &DrawOptions { key: Some(Rgba::WHITE), ..Default::default() });
        assert_eq!(dst2.get(0, 0), Rgba::BLACK);
        assert_eq!(dst2.get(1, 0), Rgba::rgb(255, 0, 0));
    }

    #[test]
    fn resize_keeps_flat_colours() {
        let img = Image::filled(10, 6, Rgba::rgb(10, 200, 30));
        for (w, h) in [(20, 12), (3, 2), (1, 1), (10, 6)] {
            let r = img.resized(w, h);
            assert_eq!((r.width(), r.height()), (w, h));
            assert!(r.is_uniform(Rgba::rgb(10, 200, 30)), "{w}x{h}");
        }
        // Transparent pixels do not darken neighbours.
        let mut half = Image::new(2, 1);
        half.set(0, 0, Rgba::rgb(255, 0, 0));
        let r = half.resized(1, 1);
        assert_eq!((r.get(0, 0).r, r.get(0, 0).g), (255, 0));
        assert!((120..=135).contains(&r.get(0, 0).a));
    }

    #[test]
    fn skew_grows_canvas() {
        let img = Image::filled(10, 10, Rgba::BLACK);
        let s = img.skewed(45.0, 0.0, Rgba::WHITE);
        assert_eq!((s.width(), s.height()), (20, 10));
        assert_eq!(s.get(0, 0), Rgba::BLACK);
        assert_eq!(s.get(19, 0), Rgba::WHITE);
        assert_eq!(s.get(0, 9), Rgba::WHITE);
        assert_eq!(s.get(19, 9), Rgba::BLACK);
        let v = img.skewed(0.0, -30.0, Rgba::WHITE);
        assert_eq!((v.width(), v.height()), (10, 16));
        assert_eq!(img.skewed(0.0, 0.0, Rgba::WHITE), img);
    }

    #[test]
    fn canvas_resize_and_thumbnail() {
        let img = gradient(4, 4);
        let big = img.with_canvas_size(6, 5, Rgba::WHITE);
        assert_eq!(big.get(3, 3), img.get(3, 3));
        assert_eq!(big.get(5, 4), Rgba::WHITE);
        let small = img.with_canvas_size(2, 2, Rgba::WHITE);
        assert_eq!(small.get(1, 1), img.get(1, 1));
        let t = Image::new(400, 100).thumbnail(40);
        assert_eq!((t.width(), t.height()), (40, 10));
    }

    #[test]
    fn mask_ops() {
        let mut m = Mask::new(5, 5);
        assert_eq!(m.bbox(), None);
        m.set(1, 2, 200);
        m.max_at(3, 3, 10);
        m.max_at(3, 3, 5);
        assert_eq!(m.get(3, 3), 10);
        assert_eq!(m.bbox(), Some(IRect::new(1, 2, 3, 2)));
        m.threshold(128);
        assert_eq!(m.count_set(), 1);
        let r = m.resized(10, 10);
        assert_eq!(r.get(2, 4), 255);
        assert_eq!(r.get(3, 5), 255);
        let rot = m.map_image(|i| i.rotate_cw());
        assert_eq!(rot.get(2, 1), 255);
        m.invert();
        assert_eq!(m.count_set(), 24);
    }
}
