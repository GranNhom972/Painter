//! Anti-aliased coverage rasterisation into [`Mask`]s.

use crate::geom::{IRect, Pt};
use crate::image::Mask;

#[inline]
fn to_u8(c: f32) -> u8 {
    (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// Distance from `p` to the segment `a`-`b`.
#[inline]
pub fn dist_to_segment(p: Pt, a: Pt, b: Pt) -> f32 {
    let ab = b - a;
    let l2 = ab.x * ab.x + ab.y * ab.y;
    if l2 == 0.0 {
        return p.dist(a);
    }
    let t = ((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / l2;
    p.dist(a.lerp(b, t.clamp(0.0, 1.0)))
}

/// Anti-aliased disc. Returns the touched rectangle.
pub fn stamp_disc(mask: &mut Mask, c: Pt, radius: f32, strength: f32) -> IRect {
    stamp_capsule(mask, c, c, radius, strength)
}

/// Anti-aliased capsule (thick segment with round caps), combined with `max`.
pub fn stamp_capsule(mask: &mut Mask, a: Pt, b: Pt, radius: f32, strength: f32) -> IRect {
    let r = radius.max(0.5);
    let bounds = IRect::from_bounds(a.x.min(b.x) - r - 1.0, a.y.min(b.y) - r - 1.0, a.x.max(b.x) + r + 1.0, a.y.max(b.y) + r + 1.0)
        .clamp_to(mask.width(), mask.height());
    let d = b - a;
    let len = d.len();
    for y in bounds.y..bounds.bottom() {
        // For steep segments only scan the band around the line on this row,
        // instead of the whole bounding box.
        let (x0, x1) = if len > 0.0 && d.y.abs() > 0.25 * len {
            let yc = y as f32 + 0.5;
            let reach = (r + 1.5) * len / d.y.abs();
            let xc = a.x + (yc - a.y) * d.x / d.y;
            (((xc - reach).floor() as i32).max(bounds.x), ((xc + reach).ceil() as i32 + 1).min(bounds.right()))
        } else {
            (bounds.x, bounds.right())
        };
        for x in x0..x1 {
            let dist = dist_to_segment(Pt::new(x as f32 + 0.5, y as f32 + 0.5), a, b);
            let cov = (r + 0.5 - dist).clamp(0.0, 1.0) * strength;
            if cov > 0.0 {
                mask.max_at(x, y, to_u8(cov));
            }
        }
    }
    bounds
}

/// Soft disc whose coverage falls off towards the rim (watercolour, airbrush halos).
pub fn stamp_soft_disc(mask: &mut Mask, c: Pt, radius: f32, strength: f32) -> IRect {
    let r = radius.max(1.0);
    let bounds = IRect::from_bounds(c.x - r - 1.0, c.y - r - 1.0, c.x + r + 1.0, c.y + r + 1.0).clamp_to(mask.width(), mask.height());
    for y in bounds.y..bounds.bottom() {
        for x in bounds.x..bounds.right() {
            let d = Pt::new(x as f32 + 0.5, y as f32 + 0.5).dist(c) / r;
            if d < 1.0 {
                let cov = (1.0 - d).powf(1.5) * strength;
                mask.max_at(x, y, to_u8(cov));
            }
        }
    }
    bounds
}

/// Hard-edged axis-aligned square centred on `c` (eraser, marker nib).
pub fn stamp_square(mask: &mut Mask, c: Pt, side: f32, strength: f32) -> IRect {
    let half = side.max(1.0) / 2.0;
    let x0 = (c.x - half).round() as i32;
    let y0 = (c.y - half).round() as i32;
    let n = side.max(1.0).round() as i32;
    let r = IRect::new(x0, y0, n, n).clamp_to(mask.width(), mask.height());
    let v = to_u8(strength);
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            mask.max_at(x, y, v);
        }
    }
    r
}

/// Hard-edged (aliased) disc used by the pencil.
pub fn stamp_hard_disc(mask: &mut Mask, cx: i32, cy: i32, size: u32) -> IRect {
    if size <= 1 {
        mask.max_at(cx, cy, 255);
        return IRect::new(cx, cy, 1, 1).clamp_to(mask.width(), mask.height());
    }
    let r = size as f32 / 2.0;
    let off = if size.is_multiple_of(2) { 0.0 } else { 0.5 };
    let center = Pt::new(cx as f32 + off, cy as f32 + off);
    let n = size as i32;
    let rect = IRect::new(cx - n / 2 - 1, cy - n / 2 - 1, n + 2, n + 2).clamp_to(mask.width(), mask.height());
    for y in rect.y..rect.bottom() {
        for x in rect.x..rect.right() {
            if Pt::new(x as f32 + 0.5, y as f32 + 0.5).dist(center) <= r {
                mask.max_at(x, y, 255);
            }
        }
    }
    rect
}

/// Integer Bresenham line, endpoints included.
pub fn line_pixels(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let mut pts = Vec::new();
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    loop {
        pts.push((x, y));
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
    pts
}

/// Fill rule for polygon rasterisation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

/// Anti-aliased polygon fill (several closed sub-paths), 4 vertical samples per pixel
/// with exact horizontal span coverage.
pub fn fill_polygons(mask: &mut Mask, paths: &[Vec<Pt>], rule: FillRule, strength: f32) -> IRect {
    const SUB: usize = 4;
    let mut edges: Vec<(Pt, Pt, i32)> = Vec::new();
    let (mut minx, mut miny, mut maxx, mut maxy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for path in paths {
        if path.len() < 3 {
            continue;
        }
        for i in 0..path.len() {
            let a = path[i];
            let b = path[(i + 1) % path.len()];
            minx = minx.min(a.x);
            maxx = maxx.max(a.x);
            miny = miny.min(a.y);
            maxy = maxy.max(a.y);
            if a.y != b.y {
                let dir = if b.y > a.y { 1 } else { -1 };
                edges.push((a, b, dir));
            }
        }
    }
    if edges.is_empty() {
        return IRect::default();
    }
    let bounds = IRect::from_bounds(minx, miny, maxx, maxy).clamp_to(mask.width(), mask.height());
    if bounds.is_empty() {
        return bounds;
    }
    let w = bounds.w as usize;
    let mut acc = vec![0f32; w + 1];
    let mut xs: Vec<(f32, i32)> = Vec::with_capacity(16);
    for y in bounds.y..bounds.bottom() {
        acc.iter_mut().for_each(|v| *v = 0.0);
        for s in 0..SUB {
            let sy = y as f32 + (s as f32 + 0.5) / SUB as f32;
            xs.clear();
            for &(a, b, dir) in &edges {
                let (lo, hi) = if a.y < b.y { (a.y, b.y) } else { (b.y, a.y) };
                if sy >= lo && sy < hi {
                    let t = (sy - a.y) / (b.y - a.y);
                    xs.push((a.x + (b.x - a.x) * t, dir));
                }
            }
            xs.sort_by(|p, q| p.0.total_cmp(&q.0));
            let mut winding = 0;
            for i in 0..xs.len() {
                let inside_before = match rule {
                    FillRule::NonZero => winding != 0,
                    FillRule::EvenOdd => winding % 2 != 0,
                };
                winding += match rule {
                    FillRule::NonZero => xs[i].1,
                    FillRule::EvenOdd => 1,
                };
                let inside_after = match rule {
                    FillRule::NonZero => winding != 0,
                    FillRule::EvenOdd => winding % 2 != 0,
                };
                if !inside_before && inside_after {
                    // Find the end of the span.
                    let start = xs[i].0;
                    let mut wnd = winding;
                    let mut end = start;
                    for &(x, d) in xs.iter().skip(i + 1) {
                        wnd += match rule {
                            FillRule::NonZero => d,
                            FillRule::EvenOdd => 1,
                        };
                        let inside = match rule {
                            FillRule::NonZero => wnd != 0,
                            FillRule::EvenOdd => wnd % 2 != 0,
                        };
                        end = x;
                        if !inside {
                            break;
                        }
                    }
                    add_span(&mut acc, start - bounds.x as f32, end - bounds.x as f32, w, 1.0 / SUB as f32);
                }
            }
        }
        for (i, v) in acc.iter().take(w).enumerate() {
            if *v > 0.0 {
                mask.max_at(bounds.x + i as i32, y, to_u8(v * strength));
            }
        }
    }
    bounds
}

fn add_span(acc: &mut [f32], x0: f32, x1: f32, w: usize, weight: f32) {
    let x0 = x0.max(0.0);
    let x1 = x1.min(w as f32);
    if x1 <= x0 {
        return;
    }
    let (i0, i1) = (x0.floor() as usize, (x1.ceil() as usize).min(w));
    for (i, slot) in acc.iter_mut().enumerate().take(i1).skip(i0) {
        let l = x0.max(i as f32);
        let r = x1.min(i as f32 + 1.0);
        if r > l {
            *slot += (r - l) * weight;
        }
    }
}

/// Strokes a polyline (optionally closed) with round joins and caps.
pub fn stroke_polyline(mask: &mut Mask, pts: &[Pt], width: f32, closed: bool, strength: f32) -> IRect {
    let r = width / 2.0;
    let mut dirty = IRect::default();
    if pts.len() == 1 {
        return stamp_disc(mask, pts[0], r, strength);
    }
    let n = if closed { pts.len() } else { pts.len() - 1 };
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        dirty = dirty.union(&stamp_capsule(mask, a, b, r, strength));
    }
    dirty
}

/// Flattened cubic Bézier.
pub fn cubic_bezier(p0: Pt, p1: Pt, p2: Pt, p3: Pt) -> Vec<Pt> {
    let approx = p0.dist(p1) + p1.dist(p2) + p2.dist(p3);
    let n = ((approx / 3.0) as usize).clamp(8, 400);
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            Pt::new(a * p0.x + b * p1.x + c * p2.x + d * p3.x, a * p0.y + b * p1.y + c * p2.y + d * p3.y)
        })
        .collect()
}

/// Polygonal approximation of an ellipse inscribed in the given bounds.
pub fn ellipse_points(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<Pt> {
    let n = ((rx.abs() + ry.abs()) * 1.2).clamp(24.0, 720.0) as usize;
    (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            Pt::new(cx + rx * a.cos(), cy + ry * a.sin())
        })
        .collect()
}

/// Hard-edged polygon mask (used by free-form selection).
pub fn polygon_mask(w: u32, h: u32, pts: &[Pt]) -> Mask {
    let mut m = Mask::new(w, h);
    fill_polygons(&mut m, &[pts.to_vec()], FillRule::NonZero, 1.0);
    m.threshold(128);
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bresenham() {
        assert_eq!(line_pixels(0, 0, 3, 0), vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
        assert_eq!(line_pixels(0, 0, 2, 2), vec![(0, 0), (1, 1), (2, 2)]);
        assert_eq!(line_pixels(2, 1, 2, 1), vec![(2, 1)]);
        let l = line_pixels(5, 0, 0, 3);
        assert_eq!(l.first(), Some(&(5, 0)));
        assert_eq!(l.last(), Some(&(0, 3)));
    }

    #[test]
    fn capsule_coverage() {
        let mut m = Mask::new(20, 20);
        stamp_capsule(&mut m, Pt::new(2.5, 10.5), Pt::new(17.5, 10.5), 2.0, 1.0);
        assert_eq!(m.get(10, 10), 255);
        assert_eq!(m.get(10, 11), 255);
        // Pixel row 12 is crossed by the edge: half covered.
        assert!(m.get(10, 12) > 0 && m.get(10, 12) < 255);
        assert_eq!(m.get(10, 14), 0);
        assert!(m.get(0, 10) < 255); // round cap edge
        assert_eq!(m.get(19, 2), 0);
    }

    #[test]
    fn steep_capsules_match_brute_force() {
        // The per-row band must cover exactly what a full scan covers.
        for (a, b, r) in [
            (Pt::new(3.0, 2.0), Pt::new(17.0, 38.0), 2.0),
            (Pt::new(30.0, 1.0), Pt::new(2.5, 39.0), 0.5),
            (Pt::new(10.0, 5.0), Pt::new(14.0, 30.0), 6.0),
            (Pt::new(5.0, 35.0), Pt::new(35.0, 5.0), 3.5),
        ] {
            let mut fast = Mask::new(40, 40);
            stamp_capsule(&mut fast, a, b, r, 1.0);
            let mut full = Mask::new(40, 40);
            for y in 0..40 {
                for x in 0..40 {
                    let d = dist_to_segment(Pt::new(x as f32 + 0.5, y as f32 + 0.5), a, b);
                    let cov = (r.max(0.5) + 0.5 - d).clamp(0.0, 1.0);
                    if cov > 0.0 {
                        full.max_at(x, y, to_u8(cov));
                    }
                }
            }
            assert_eq!(fast, full, "{a:?} {b:?} {r}");
        }
    }

    #[test]
    fn polygon_fill_square_is_exact() {
        let mut m = Mask::new(10, 10);
        let sq = vec![Pt::new(2.0, 2.0), Pt::new(6.0, 2.0), Pt::new(6.0, 6.0), Pt::new(2.0, 6.0)];
        fill_polygons(&mut m, &[sq], FillRule::NonZero, 1.0);
        assert_eq!(m.count_set(), 16);
        assert_eq!(m.get(2, 2), 255);
        assert_eq!(m.get(5, 5), 255);
        assert_eq!(m.get(6, 6), 0);
        // Half-pixel edges give half coverage.
        let mut m2 = Mask::new(4, 4);
        fill_polygons(&mut m2, &[vec![Pt::new(0.0, 0.0), Pt::new(1.5, 0.0), Pt::new(1.5, 1.0), Pt::new(0.0, 1.0)]], FillRule::NonZero, 1.0);
        assert_eq!(m2.get(0, 0), 255);
        assert!((126..=129).contains(&m2.get(1, 0)));
    }

    #[test]
    fn fill_rules() {
        let outer = vec![Pt::new(0.0, 0.0), Pt::new(10.0, 0.0), Pt::new(10.0, 10.0), Pt::new(0.0, 10.0)];
        let inner = vec![Pt::new(3.0, 3.0), Pt::new(7.0, 3.0), Pt::new(7.0, 7.0), Pt::new(3.0, 7.0)];
        let mut eo = Mask::new(10, 10);
        fill_polygons(&mut eo, &[outer.clone(), inner.clone()], FillRule::EvenOdd, 1.0);
        assert_eq!(eo.get(5, 5), 0);
        assert_eq!(eo.get(1, 1), 255);
        let mut nz = Mask::new(10, 10);
        fill_polygons(&mut nz, &[outer, inner], FillRule::NonZero, 1.0);
        assert_eq!(nz.get(5, 5), 255);
    }

    #[test]
    fn curves_and_ellipses() {
        let b = cubic_bezier(Pt::new(0.0, 0.0), Pt::new(0.0, 10.0), Pt::new(10.0, 10.0), Pt::new(10.0, 0.0));
        assert_eq!(b.first(), Some(&Pt::new(0.0, 0.0)));
        assert!(b.last().unwrap().dist(Pt::new(10.0, 0.0)) < 1e-4);
        let e = ellipse_points(10.0, 10.0, 5.0, 3.0);
        assert!(e.iter().all(|p| ((p.x - 10.0) / 5.0).powi(2) + ((p.y - 10.0) / 3.0).powi(2) < 1.001));
        let m = polygon_mask(20, 20, &e);
        assert_eq!(m.get(10, 10), 255);
        assert_eq!(m.get(1, 1), 0);
    }

    #[test]
    fn stamps() {
        let mut m = Mask::new(10, 10);
        stamp_square(&mut m, Pt::new(5.0, 5.0), 4.0, 1.0);
        assert_eq!(m.count_set(), 16);
        let mut h = Mask::new(10, 10);
        stamp_hard_disc(&mut h, 5, 5, 1);
        assert_eq!(h.count_set(), 1);
        let mut h3 = Mask::new(10, 10);
        stamp_hard_disc(&mut h3, 5, 5, 3);
        assert!(h3.get(5, 5) == 255 && h3.get(4, 5) == 255 && h3.get(5, 4) == 255);
        assert_eq!(h3.get(3, 5), 0);
        let mut s = Mask::new(20, 20);
        stamp_soft_disc(&mut s, Pt::new(10.0, 10.0), 6.0, 1.0);
        assert!(s.get(10, 10) > s.get(13, 10));
        assert_eq!(s.get(17, 10), 0);
    }
}
