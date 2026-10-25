//! Brush engines. A stroke accumulates coverage in a [`Mask`]; the editor then
//! composites `base + colour * coverage` so overlapping dabs never build up.

use crate::geom::{IRect, Pt};
use crate::image::Mask;
use crate::raster;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BrushKind {
    Pencil,
    Brush,
    CalligraphyLeft,
    CalligraphyRight,
    Airbrush,
    Oil,
    Crayon,
    Marker,
    NaturalPencil,
    Watercolor,
    /// Hard square, used by the eraser.
    Eraser,
}

impl BrushKind {
    pub const ALL_BRUSHES: [BrushKind; 9] = [
        BrushKind::Brush,
        BrushKind::CalligraphyLeft,
        BrushKind::CalligraphyRight,
        BrushKind::Airbrush,
        BrushKind::Oil,
        BrushKind::Crayon,
        BrushKind::Marker,
        BrushKind::NaturalPencil,
        BrushKind::Watercolor,
    ];

    pub fn name(self) -> &'static str {
        match self {
            BrushKind::Pencil => "pencil",
            BrushKind::Brush => "brush",
            BrushKind::CalligraphyLeft => "calligraphy-1",
            BrushKind::CalligraphyRight => "calligraphy-2",
            BrushKind::Airbrush => "airbrush",
            BrushKind::Oil => "oil",
            BrushKind::Crayon => "crayon",
            BrushKind::Marker => "marker",
            BrushKind::NaturalPencil => "natural-pencil",
            BrushKind::Watercolor => "watercolor",
            BrushKind::Eraser => "eraser",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL_BRUSHES.iter().copied().chain([BrushKind::Pencil, BrushKind::Eraser]).find(|b| b.name() == s)
    }

    /// Colour opacity multiplier applied when compositing the stroke.
    pub fn opacity(self) -> f32 {
        match self {
            BrushKind::Marker => 0.5,
            BrushKind::NaturalPencil => 0.85,
            _ => 1.0,
        }
    }
}

/// Small deterministic PRNG (xorshift32) so strokes are reproducible in tests.
#[derive(Clone, Debug)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng(seed.max(1))
    }
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// Uniform in [0, 1).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Position-based hash in [0, 1) for paper-grain textures.
fn grain(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65536.0
}

#[derive(Clone, Debug)]
pub struct Stroke {
    pub kind: BrushKind,
    pub size: u32,
    last: Option<Pt>,
    rng: Rng,
    bristles: Vec<(Pt, f32)>,
    /// Distance travelled since the last spaced dab.
    carry: f32,
}

impl Stroke {
    pub fn new(kind: BrushKind, size: u32, seed: u32) -> Self {
        let mut rng = Rng::new(seed);
        let size = size.max(1);
        let bristles = if kind == BrushKind::Oil {
            let r = size as f32 / 2.0;
            let n = (size as usize).clamp(3, 24);
            (0..n)
                .map(|_| {
                    let a = rng.next_f32() * std::f32::consts::TAU;
                    let d = rng.next_f32().sqrt() * r * 0.8;
                    (Pt::new(a.cos() * d, a.sin() * d), 0.55 + rng.next_f32() * 0.45)
                })
                .collect()
        } else {
            Vec::new()
        };
        Stroke { kind, size, last: None, rng, bristles, carry: 0.0 }
    }

    pub fn airbrush_radius(&self) -> f32 {
        self.size as f32 * 1.5 + 3.0
    }

