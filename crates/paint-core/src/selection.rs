//! Rectangular and free-form selections, optionally holding "floating" (lifted) pixels.

use crate::color::Rgba;
use crate::geom::{IRect, Pt};
use crate::image::{DrawOptions, Image, Mask, Transform};
use crate::raster;

#[derive(Clone, Debug, PartialEq)]
pub struct Selection {
    /// Position and size on the canvas.
    pub rect: IRect,
    /// Free-form coverage at its original size; `None` means the whole rectangle.
    mask: Option<Mask>,
    /// Lifted pixels at their original size; `None` while they still live in the layer.
    floating: Option<Image>,
    /// Outline sub-paths in coordinates relative to `rect` (0..1), for display.
    outline: Vec<Vec<Pt>>,
    /// Caches of `floating` and `mask` resampled to `rect` size.
    scaled: Option<Image>,
    scaled_mask: Option<Mask>,
}

fn rect_outline() -> Vec<Vec<Pt>> {
    vec![vec![Pt::new(0.0, 0.0), Pt::new(1.0, 0.0), Pt::new(1.0, 1.0), Pt::new(0.0, 1.0)]]
}

impl Selection {
    pub fn rect(rect: IRect) -> Self {
        Selection { rect, mask: None, floating: None, outline: rect_outline(), scaled: None, scaled_mask: None }
    }

    /// Lasso selection from a polyline in canvas coordinates. `None` when it encloses no pixel.
    pub fn freeform(points: &[Pt], canvas_w: u32, canvas_h: u32) -> Option<Self> {
        if points.len() < 3 {
            return None;
        }
        let full = raster::polygon_mask(canvas_w, canvas_h, points);
        let bbox = full.bbox()?;
        let mask = full.sub_mask(bbox);
        let outline = vec![points
            .iter()
            .map(|p| Pt::new((p.x - bbox.x as f32) / bbox.w as f32, (p.y - bbox.y as f32) / bbox.h as f32))
            .collect()];
        let mut s = Selection { rect: bbox, mask: Some(mask), floating: None, outline, scaled: None, scaled_mask: None };
        s.refresh();
        Some(s)
    }

    /// Floating selection holding `image` at (`x`, `y`) (paste).
    pub fn pasted(image: Image, x: i32, y: i32) -> Self {
        let rect = IRect::new(x, y, image.width() as i32, image.height() as i32);
        let mut s = Selection::rect(rect);
        s.floating = Some(image);
        s.refresh();
        s
    }

    /// Everything except `self` within a canvas of the given size.
    pub fn inverted(&self, canvas_w: u32, canvas_h: u32) -> Self {
        let mut m = Mask::full(canvas_w, canvas_h);
        let own = self.rect;
        for y in own.y.max(0)..own.bottom().min(canvas_h as i32) {
            for x in own.x.max(0)..own.right().min(canvas_w as i32) {
                if self.contains(x, y) {
                    m.set(x, y, 0);
                }
            }
        }
        let rect = IRect::new(0, 0, canvas_w as i32, canvas_h as i32);
        let to_rel = |p: Pt| Pt::new(p.x / canvas_w as f32, p.y / canvas_h as f32);
        let mut outline = rect_outline();
        for path in &self.outline {
            outline.push(
                path.iter()
                    .map(|p| to_rel(Pt::new(self.rect.x as f32 + p.x * self.rect.w as f32, self.rect.y as f32 + p.y * self.rect.h as f32)))
                    .collect(),
            );
        }
        let mut s = Selection { rect, mask: Some(m), floating: None, outline, scaled: None, scaled_mask: None };
        s.refresh();
        s
    }

    pub fn is_floating(&self) -> bool {
        self.floating.is_some()
    }

    pub fn is_freeform(&self) -> bool {
        self.mask.is_some()
    }

    pub fn outline(&self) -> &[Vec<Pt>] {
        &self.outline
    }

