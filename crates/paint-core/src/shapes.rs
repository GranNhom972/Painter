//! Shape geometry (Paint's shape gallery) and rendering to coverage masks.

use crate::geom::{IRect, Pt};
use crate::image::Mask;
use crate::raster::{self, FillRule};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShapeKind {
    Line,
    Curve,
    Oval,
    Rectangle,
    RoundedRectangle,
    Polygon,
    Triangle,
    RightTriangle,
    Diamond,
    Pentagon,
    Hexagon,
    ArrowRight,
    ArrowLeft,
    ArrowUp,
    ArrowDown,
    Star4,
    Star5,
    Star6,
    CalloutRect,
    CalloutOval,
    CalloutCloud,
    Heart,
    Lightning,
}

impl ShapeKind {
    pub const ALL: [ShapeKind; 23] = [
        ShapeKind::Line,
        ShapeKind::Curve,
        ShapeKind::Oval,
        ShapeKind::Rectangle,
        ShapeKind::RoundedRectangle,
        ShapeKind::Polygon,
        ShapeKind::Triangle,
        ShapeKind::RightTriangle,
        ShapeKind::Diamond,
        ShapeKind::Pentagon,
        ShapeKind::Hexagon,
        ShapeKind::ArrowRight,
        ShapeKind::ArrowLeft,
        ShapeKind::ArrowUp,
        ShapeKind::ArrowDown,
        ShapeKind::Star4,
        ShapeKind::Star5,
        ShapeKind::Star6,
        ShapeKind::CalloutRect,
        ShapeKind::CalloutOval,
        ShapeKind::CalloutCloud,
        ShapeKind::Heart,
        ShapeKind::Lightning,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ShapeKind::Line => "line",
            ShapeKind::Curve => "curve",
            ShapeKind::Oval => "oval",
            ShapeKind::Rectangle => "rectangle",
            ShapeKind::RoundedRectangle => "rounded-rectangle",
            ShapeKind::Polygon => "polygon",
            ShapeKind::Triangle => "triangle",
            ShapeKind::RightTriangle => "right-triangle",
            ShapeKind::Diamond => "diamond",
            ShapeKind::Pentagon => "pentagon",
            ShapeKind::Hexagon => "hexagon",
            ShapeKind::ArrowRight => "arrow-right",
            ShapeKind::ArrowLeft => "arrow-left",
            ShapeKind::ArrowUp => "arrow-up",
            ShapeKind::ArrowDown => "arrow-down",
            ShapeKind::Star4 => "star-4",
            ShapeKind::Star5 => "star-5",
            ShapeKind::Star6 => "star-6",
            ShapeKind::CalloutRect => "callout-rect",
            ShapeKind::CalloutOval => "callout-oval",
            ShapeKind::CalloutCloud => "callout-cloud",
            ShapeKind::Heart => "heart",
            ShapeKind::Lightning => "lightning",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.name() == s)
    }

    /// Shapes defined by a bounding box (everything except line, curve and polygon).
    pub fn is_box(self) -> bool {
        !matches!(self, ShapeKind::Line | ShapeKind::Curve | ShapeKind::Polygon)
    }

    pub fn is_open(self) -> bool {
        matches!(self, ShapeKind::Line | ShapeKind::Curve)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutlineStyle {
    None,
    Solid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillStyle {
    None,
    Solid,
}

/// Geometry of a shape being drawn or edited.
#[derive(Clone, Debug, PartialEq)]
pub enum ShapeGeom {
    /// Corners of the bounding box (any order).
    Box(Pt, Pt),
    Line(Pt, Pt),
    /// Start, control 1, control 2, end.
    Curve([Pt; 4]),
    Polygon { points: Vec<Pt>, closed: bool },
}

impl ShapeGeom {
    /// Points used for bounds and handles.
    pub fn points(&self) -> Vec<Pt> {
        match self {
            ShapeGeom::Box(a, b) | ShapeGeom::Line(a, b) => vec![*a, *b],
            ShapeGeom::Curve(p) => p.to_vec(),
            ShapeGeom::Polygon { points, .. } => points.clone(),
        }
    }

    pub fn bounds(&self) -> (Pt, Pt) {
        let pts = self.points();
        let mut lo = Pt::new(f32::MAX, f32::MAX);
        let mut hi = Pt::new(f32::MIN, f32::MIN);
        for p in pts {
            lo = Pt::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Pt::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        (lo, hi)
    }

    pub fn translate(&mut self, d: Pt) {
        match self {
            ShapeGeom::Box(a, b) | ShapeGeom::Line(a, b) => {
                *a = *a + d;
                *b = *b + d;
            }
            ShapeGeom::Curve(p) => p.iter_mut().for_each(|q| *q = *q + d),
            ShapeGeom::Polygon { points, .. } => points.iter_mut().for_each(|q| *q = *q + d),
        }
    }

    /// Maps every point from the box `from` to the box `to` (both as (min, max)).
    pub fn remap(&mut self, from: (Pt, Pt), to: (Pt, Pt)) {
        // A box shape is its bounds: take the new ones as they are, even from a
        // collapsed (zero width or height) box.
        if let ShapeGeom::Box(a, b) = self {
            *a = to.0;
            *b = to.1;
            return;
        }
        let map = |p: Pt| {
            let fx = if from.1.x != from.0.x { (p.x - from.0.x) / (from.1.x - from.0.x) } else { 0.0 };
            let fy = if from.1.y != from.0.y { (p.y - from.0.y) / (from.1.y - from.0.y) } else { 0.0 };
            Pt::new(to.0.x + fx * (to.1.x - to.0.x), to.0.y + fy * (to.1.y - to.0.y))
        };
        match self {
            ShapeGeom::Box(a, b) | ShapeGeom::Line(a, b) => {
                *a = map(*a);
                *b = map(*b);
            }
            ShapeGeom::Curve(p) => p.iter_mut().for_each(|q| *q = map(*q)),
            ShapeGeom::Polygon { points, .. } => points.iter_mut().for_each(|q| *q = map(*q)),
        }
    }
}

/// Snaps a point to the centre of the pixel it falls in (crisp odd-width strokes).
pub fn pixel_center(p: Pt) -> Pt {
    Pt::new(p.x.floor() + 0.5, p.y.floor() + 0.5)
}

fn unit(points: &[(f32, f32)], l: f32, t: f32, r: f32, b: f32) -> Vec<Pt> {
    points.iter().map(|(u, v)| Pt::new(l + u * (r - l), t + v * (b - t))).collect()
}

/// Regular polygon / star vertices normalised to the unit square.
fn radial(n: usize, start_deg: f32, inner: Option<f32>) -> Vec<(f32, f32)> {
    let count = if inner.is_some() { n * 2 } else { n };
    let raw: Vec<(f32, f32)> = (0..count)
        .map(|i| {
            let a = start_deg.to_radians() + i as f32 * TAU / count as f32;
            let r = match inner {
                Some(k) if i % 2 == 1 => k,
                _ => 1.0,
            };
            (r * a.cos(), r * a.sin())
        })
        .collect();
    normalise(raw)
}

fn normalise(pts: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (x, y) in &pts {
        x0 = x0.min(*x);
        y0 = y0.min(*y);
        x1 = x1.max(*x);
        y1 = y1.max(*y);
    }
    pts.into_iter().map(|(x, y)| ((x - x0) / (x1 - x0), (y - y0) / (y1 - y0))).collect()
}

fn rounded_rect(l: f32, t: f32, r: f32, b: f32, rad: f32) -> Vec<Pt> {
    let rad = rad.min((r - l) / 2.0).min((b - t) / 2.0).max(0.0);
    let mut pts = Vec::new();
    let corners = [(r - rad, t + rad, -90.0f32), (r - rad, b - rad, 0.0), (l + rad, b - rad, 90.0), (l + rad, t + rad, 180.0)];
    for (cx, cy, start) in corners {
        for i in 0..=8 {
            let a = (start + i as f32 * 90.0 / 8.0).to_radians();
            pts.push(Pt::new(cx + rad * a.cos(), cy + rad * a.sin()));
        }
    }
    pts
}

/// Closed sub-paths of a box shape inside the bounds `l, t, r, b`.
pub fn box_paths(kind: ShapeKind, l: f32, t: f32, r: f32, b: f32) -> Vec<Vec<Pt>> {
    let (w, h) = (r - l, b - t);
    match kind {
        ShapeKind::Oval => vec![raster::ellipse_points((l + r) / 2.0, (t + b) / 2.0, w / 2.0, h / 2.0)],
        ShapeKind::Rectangle => vec![vec![Pt::new(l, t), Pt::new(r, t), Pt::new(r, b), Pt::new(l, b)]],
        ShapeKind::RoundedRectangle => vec![rounded_rect(l, t, r, b, w.min(h) * 0.2)],
        ShapeKind::Triangle => vec![unit(&[(0.5, 0.0), (1.0, 1.0), (0.0, 1.0)], l, t, r, b)],
        ShapeKind::RightTriangle => vec![unit(&[(0.0, 0.0), (1.0, 1.0), (0.0, 1.0)], l, t, r, b)],
        ShapeKind::Diamond => vec![unit(&[(0.5, 0.0), (1.0, 0.5), (0.5, 1.0), (0.0, 0.5)], l, t, r, b)],
        ShapeKind::Pentagon => vec![unit(&radial(5, -90.0, None), l, t, r, b)],
        ShapeKind::Hexagon => vec![unit(&radial(6, 0.0, None), l, t, r, b)],
        ShapeKind::ArrowRight | ShapeKind::ArrowLeft | ShapeKind::ArrowUp | ShapeKind::ArrowDown => {
            let right = [(0.0, 0.3), (0.6, 0.3), (0.6, 0.0), (1.0, 0.5), (0.6, 1.0), (0.6, 0.7), (0.0, 0.7)];
            let pts: Vec<(f32, f32)> = right
                .iter()
                .map(|&(x, y)| match kind {
                    ShapeKind::ArrowLeft => (1.0 - x, y),
                    ShapeKind::ArrowUp => (y, 1.0 - x),
                    ShapeKind::ArrowDown => (y, x),
                    _ => (x, y),
                })
                .collect();
            vec![unit(&pts, l, t, r, b)]
        }
        ShapeKind::Star4 => vec![unit(&radial(4, -90.0, Some(0.4)), l, t, r, b)],
        ShapeKind::Star5 => vec![unit(&radial(5, -90.0, Some(0.4)), l, t, r, b)],
        ShapeKind::Star6 => vec![unit(&radial(6, -90.0, Some(0.55)), l, t, r, b)],
        ShapeKind::CalloutRect => {
            let body_b = t + h * 0.78;
            let rad = w.min(body_b - t) * 0.15;
            let mut pts = rounded_rect(l, t, r, body_b, rad);
            // Insert the tail between the bottom-right and bottom-left corners (index 17/18).
            let tail = [Pt::new(l + w * 0.38, body_b), Pt::new(l + w * 0.15, b), Pt::new(l + w * 0.22, body_b)];
            pts.splice(18..18, tail);
            vec![pts]
        }
        ShapeKind::CalloutOval => {
            let (cx, cy, rx, ry) = (l + w * 0.5, t + h * 0.4, w * 0.5, h * 0.4);
            let (a0, a1) = (115f32.to_radians(), 135f32.to_radians());
            let n = 96;
            let mut pts: Vec<Pt> = (0..=n)
                .map(|i| {
                    let a = a1 + (TAU - (a1 - a0)) * i as f32 / n as f32;
                    Pt::new(cx + rx * a.cos(), cy + ry * a.sin())
                })
                .collect();
            pts.push(Pt::new(l + w * 0.18, b));
            vec![pts]
        }
        ShapeKind::CalloutCloud => {
            // Scalloped outline normalised to the upper 80 % of the box, plus two puffs.
            let n = 240;
            let raw: Vec<(f32, f32)> = (0..n)
                .map(|i| {
                    let a = i as f32 / n as f32 * TAU;
                    let k = 0.82 + 0.18 * (5.0 * a).sin().abs();
                    (k * a.cos(), k * a.sin())
                })
                .collect();
            let body: Vec<(f32, f32)> = normalise(raw).into_iter().map(|(u, v)| (u, v * 0.8)).collect();
            vec![
                unit(&body, l, t, r, b),
                raster::ellipse_points(l + w * 0.24, t + h * 0.86, w * 0.07, h * 0.05),
                raster::ellipse_points(l + w * 0.13, t + h * 0.96, w * 0.04, h * 0.03),
            ]
        }
        ShapeKind::Heart => {
            let n = 160;
            let raw: Vec<(f32, f32)> = (0..n)
                .map(|i| {
                    let s = i as f32 / n as f32 * TAU;
                    let x = 16.0 * s.sin().powi(3);
                    let y = -(13.0 * s.cos() - 5.0 * (2.0 * s).cos() - 2.0 * (3.0 * s).cos() - (4.0 * s).cos());
                    (x, y)
                })
                .collect();
            vec![unit(&normalise(raw), l, t, r, b)]
        }
        ShapeKind::Lightning => vec![unit(
            &[(0.0, 0.24), (0.38, 0.0), (0.56, 0.31), (0.48, 0.35), (0.76, 0.6), (0.69, 0.64), (1.0, 1.0), (0.5, 0.72), (0.58, 0.68), (0.23, 0.5), (0.32, 0.45)],
            l,
            t,
            r,
            b,
        )],
        ShapeKind::Line | ShapeKind::Curve | ShapeKind::Polygon => Vec::new(),
    }
}

/// Coverage of a shape over `rect` (canvas coordinates): `fill` and `line` are
/// `rect`-sized masks, so rendering costs only the shape's area, not the canvas.
#[derive(Clone, Debug)]
pub struct Coverage {
    pub rect: IRect,
    pub fill: Mask,
    pub line: Mask,
}

impl Coverage {
    pub fn fill_at(&self, x: i32, y: i32) -> u8 {
        self.fill.get(x - self.rect.x, y - self.rect.y)
    }
    pub fn line_at(&self, x: i32, y: i32) -> u8 {
        self.line.get(x - self.rect.x, y - self.rect.y)
    }
}

/// Renders a shape on a `w` x `h` canvas.
pub fn render(kind: ShapeKind, geom: &ShapeGeom, outline: OutlineStyle, fill: FillStyle, width: u32, w: u32, h: u32) -> Coverage {
    let lw = width.max(1) as f32;
    let (lo, hi) = geom.bounds();
    let m = lw / 2.0 + 2.0;
    let rect = IRect::from_bounds(lo.x - m, lo.y - m, hi.x + m + 1.0, hi.y + m + 1.0).clamp_to(w, h);
    // Work in the frame of `rect`: integer offsets keep the pixel-centre snapping intact.
    let mut local = geom.clone();
    local.translate(Pt::new(-rect.x as f32, -rect.y as f32));
    let (fill_m, line_m) = render_local(kind, &local, outline, fill, lw, rect.w.max(0) as u32, rect.h.max(0) as u32);
    Coverage { rect, fill: fill_m, line: line_m }
}

fn render_local(kind: ShapeKind, geom: &ShapeGeom, outline: OutlineStyle, fill: FillStyle, lw: f32, w: u32, h: u32) -> (Mask, Mask) {
    let mut fill_m = Mask::new(w, h);
    let mut line_m = Mask::new(w, h);
    let has_outline = outline == OutlineStyle::Solid || kind.is_open();
    match geom {
        ShapeGeom::Line(a, b) => {
            raster::stroke_polyline(&mut line_m, &[pixel_center(*a), pixel_center(*b)], lw, false, 1.0);
        }
        ShapeGeom::Curve(p) => {
            let pts = raster::cubic_bezier(pixel_center(p[0]), p[1], p[2], pixel_center(p[3]));
            raster::stroke_polyline(&mut line_m, &pts, lw, false, 1.0);
        }
        ShapeGeom::Polygon { points, closed } => {
            let pts: Vec<Pt> = points.iter().map(|p| pixel_center(*p)).collect();
            if *closed && fill == FillStyle::Solid && pts.len() >= 3 {
                raster::fill_polygons(&mut fill_m, std::slice::from_ref(&pts), FillRule::NonZero, 1.0);
            }
            if has_outline || !*closed {
                raster::stroke_polyline(&mut line_m, &pts, lw, *closed, 1.0);
            }
        }
        ShapeGeom::Box(a, b) => {
            let (lo, hi) = (Pt::new(a.x.min(b.x), a.y.min(b.y)), Pt::new(a.x.max(b.x), a.y.max(b.y)));
            // With an outline the path runs through pixel centres; fill-only shapes cover whole pixels.
            let (l, t, r, bt) = if has_outline {
                (lo.x.floor() + 0.5, lo.y.floor() + 0.5, hi.x.floor() + 0.5, hi.y.floor() + 0.5)
            } else {
                (lo.x.floor(), lo.y.floor(), hi.x.floor() + 1.0, hi.y.floor() + 1.0)
            };
            let paths = box_paths(kind, l, t, r, bt);
            if fill == FillStyle::Solid {
                raster::fill_polygons(&mut fill_m, &paths, FillRule::NonZero, 1.0);
            }
            if has_outline {
                if kind == ShapeKind::Rectangle {
                    let hw = lw / 2.0;
                    let outer = vec![Pt::new(l - hw, t - hw), Pt::new(r + hw, t - hw), Pt::new(r + hw, bt + hw), Pt::new(l - hw, bt + hw)];
                    let mut rings = vec![outer];
                    if r - l > lw && bt - t > lw {
                        rings.push(vec![Pt::new(l + hw, t + hw), Pt::new(l + hw, bt - hw), Pt::new(r - hw, bt - hw), Pt::new(r - hw, t + hw)]);
                    }
                    raster::fill_polygons(&mut line_m, &rings, FillRule::EvenOdd, 1.0);
                } else {
                    for p in &paths {
                        raster::stroke_polyline(&mut line_m, p, lw, true, 1.0);
                    }
                }
            }
        }
    }
    (fill_m, line_m)
}

/// Constrains a drag for Shift: squares for box shapes, 45° steps for lines.
pub fn constrain(kind: ShapeKind, a: Pt, b: Pt) -> Pt {
    let d = b - a;
    if kind.is_box() {
        let s = d.x.abs().max(d.y.abs());
        Pt::new(a.x + s * d.x.signum(), a.y + s * d.y.signum())
    } else {
        let ang = d.y.atan2(d.x);
        let snapped = (ang / (PI / 4.0)).round() * (PI / 4.0);
        let len = d.len();
        Pt::new(a.x + len * snapped.cos(), a.y + len * snapped.sin())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_roundtrip() {
        for k in ShapeKind::ALL {
            assert_eq!(ShapeKind::from_name(k.name()), Some(k));
        }
        assert!(ShapeKind::Rectangle.is_box() && !ShapeKind::Line.is_box() && !ShapeKind::Polygon.is_box());
    }

    #[test]
    fn every_box_shape_stays_in_its_box() {
        for k in ShapeKind::ALL.into_iter().filter(|k| k.is_box()) {
            let paths = box_paths(k, 10.0, 20.0, 110.0, 70.0);
            assert!(!paths.is_empty(), "{k:?}");
            for p in paths.iter().flatten() {
                assert!(p.x >= 9.99 && p.x <= 110.01 && p.y >= 19.99 && p.y <= 70.01, "{k:?} {p:?}");
            }
            // ...and fills it: the outline reaches every side of the box.
            let (x0, x1) = paths.iter().flatten().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
            let (y0, y1) = paths.iter().flatten().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
            assert!(x0 < 11.0 && x1 > 109.0 && y0 < 21.0 && y1 > 69.0, "{k:?} spans only {x0}..{x1} x {y0}..{y1}");
            let c = render(k, &ShapeGeom::Box(Pt::new(10.0, 20.0), Pt::new(110.0, 70.0)), OutlineStyle::Solid, FillStyle::Solid, 3, 200, 100);
            assert!(c.fill.count_set() > 300, "{k:?} fill {}", c.fill.count_set());
            assert!(c.line.count_set() > 100, "{k:?} outline {}", c.line.count_set());
            // The coverage frame is the shape's area (plus a margin), not the canvas.
            assert!(c.rect.x >= 5 && c.rect.right() <= 116 && c.rect.y >= 15 && c.rect.bottom() <= 76, "{k:?} {:?}", c.rect);
        }
    }

    #[test]
    fn crisp_rectangle_outline() {
        let c = render(ShapeKind::Rectangle, &ShapeGeom::Box(Pt::new(2.2, 2.7), Pt::new(7.9, 6.1)), OutlineStyle::Solid, FillStyle::None, 1, 12, 12);
        assert_eq!(c.fill.count_set(), 0);
        // Border of the 6x5 pixel box (2..=7, 2..=6) only, fully opaque.
        assert_eq!(c.line.count_set(), 2 * 6 + 2 * 3);
        assert_eq!(c.line_at(2, 2), 255);
        assert_eq!(c.line_at(7, 6), 255);
        assert_eq!(c.line_at(4, 4), 0);
        // Fill only covers whole pixels.
        let c = render(ShapeKind::Rectangle, &ShapeGeom::Box(Pt::new(2.0, 2.0), Pt::new(4.0, 3.0)), OutlineStyle::None, FillStyle::Solid, 1, 12, 12);
        assert_eq!(c.line.count_set(), 0);
        assert_eq!(c.fill.count_set(), 6);
        assert_eq!(c.fill_at(4, 3), 255);
    }

    #[test]
    fn line_and_curve() {
        let c = render(ShapeKind::Line, &ShapeGeom::Line(Pt::new(1.0, 5.0), Pt::new(9.0, 5.0)), OutlineStyle::None, FillStyle::Solid, 1, 12, 12);
        assert_eq!(c.line.count_set(), 9);
        assert_eq!(c.line_at(1, 5), 255);
        let g = ShapeGeom::Curve([Pt::new(1.0, 10.0), Pt::new(1.0, 0.0), Pt::new(20.0, 0.0), Pt::new(20.0, 10.0)]);
        let c = render(ShapeKind::Curve, &g, OutlineStyle::Solid, FillStyle::None, 2, 30, 30);
        assert!(c.line_at(10, 3) > 0 && c.line_at(10, 10) == 0);
        // Shapes partly outside the canvas are clipped, not shifted.
        let c = render(ShapeKind::Line, &ShapeGeom::Line(Pt::new(-20.0, 5.0), Pt::new(9.0, 5.0)), OutlineStyle::Solid, FillStyle::None, 1, 12, 12);
        assert_eq!(c.rect.x, 0);
        assert_eq!(c.line_at(0, 5), 255);
        assert_eq!(c.line_at(9, 5), 255);
        assert_eq!(c.line_at(10, 5), 0);
        let off = render(ShapeKind::Oval, &ShapeGeom::Box(Pt::new(-50.0, -50.0), Pt::new(-10.0, -10.0)), OutlineStyle::Solid, FillStyle::Solid, 3, 12, 12);
        assert!(off.rect.is_empty());
    }

    #[test]
    fn polygon_open_and_closed() {
        let pts = vec![Pt::new(2.0, 2.0), Pt::new(20.0, 2.0), Pt::new(11.0, 18.0)];
        let c = render(ShapeKind::Polygon, &ShapeGeom::Polygon { points: pts.clone(), closed: false }, OutlineStyle::Solid, FillStyle::Solid, 1, 30, 30);
        assert_eq!(c.fill.count_set(), 0);
        assert!(c.line_at(11, 2) > 0);
        let c = render(ShapeKind::Polygon, &ShapeGeom::Polygon { points: pts, closed: true }, OutlineStyle::Solid, FillStyle::Solid, 1, 30, 30);
        assert!(c.fill_at(11, 7) > 0);
    }

    #[test]
    fn geometry_helpers() {
        let mut g = ShapeGeom::Box(Pt::new(0.0, 0.0), Pt::new(10.0, 10.0));
        g.translate(Pt::new(5.0, 1.0));
        assert_eq!(g.bounds(), (Pt::new(5.0, 1.0), Pt::new(15.0, 11.0)));
        g.remap(g.bounds(), (Pt::new(0.0, 0.0), Pt::new(20.0, 5.0)));
        assert_eq!(g, ShapeGeom::Box(Pt::new(0.0, 0.0), Pt::new(20.0, 5.0)));
        let c = constrain(ShapeKind::Rectangle, Pt::new(0.0, 0.0), Pt::new(10.0, -4.0));
        assert_eq!(c, Pt::new(10.0, -10.0));
        let l = constrain(ShapeKind::Line, Pt::new(0.0, 0.0), Pt::new(10.0, 1.0));
        assert!(l.y.abs() < 1e-4);
    }
}