    /// Extends the stroke to `p`, returning the dirty rectangle.
    pub fn to(&mut self, mask: &mut Mask, p: Pt) -> IRect {
        let first = self.last.is_none();
        let from = self.last.unwrap_or(p);
        self.last = Some(p);
        let size = self.size as f32;
        match self.kind {
            BrushKind::Pencil => {
                let (x0, y0, x1, y1) = (from.x.floor() as i32, from.y.floor() as i32, p.x.floor() as i32, p.y.floor() as i32);
                let mut dirty = IRect::default();
                for (x, y) in raster::line_pixels(x0, y0, x1, y1) {
                    dirty = dirty.union(&raster::stamp_hard_disc(mask, x, y, self.size));
                }
                dirty
            }
            BrushKind::Brush => raster::stamp_capsule(mask, from, p, size / 2.0, 1.0),
            BrushKind::CalligraphyLeft | BrushKind::CalligraphyRight => {
                let ang: f32 = if self.kind == BrushKind::CalligraphyLeft { -45f32 } else { 45f32 };
                let (s, c) = ang.to_radians().sin_cos();
                let half = Pt::new(c, s).scale(size.max(2.0) / 2.0);
                let quad = vec![from - half, from + half, p + half, p - half];
                let mut d = raster::fill_polygons(mask, &[quad], raster::FillRule::NonZero, 1.0);
                d = d.union(&raster::stamp_capsule(mask, p - half, p + half, 0.5, 1.0));
                d.union(&raster::stamp_capsule(mask, from - half, from + half, 0.5, 1.0))
            }
            BrushKind::Oil => {
                let mut d = IRect::default();
                for (off, strength) in self.bristles.clone() {
                    let r = (size / 6.0).max(0.6);
                    d = d.union(&raster::stamp_capsule(mask, from + off, p + off, r, strength));
                }
                d
            }
            BrushKind::Marker | BrushKind::Eraser => {
                self.spaced(from, p, 1.0, |m, q| raster::stamp_square(m, q, size, 1.0), mask)
            }
            BrushKind::Crayon => {
                let r = (size / 2.0).max(1.0);
                self.spaced(from, p, 1.0, |m, q| crayon_dab(m, q, r), mask)
            }
            BrushKind::NaturalPencil => {
                let r = (size / 3.0).max(0.7);
                self.spaced(from, p, 0.7, |m, q| crayon_dab(m, q, r), mask)
            }
            BrushKind::Watercolor => {
                let r = (size / 2.0).max(2.0);
                self.spaced(from, p, (r / 4.0).max(1.0), |m, q| raster::stamp_soft_disc(m, q, r, 0.45), mask)
            }
            BrushKind::Airbrush => {
                let step = (self.airbrush_radius() / 2.0).max(1.0);
                let mut d = if first { self.spray(mask, p) } else { IRect::default() };
                let dist = from.dist(p);
                // `carry` is the distance travelled since the previous burst.
                let mut t = step - self.carry;
                while t <= dist {
                    d = d.union(&self.spray(mask, from.lerp(p, t / dist)));
                    t += step;
                }
                self.carry = dist - (t - step);
                d
            }
        }
    }

    /// One airbrush burst at the last position (called on a timer while the button is held).
    pub fn tick(&mut self, mask: &mut Mask) -> IRect {
        match (self.kind, self.last) {
            (BrushKind::Airbrush, Some(p)) => self.spray(mask, p),
            _ => IRect::default(),
        }
    }

    fn spray(&mut self, mask: &mut Mask, c: Pt) -> IRect {
        let r = self.airbrush_radius();
        let dots = (r * 1.2) as usize + 2;
        for _ in 0..dots {
            let a = self.rng.next_f32() * std::f32::consts::TAU;
            let d = self.rng.next_f32().sqrt() * r;
            let q = c + Pt::new(a.cos() * d, a.sin() * d);
            mask.max_at(q.x.floor() as i32, q.y.floor() as i32, 255);
        }
        IRect::from_bounds(c.x - r - 1.0, c.y - r - 1.0, c.x + r + 1.0, c.y + r + 1.0).clamp_to(mask.width(), mask.height())
    }

    /// Places dabs every `spacing` pixels from `from` to `p`.
    fn spaced(&mut self, from: Pt, p: Pt, spacing: f32, mut dab: impl FnMut(&mut Mask, Pt) -> IRect, mask: &mut Mask) -> IRect {
        let dist = from.dist(p);
        let steps = (dist / spacing).ceil().max(1.0) as usize;
        let mut d = IRect::default();
        for i in 0..=steps {
            d = d.union(&dab(mask, from.lerp(p, i as f32 / steps as f32)));
        }
        d
    }
}