    /// Outline in canvas coordinates.
    pub fn outline_canvas(&self) -> Vec<Vec<Pt>> {
        let r = self.rect;
        self.outline
            .iter()
            .map(|path| path.iter().map(|p| Pt::new(r.x as f32 + p.x * r.w as f32, r.y as f32 + p.y * r.h as f32)).collect())
            .collect()
    }

    /// Recomputes the size-dependent caches after `rect` changes.
    pub fn refresh(&mut self) {
        let (w, h) = (self.rect.w.max(1) as u32, self.rect.h.max(1) as u32);
        self.scaled = self.floating.as_ref().map(|f| if f.width() == w && f.height() == h { f.clone() } else { f.resized(w, h) });
        self.scaled_mask = self.mask.as_ref().map(|m| if m.width() == w && m.height() == h { m.clone() } else { m.resized(w, h) });
    }

    /// Mask at the current size (None = full rectangle).
    pub fn current_mask(&self) -> Option<&Mask> {
        self.scaled_mask.as_ref()
    }

    /// Floating pixels at the current size.
    pub fn current_image(&self) -> Option<&Image> {
        self.scaled.as_ref()
    }

    /// Whether the canvas pixel (x, y) is selected.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        if !self.rect.contains(x, y) {
            return false;
        }
        match &self.scaled_mask {
            Some(m) => m.get(x - self.rect.x, y - self.rect.y) >= 128,
            None => true,
        }
    }

    pub fn contains_pt(&self, p: Pt) -> bool {
        self.contains(p.x.floor() as i32, p.y.floor() as i32)
    }

    /// Selected pixels of `layer` (or the floating pixels), transparent outside the mask.
    pub fn content(&self, layer: &Image) -> Image {
        let mut img = match &self.scaled {
            Some(f) => f.clone(),
            None => layer.sub_image(self.rect),
        };
        if let Some(m) = &self.scaled_mask {
            for y in 0..img.height() as i32 {
                for x in 0..img.width() as i32 {
                    if m.get(x, y) < 128 {
                        img.set(x, y, Rgba::TRANSPARENT);
                    }
                }
            }
        }
        img
    }

    /// Lifts the selected pixels out of `layer`. Unless `keep_source`, the hole is
    /// filled with `hole` (the secondary colour, or transparent on upper layers).
    pub fn lift(&mut self, layer: &mut Image, hole: Rgba, keep_source: bool) {
        if self.floating.is_some() {
            return;
        }
        let content = self.content(layer);
        if !keep_source {
            self.clear_area(layer, hole);
        }
        self.floating = Some(content);
        self.mask = self.scaled_mask.clone();
        self.refresh();
    }

    /// Fills the selected area of `layer` with `c`.
    pub fn clear_area(&self, layer: &mut Image, c: Rgba) {
        match &self.scaled_mask {
            Some(m) => layer.fill_masked(self.rect.x, self.rect.y, m, c),
            None => layer.fill_rect(self.rect, c),
        }
    }

    /// Stamps the floating pixels onto `layer` (the selection stays floating).
    /// `key` skips pixels of that colour (transparent selection mode).
    pub fn stamp(&self, layer: &mut Image, key: Option<Rgba>) {
        if let Some(img) = &self.scaled {
            layer.draw(img, self.rect.x, self.rect.y, &DrawOptions { mask: self.scaled_mask.as_ref(), key, replace: false });
        }
    }

    /// Transforms the floating pixels (rotate/flip/resize/skew of a selection).
    pub fn transform(&mut self, t: Transform) {
        if let Some(img) = &self.scaled {
            let out = t.apply(img, Rgba::TRANSPARENT);
            self.mask = self.scaled_mask.as_ref().map(|m| m.map_image(|i| t.apply(i, Rgba::TRANSPARENT)));
            self.rect = IRect::new(self.rect.x, self.rect.y, out.width() as i32, out.height() as i32);
            self.floating = Some(out);
            self.outline = match self.outline.iter().map(|path| path.iter().map(|p| t.map_relative(*p)).collect::<Option<Vec<_>>>()).collect() {
                Some(o) => o,
                None => rect_outline(),
            };
            self.refresh();
        }
    }

    pub fn move_to(&mut self, x: i32, y: i32) {
        self.rect.x = x;
        self.rect.y = y;
    }

    pub fn resize_to(&mut self, r: IRect) {
        self.rect = IRect::new(r.x, r.y, r.w.max(1), r.h.max(1));
        self.refresh();
    }

    pub fn byte_size(&self) -> usize {
        self.floating.as_ref().map_or(0, Image::byte_size) + self.scaled.as_ref().map_or(0, Image::byte_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker() -> Image {
        let mut img = Image::filled(10, 10, Rgba::WHITE);
        img.fill_rect(IRect::new(2, 2, 4, 4), Rgba::rgb(255, 0, 0));
        img
    }

    #[test]
    fn rect_lift_and_stamp() {
        let mut layer = checker();
        let mut s = Selection::rect(IRect::new(2, 2, 4, 4));
        assert!(s.contains(2, 2) && !s.contains(6, 6));
        s.lift(&mut layer, Rgba::WHITE, false);
        assert!(s.is_floating());
        assert!(layer.is_uniform(Rgba::WHITE));
        s.move_to(5, 5);
        s.stamp(&mut layer, None);
        assert_eq!(layer.get(5, 5), Rgba::rgb(255, 0, 0));
        assert_eq!(layer.get(8, 8), Rgba::rgb(255, 0, 0));
        assert_eq!(layer.get(9, 9), Rgba::WHITE);
    }

    #[test]
    fn freeform_mask() {
        let tri = [Pt::new(0.0, 0.0), Pt::new(10.0, 0.0), Pt::new(0.0, 10.0)];
        let s = Selection::freeform(&tri, 10, 10).unwrap();
        assert!(s.is_freeform());
        assert!(s.contains(1, 1));
        assert!(!s.contains(8, 8));
        assert!(Selection::freeform(&tri[..2], 10, 10).is_none());
        assert!(Selection::freeform(&[Pt::new(1.0, 1.0), Pt::new(1.2, 1.0), Pt::new(1.1, 1.1)], 10, 10).is_none());
    }

    #[test]
    fn inverted() {
        let s = Selection::rect(IRect::new(2, 2, 3, 3));
        let inv = s.inverted(10, 10);
        assert_eq!(inv.rect, IRect::new(0, 0, 10, 10));
        assert!(!inv.contains(3, 3));
        assert!(inv.contains(0, 0) && inv.contains(9, 9));
        assert_eq!(inv.outline().len(), 2);
    }

    #[test]
    fn transform_and_resize() {
        let mut layer = checker();
        let mut s = Selection::rect(IRect::new(0, 0, 6, 4));
        s.lift(&mut layer, Rgba::WHITE, true);
        s.transform(Transform::RotateCw);
        assert_eq!((s.rect.w, s.rect.h), (4, 6));
        s.resize_to(IRect::new(0, 0, 8, 12));
        assert_eq!(s.current_image().unwrap().width(), 8);
        let c = s.content(&layer);
        assert_eq!((c.width(), c.height()), (8, 12));
        let o = s.outline_canvas();
        assert_eq!(o[0].len(), 4);
        assert!(o[0].iter().all(|p| p.x >= 0.0 && p.x <= 8.0 && p.y >= 0.0 && p.y <= 12.0));
    }

    #[test]
    fn pasted_selection_floats() {
        let s = Selection::pasted(Image::filled(3, 2, Rgba::BLACK), 4, 5);
        assert!(s.is_floating());
        assert_eq!(s.rect, IRect::new(4, 5, 3, 2));
        let mut layer = Image::filled(10, 10, Rgba::WHITE);
        s.stamp(&mut layer, Some(Rgba::BLACK));
        assert!(layer.is_uniform(Rgba::WHITE));
        s.stamp(&mut layer, None);
        assert_eq!(layer.get(6, 6), Rgba::BLACK);
    }
}
