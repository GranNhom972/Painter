//! Points and integer rectangles in image space.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

impl Pt {
    pub const fn new(x: f32, y: f32) -> Self {
        Pt { x, y }
    }
    pub fn scale(self, k: f32) -> Pt {
        Pt::new(self.x * k, self.y * k)
    }
    pub fn len(self) -> f32 {
        self.x.hypot(self.y)
    }
    pub fn dist(self, o: Pt) -> f32 {
        (self - o).len()
    }
    pub fn lerp(self, o: Pt, t: f32) -> Pt {
        Pt::new(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
}

impl std::ops::Add for Pt {
    type Output = Pt;
    fn add(self, o: Pt) -> Pt {
        Pt::new(self.x + o.x, self.y + o.y)
    }
}

impl std::ops::Sub for Pt {
    type Output = Pt;
    fn sub(self, o: Pt) -> Pt {
        Pt::new(self.x - o.x, self.y - o.y)
    }
}

/// Rectangle with integer origin and size. Empty when `w <= 0 || h <= 0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize)]
pub struct IRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl IRect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        IRect { x, y, w, h }
    }

    /// Rectangle spanned by two corners, in any order.
    pub fn from_corners(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        IRect::new(x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs())
    }

    /// Smallest rectangle containing every pixel touched by the float bounds.
    pub fn from_bounds(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        let l = x0.min(x1).floor() as i32;
        let t = y0.min(y1).floor() as i32;
        let r = x0.max(x1).ceil() as i32;
        let b = y0.max(y1).ceil() as i32;
        IRect::new(l, t, r - l, b - t)
    }

    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }
    pub fn is_empty(&self) -> bool {
        self.w <= 0 || self.h <= 0
    }
    pub fn area(&self) -> i64 {
        if self.is_empty() {
            0
        } else {
            self.w as i64 * self.h as i64
        }
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.right() && y < self.bottom()
    }
    pub fn contains_pt(&self, p: Pt) -> bool {
        p.x >= self.x as f32 && p.y >= self.y as f32 && p.x < self.right() as f32 && p.y < self.bottom() as f32
    }
    pub fn intersect(&self, o: &IRect) -> IRect {
        let x = self.x.max(o.x);
        let y = self.y.max(o.y);
        let r = self.right().min(o.right());
        let b = self.bottom().min(o.bottom());
        if r <= x || b <= y {
            IRect::default()
        } else {
            IRect::new(x, y, r - x, b - y)
        }
    }
    pub fn union(&self, o: &IRect) -> IRect {
        if self.is_empty() {
            return *o;
        }
        if o.is_empty() {
            return *self;
        }
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        IRect::new(x, y, self.right().max(o.right()) - x, self.bottom().max(o.bottom()) - y)
    }
    pub fn expand(&self, n: i32) -> IRect {
        IRect::new(self.x - n, self.y - n, self.w + 2 * n, self.h + 2 * n)
    }
    pub fn translate(&self, dx: i32, dy: i32) -> IRect {
        IRect::new(self.x + dx, self.y + dy, self.w, self.h)
    }
    pub fn clamp_to(&self, w: u32, h: u32) -> IRect {
        self.intersect(&IRect::new(0, 0, w as i32, h as i32))
    }
}

/// Union of two optional dirty rectangles.
pub fn union_opt(a: Option<IRect>, b: IRect) -> Option<IRect> {
    if b.is_empty() {
        return a;
    }
    Some(match a {
        Some(a) => a.union(&b),
        None => b,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_ops() {
        let a = IRect::from_corners(10, 10, 0, 0);
        assert_eq!(a, IRect::new(0, 0, 10, 10));
        let b = IRect::new(5, 5, 10, 10);
        assert_eq!(a.intersect(&b), IRect::new(5, 5, 5, 5));
        assert_eq!(a.union(&b), IRect::new(0, 0, 15, 15));
        assert!(a.intersect(&IRect::new(20, 20, 1, 1)).is_empty());
        assert!(a.contains(9, 9) && !a.contains(10, 0));
        assert_eq!(IRect::new(-5, -5, 20, 20).clamp_to(10, 8), IRect::new(0, 0, 10, 8));
        assert_eq!(IRect::from_bounds(1.5, 2.2, 0.2, 4.0), IRect::new(0, 2, 2, 2));
        assert_eq!(union_opt(None, IRect::default()), None);
        assert_eq!(union_opt(None, a), Some(a));
    }

    #[test]
    fn point_math() {
        let p = Pt::new(3.0, 4.0);
        assert_eq!(p.len(), 5.0);
        assert_eq!(p.lerp(Pt::new(5.0, 4.0), 0.5), Pt::new(4.0, 4.0));
        assert_eq!((p - Pt::new(1.0, 1.0)).scale(2.0), Pt::new(4.0, 6.0));
    }
}
