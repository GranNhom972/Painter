#![allow(dead_code)]

use paint_core::{Button, Editor, IRect, Image, Mods, Rgba, Tool};

pub const L: Button = Button::Left;
pub const R: Button = Button::Right;
pub const NO: Mods = Mods { shift: false, ctrl: false, alt: false };
pub const SHIFT: Mods = Mods { shift: true, ctrl: false, alt: false };
pub const CTRL: Mods = Mods { shift: false, ctrl: true, alt: false };

pub const RED: Rgba = Rgba::rgb(237, 28, 36);
pub const BLUE: Rgba = Rgba::rgb(63, 72, 204);
pub const GREEN: Rgba = Rgba::rgb(34, 177, 76);
pub const WHITE: Rgba = Rgba::WHITE;
pub const BLACK: Rgba = Rgba::BLACK;

/// White canvas with a red square.
pub fn with_square(w: u32, h: u32, sq: IRect) -> Editor {
    let mut img = Image::filled(w, h, WHITE);
    img.fill_rect(sq, RED);
    Editor::from_image(img)
}

pub fn drag_with(e: &mut Editor, pts: &[(f32, f32)], b: Button, m: Mods) {
    let (x0, y0) = pts[0];
    e.pointer_down(x0, y0, b, m);
    for &(x, y) in &pts[1..] {
        e.pointer_move(x, y, m);
    }
    let (x1, y1) = *pts.last().unwrap();
    e.pointer_up(x1, y1);
}

pub fn drag(e: &mut Editor, from: (f32, f32), to: (f32, f32)) {
    drag_with(e, &[from, to], L, NO);
}

pub fn click(e: &mut Editor, x: f32, y: f32) {
    e.pointer_down(x, y, L, NO);
    e.pointer_up(x, y);
}

pub fn tool(e: &mut Editor, t: Tool) {
    e.set_tool(t);
}

/// Number of pixels of the composite inside `r` equal to `c`.
pub fn count(e: &Editor, r: IRect, c: Rgba) -> usize {
    let mut n = 0;
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            if e.composite_pixel(x, y) == c {
                n += 1;
            }
        }
    }
    n
}

/// Number of pixels of the composite that differ from `c` in the whole canvas.
pub fn count_not(e: &Editor, c: Rgba) -> usize {
    let r = IRect::new(0, 0, e.width() as i32, e.height() as i32);
    (r.area() as usize) - count(e, r, c)
}
