//! Layers and the layer stack.

use crate::color::{blend_over, Rgba};
use crate::geom::IRect;
use crate::image::{Image, Mask, Transform};
use crate::selection::Selection;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub id: u32,
    pub name: String,
    pub visible: bool,
    /// Shared with history snapshots; copied on first write.
    pub image: Arc<Image>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    width: u32,
    height: u32,
    pub layers: Vec<Layer>,
    pub active: usize,
    next_id: u32,
}

impl Document {
    pub fn new(width: u32, height: u32, background: Rgba) -> Self {
        Document::from_image(Image::filled(width.max(1), height.max(1), background))
    }

    pub fn from_image(img: Image) -> Self {
        let (width, height) = (img.width(), img.height());
        Document { width, height, layers: vec![Layer { id: 1, name: "Layer 1".into(), visible: true, image: Arc::new(img) }], active: 0, next_id: 2 }
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

    pub fn active_image(&self) -> &Image {
        &self.layers[self.active].image
    }

    pub fn active_image_mut(&mut self) -> &mut Image {
        Arc::make_mut(&mut self.layers[self.active].image)
    }

    pub fn active_arc(&self) -> Arc<Image> {
        self.layers[self.active].image.clone()
    }

    /// Colour left behind by erasing on the active layer: the secondary colour on the
    /// background layer, transparency on the layers above it.
    pub fn hole_color(&self, secondary: Rgba) -> Rgba {
        if self.active == 0 {
            secondary
        } else {
            Rgba::TRANSPARENT
        }
    }

    /// Composites visible layers into `out` (document-sized) within `rect`. A floating
    /// selection is drawn just above the active layer.
    pub fn composite_into(&self, out: &mut Image, rect: IRect, floating: Option<(&Selection, Option<Rgba>)>) {
        let rect = rect.clamp_to(self.width, self.height);
        if rect.is_empty() {
            return;
        }
        let w = self.width as usize;
        for y in rect.y..rect.bottom() {
            let row = (y as usize * w + rect.x as usize) * 4;
            let n = rect.w as usize * 4;
            out.data_mut()[row..row + n].iter_mut().for_each(|v| *v = 0);
        }
        for (i, layer) in self.layers.iter().enumerate() {
            if layer.visible {
                let src = layer.image.data();
                let dst = out.data_mut();
                for y in rect.y..rect.bottom() {
                    let row = (y as usize * w + rect.x as usize) * 4;
                    for k in (row..row + rect.w as usize * 4).step_by(4) {
                        let s = [src[k], src[k + 1], src[k + 2], src[k + 3]];
                        if s[3] == 0 {
                            continue;
                        }
                        let d = [dst[k], dst[k + 1], dst[k + 2], dst[k + 3]];
                        dst[k..k + 4].copy_from_slice(&blend_over(d, s));
                    }
                }
            }
            if i == self.active {
                if let Some((sel, key)) = floating {
                    if let Some(img) = sel.current_image() {
                        let r = sel.rect.intersect(&rect);
                        let mask = sel.current_mask();
                        for y in r.y..r.bottom() {
                            for x in r.x..r.right() {
                                let (sx, sy) = (x - sel.rect.x, y - sel.rect.y);
                                let mut p = img.get_raw(sx, sy);
                                if let Some(m) = mask {
                                    if m.get(sx, sy) < 128 {
                                        continue;
                                    }
                                }
                                if key == Some(Rgba::from_array(p)) {
                                    continue;
                                }
                                if p[3] == 0 {
                                    continue;
                                }
                                let d = out.get_raw(x, y);
                                p = blend_over(d, p);
                                out.set_raw(x, y, p);
                            }
                        }
                    }
                }
            }
        }
    }

    /// All visible layers merged into one image.
    pub fn flatten(&self, floating: Option<(&Selection, Option<Rgba>)>) -> Image {
        let mut out = Image::new(self.width, self.height);
        self.composite_into(&mut out, self.bounds(), floating);
        out
    }

    fn fresh_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Adds a transparent layer above the active one and activates it.
    pub fn add_layer(&mut self) -> usize {
        let id = self.fresh_id();
        let layer = Layer { id, name: format!("Layer {id}"), visible: true, image: Arc::new(Image::new(self.width, self.height)) };
        self.active += 1;
        self.layers.insert(self.active, layer);
        self.active
    }

    pub fn delete_layer(&mut self, i: usize) -> bool {
        if self.layers.len() <= 1 || i >= self.layers.len() {
            return false;
        }
        self.layers.remove(i);
        if self.active >= self.layers.len() || self.active > i || (self.active == i && i > 0) {
            self.active = self.active.saturating_sub(1).min(self.layers.len() - 1);
        }
        true
    }

    pub fn duplicate_layer(&mut self, i: usize) -> bool {
        if i >= self.layers.len() {
            return false;
        }
        let id = self.fresh_id();
        let mut copy = self.layers[i].clone();
        copy.id = id;
        copy.name = format!("{} (copy)", copy.name);
        self.layers.insert(i + 1, copy);
        self.active = i + 1;
        true
    }

    /// Merges layer `i` into the layer below it.
    pub fn merge_down(&mut self, i: usize) -> bool {
        if i == 0 || i >= self.layers.len() {
            return false;
        }
        let top = self.layers.remove(i);
        if top.visible {
            let below = Arc::make_mut(&mut self.layers[i - 1].image);
            below.draw(&top.image, 0, 0, &Default::default());
        }
        self.active = i - 1;
        true
    }

    /// Moves layer `from` to index `to`; the active layer follows its content.
    pub fn move_layer(&mut self, from: usize, to: usize) -> bool {
        if from >= self.layers.len() || to >= self.layers.len() || from == to {
            return false;
        }
        let active_id = self.layers[self.active].id;
        let l = self.layers.remove(from);
        self.layers.insert(to, l);
        self.active = self.layers.iter().position(|l| l.id == active_id).unwrap_or(0);
        true
    }

    pub fn set_active(&mut self, i: usize) -> bool {
        if i < self.layers.len() {
            self.active = i;
            true
        } else {
            false
        }
    }

    /// Applies a geometric transform to every layer. `bg` fills the area a skew
    /// uncovers on the background layer.
    pub fn transform_all(&mut self, t: Transform, bg: Rgba) {
        for (i, l) in self.layers.iter_mut().enumerate() {
            let fill = if i == 0 { bg } else { Rgba::TRANSPARENT };
            l.image = Arc::new(t.apply(&l.image, fill));
        }
        self.sync_size();
    }

    /// Changes the canvas size keeping pixels anchored top-left.
    pub fn resize_canvas(&mut self, w: u32, h: u32, bg: Rgba) {
        self.resize_canvas_at(w, h, 0, 0, bg);
    }

    /// Changes the canvas size, placing the old pixels at (`dx`, `dy`).
    pub fn resize_canvas_at(&mut self, w: u32, h: u32, dx: i32, dy: i32, bg: Rgba) {
        let (w, h) = (w.max(1), h.max(1));
        for (i, l) in self.layers.iter_mut().enumerate() {
            let fill = if i == 0 { bg } else { Rgba::TRANSPARENT };
            l.image = Arc::new(l.image.with_canvas_at(w, h, dx, dy, fill));
        }
        self.sync_size();
    }

    /// Crops every layer to `rect`. With a mask, pixels outside it become `bg`
    /// on the background layer and transparent elsewhere.
    pub fn crop(&mut self, rect: IRect, mask: Option<&Mask>, bg: Rgba) {
        for (i, l) in self.layers.iter_mut().enumerate() {
            let mut img = l.image.sub_image(rect);
            if let Some(m) = mask {
                let fill = if i == 0 { bg } else { Rgba::TRANSPARENT };
                for y in 0..img.height() as i32 {
                    for x in 0..img.width() as i32 {
                        if m.get(x, y) < 128 {
                            img.set(x, y, fill);
                        }
                    }
                }
            }
            l.image = Arc::new(img);
        }
        self.sync_size();
    }

    fn sync_size(&mut self) {
        let img = &self.layers[0].image;
        self.width = img.width();
        self.height = img.height();
    }

    pub fn byte_size(&self) -> usize {
        self.layers.iter().map(|l| l.image.byte_size()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composite_respects_order_and_visibility() {
        let mut d = Document::new(4, 4, Rgba::WHITE);
        d.add_layer();
        d.active_image_mut().set(1, 1, Rgba::rgb(255, 0, 0));
        assert_eq!(d.flatten(None).get(1, 1), Rgba::rgb(255, 0, 0));
        assert_eq!(d.flatten(None).get(0, 0), Rgba::WHITE);
        d.layers[1].visible = false;
        assert_eq!(d.flatten(None).get(1, 1), Rgba::WHITE);
    }

    #[test]
    fn layer_management() {
        let mut d = Document::new(4, 4, Rgba::WHITE);
        assert!(!d.delete_layer(0));
        assert_eq!(d.add_layer(), 1);
        assert_eq!(d.add_layer(), 2);
        assert_eq!(d.layers.iter().map(|l| l.id).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert!(d.move_layer(2, 0));
        assert_eq!(d.layers[0].id, 3);
        assert_eq!(d.active, 0);
        assert!(d.duplicate_layer(1));
        assert_eq!(d.layers.len(), 4);
        assert_eq!(d.active, 2);
        assert!(d.delete_layer(2));
        assert_eq!(d.layers.len(), 3);
        assert!(!d.merge_down(0));
        assert!(!d.set_active(9));
    }

    #[test]
    fn merge_down_blends() {
        let mut d = Document::new(2, 2, Rgba::WHITE);
        d.add_layer();
        d.active_image_mut().set(0, 0, Rgba::BLACK);
        assert!(d.merge_down(1));
        assert_eq!(d.layers.len(), 1);
        assert_eq!(d.active_image().get(0, 0), Rgba::BLACK);
        assert_eq!(d.active_image().get(1, 1), Rgba::WHITE);
    }

    #[test]
    fn transforms_resize_all_layers() {
        let mut d = Document::new(6, 3, Rgba::WHITE);
        d.add_layer();
        d.transform_all(Transform::RotateCw, Rgba::WHITE);
        assert_eq!((d.width(), d.height()), (3, 6));
        assert!(d.layers.iter().all(|l| l.image.width() == 3 && l.image.height() == 6));
        d.resize_canvas(10, 8, Rgba::BLACK);
        assert_eq!(d.layers[0].image.get(9, 7), Rgba::BLACK);
        assert_eq!(d.layers[1].image.get(9, 7), Rgba::TRANSPARENT);
        d.crop(IRect::new(1, 1, 2, 2), None, Rgba::WHITE);
        assert_eq!((d.width(), d.height()), (2, 2));
    }

    #[test]
    fn copy_on_write_layers() {
        let mut d = Document::new(2, 2, Rgba::WHITE);
        let snapshot = d.clone();
        d.active_image_mut().set(0, 0, Rgba::BLACK);
        assert_eq!(snapshot.active_image().get(0, 0), Rgba::WHITE);
        assert!(!Arc::ptr_eq(&snapshot.layers[0].image, &d.layers[0].image));
    }
}