fn crayon_dab(mask: &mut Mask, c: Pt, r: f32) -> IRect {
    let bounds = IRect::from_bounds(c.x - r - 1.0, c.y - r - 1.0, c.x + r + 1.0, c.y + r + 1.0).clamp_to(mask.width(), mask.height());
    for y in bounds.y..bounds.bottom() {
        for x in bounds.x..bounds.right() {
            let d = Pt::new(x as f32 + 0.5, y as f32 + 0.5).dist(c);
            if d <= r {
                let g = grain(x, y);
                let cov = if g < 0.62 { 0.95 } else if g < 0.85 { 0.45 } else { 0.0 };
                if cov > 0.0 {
                    mask.max_at(x, y, (cov * 255.0) as u8);
                }
            }
        }
    }
    bounds
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(kind: BrushKind, size: u32, pts: &[Pt]) -> Mask {
        let mut m = Mask::new(60, 60);
        let mut s = Stroke::new(kind, size, 42);
        for p in pts {
            s.to(&mut m, *p);
        }
        m
    }

    #[test]
    fn names_roundtrip() {
        for b in BrushKind::ALL_BRUSHES {
            assert_eq!(BrushKind::from_name(b.name()), Some(b));
        }
        assert_eq!(BrushKind::from_name("nope"), None);
    }

    #[test]
    fn every_brush_marks_its_path() {
        let path = [Pt::new(10.0, 30.0), Pt::new(50.0, 30.0)];
        for kind in BrushKind::ALL_BRUSHES.into_iter().chain([BrushKind::Pencil, BrushKind::Eraser]) {
            let m = run(kind, 8, &path);
            if kind == BrushKind::Airbrush {
                // Sprayed dots are sparse: check the spread along the path instead.
                let bbox = m.bbox().unwrap();
                assert!(bbox.x <= 15 && bbox.right() >= 45 && m.count_set() > 60, "{kind:?} {bbox:?}");
                continue;
            }
            let covered = (12..48).filter(|x| (26..35).any(|y| m.get(*x, y) > 0)).count();
            assert!(covered >= 30, "{kind:?} covered only {covered} columns");
            assert_eq!(m.get(30, 5), 0, "{kind:?} leaked far from the path");
        }
    }

    #[test]
    fn pencil_is_one_pixel_wide() {
        let m = run(BrushKind::Pencil, 1, &[Pt::new(5.5, 5.5), Pt::new(15.5, 5.5)]);
        assert_eq!(m.count_set(), 11);
        assert_eq!(m.bbox(), Some(IRect::new(5, 5, 11, 1)));
    }

    #[test]
    fn calligraphy_nibs_are_directional() {
        // A stroke parallel to the nib is thin, a perpendicular one is thick.
        let left = run(BrushKind::CalligraphyLeft, 12, &[Pt::new(20.0, 40.0), Pt::new(40.0, 20.0)]);
        let right = run(BrushKind::CalligraphyRight, 12, &[Pt::new(20.0, 40.0), Pt::new(40.0, 20.0)]);
        assert!(right.count_set() > left.count_set() * 2, "{} vs {}", right.count_set(), left.count_set());
    }

    #[test]
    fn airbrush_is_sparse_and_deterministic() {
        let a = run(BrushKind::Airbrush, 10, &[Pt::new(30.0, 30.0)]);
        let b = run(BrushKind::Airbrush, 10, &[Pt::new(30.0, 30.0)]);
        assert_eq!(a, b);
        let r = 18.0f32;
        let area = (std::f32::consts::PI * r * r) as usize;
        assert!(a.count_set() > 5 && a.count_set() < area / 2);
        let mut m = Mask::new(60, 60);
        let mut s = Stroke::new(BrushKind::Airbrush, 10, 1);
        s.to(&mut m, Pt::new(30.0, 30.0));
        let before = m.count_set();
        for _ in 0..20 {
            s.tick(&mut m);
        }
        assert!(m.count_set() > before);
    }

    #[test]
    fn watercolor_is_translucent() {
        let m = run(BrushKind::Watercolor, 16, &[Pt::new(30.0, 30.0)]);
        assert!(m.get(30, 30) > 50 && m.get(30, 30) < 200);
        assert!(BrushKind::Marker.opacity() < 1.0);
    }
}
