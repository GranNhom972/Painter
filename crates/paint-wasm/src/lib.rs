//! WebAssembly API of the Paint engine, used by the UI (`ui/js`).
//!
//! The composited frame lives in WebAssembly memory: JS wraps it with
//! `new Uint8ClampedArray(memory.buffer, frame_ptr(), frame_len())` and blits
//! the rectangle returned by `render()`.

use paint_core::editor::Editor;
use paint_core::{codec, BrushKind, Button, FillStyle, Format, IRect, Image, Mods, OutlineStyle, Rgba, ShapeKind, Tool, Transform};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn button(b: u8) -> Button {
    if b == 2 {
        Button::Right
    } else {
        Button::Left
    }
}

fn err(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

#[wasm_bindgen]
pub struct Engine {
    ed: Editor,
}

#[wasm_bindgen]
impl Engine {
    #[wasm_bindgen(constructor)]
    pub fn new(width: u32, height: u32) -> Engine {
        Engine { ed: Editor::new(width.max(1), height.max(1)) }
    }

    pub fn width(&self) -> u32 {
        self.ed.width()
    }

    pub fn height(&self) -> u32 {
        self.ed.height()
    }

    /// Updates the frame; returns `[x, y, w, h]` of the changed area, or an empty array.
    pub fn render(&mut self) -> Vec<i32> {
        match self.ed.render() {
            Some(r) => vec![r.x, r.y, r.w, r.h],
            None => Vec::new(),
        }
    }

    /// Requests a full redraw on the next `render()`.
    pub fn invalidate(&mut self) {
        self.ed.invalidate();
    }

    pub fn frame_ptr(&self) -> *const u8 {
        self.ed.frame().data().as_ptr()
    }

    pub fn frame_len(&self) -> usize {
        self.ed.frame().data().len()
    }

    // ----- pointer ---------------------------------------------------------

    pub fn pointer_down(&mut self, x: f32, y: f32, btn: u8, shift: bool, ctrl: bool, alt: bool) {
        self.ed.pointer_down(x, y, button(btn), Mods { shift, ctrl, alt });
    }

    pub fn pointer_move(&mut self, x: f32, y: f32, shift: bool, ctrl: bool, alt: bool) {
        self.ed.pointer_move(x, y, Mods { shift, ctrl, alt });
    }

    pub fn pointer_up(&mut self, x: f32, y: f32) {
        self.ed.pointer_up(x, y);
    }

    pub fn double_click(&mut self, x: f32, y: f32) {
        self.ed.double_click(x, y);
    }

    pub fn tick(&mut self) {
        self.ed.tick();
    }

    pub fn cursor_at(&self, x: f32, y: f32) -> String {
        self.ed.cursor_at(x, y).to_string()
    }

    pub fn state_json(&self) -> String {
        self.ed.state_json()
    }

    // ----- settings --------------------------------------------------------

    pub fn set_tool(&mut self, name: &str) -> bool {
        Tool::from_name(name).map(|t| self.ed.set_tool(t)).is_some()
    }

    pub fn set_brush(&mut self, name: &str) -> bool {
        BrushKind::from_name(name).map(|b| self.ed.set_brush(b)).is_some()
    }

    pub fn set_shape(&mut self, name: &str) -> bool {
        ShapeKind::from_name(name).map(|s| self.ed.set_shape(s)).is_some()
    }

    pub fn set_color1(&mut self, hex: &str) -> bool {
        Rgba::from_hex(hex).map(|c| self.ed.set_color1(c)).is_some()
    }

    pub fn set_color2(&mut self, hex: &str) -> bool {
        Rgba::from_hex(hex).map(|c| self.ed.set_color2(c)).is_some()
    }

    pub fn swap_colors(&mut self) {
        self.ed.swap_colors();
    }

    pub fn set_size(&mut self, size: u32) {
        self.ed.set_size(size);
    }

    pub fn set_outline(&mut self, name: &str) {
        self.ed.set_outline(if name == "none" { OutlineStyle::None } else { OutlineStyle::Solid });
    }

    pub fn set_fill(&mut self, name: &str) {
        self.ed.set_fill(if name == "solid" { FillStyle::Solid } else { FillStyle::None });
    }

    pub fn set_transparent_selection(&mut self, on: bool) {
        self.ed.set_transparent_selection(on);
    }

    pub fn set_fill_tolerance(&mut self, t: u8) {
        self.ed.set_fill_tolerance(t);
    }

    pub fn set_zoom(&mut self, z: f32) {
        self.ed.set_zoom(z);
    }

    // ----- commands --------------------------------------------------------

    pub fn undo(&mut self) -> bool {
        self.ed.undo()
    }

    pub fn redo(&mut self) -> bool {
        self.ed.redo()
    }

    pub fn select_all(&mut self) {
        self.ed.select_all();
    }

    pub fn select_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.ed.select_rect(IRect::new(x, y, w, h));
    }

    pub fn invert_selection(&mut self) {
        self.ed.invert_selection();
    }

    pub fn deselect(&mut self) {
        self.ed.deselect();
    }

    pub fn delete_selection(&mut self) -> bool {
        self.ed.delete_selection()
    }

    pub fn crop(&mut self) -> bool {
        self.ed.crop()
    }

    pub fn nudge(&mut self, dx: i32, dy: i32) -> bool {
        self.ed.nudge(dx, dy)
    }

    pub fn escape(&mut self) {
        self.ed.escape();
    }

    pub fn enter(&mut self) {
        self.ed.enter();
    }

    /// Rotation in degrees: 90 (clockwise), -90/270 or 180.
    pub fn rotate(&mut self, degrees: i32) -> bool {
        let t = match degrees.rem_euclid(360) {
            90 => Transform::RotateCw,
            180 => Transform::Rotate180,
            270 => Transform::RotateCcw,
            _ => return false,
        };
        self.ed.transform(t);
        true
    }

    pub fn flip_horizontal(&mut self) {
        self.ed.transform(Transform::FlipHorizontal);
    }

    pub fn flip_vertical(&mut self) {
        self.ed.transform(Transform::FlipVertical);
    }

    /// Scales the selection, or the whole image without one.
    pub fn resize_image(&mut self, w: u32, h: u32) {
        self.ed.transform(Transform::Resize(w.clamp(1, codec::MAX_SIDE), h.clamp(1, codec::MAX_SIDE)));
    }

    pub fn skew(&mut self, deg_x: f32, deg_y: f32) {
        self.ed.transform(Transform::Skew(deg_x, deg_y));
    }

    pub fn resize_canvas(&mut self, w: u32, h: u32) {
        self.ed.resize_canvas(w, h);
    }

    /// Canvas resize placing the old pixels at (dx, dy) (left/top handles).
    pub fn resize_canvas_at(&mut self, w: u32, h: u32, dx: i32, dy: i32) {
        self.ed.resize_canvas_at(w, h, dx, dy);
    }

    pub fn remove_background(&mut self, tolerance: u8) -> bool {
        self.ed.remove_background(tolerance)
    }

    // ----- clipboard -------------------------------------------------------

    /// Selected pixels as PNG, or `undefined` without a selection.
    pub fn copy_png(&self) -> Option<Vec<u8>> {
        self.ed.copy().and_then(|img| codec::encode(&img, Format::Png, 100).ok())
    }

    pub fn cut_png(&mut self) -> Option<Vec<u8>> {
        self.ed.cut().and_then(|img| codec::encode(&img, Format::Png, 100).ok())
    }

    /// Pastes encoded image bytes (PNG, JPEG, ...) as a floating selection.
    pub fn paste_bytes(&mut self, bytes: &[u8], x: i32, y: i32) -> Result<(), JsValue> {
        let img = codec::decode(bytes).map_err(err)?;
        self.ed.paste(img, x, y);
        Ok(())
    }

    /// Pastes raw RGBA pixels as a floating selection.
    pub fn paste_rgba(&mut self, w: u32, h: u32, data: Vec<u8>, x: i32, y: i32) -> Result<(), JsValue> {
        let img = Image::from_rgba(w, h, data).map_err(err)?;
        self.ed.paste(img, x, y);
        Ok(())
    }

    /// Blends raw RGBA pixels (rendered text) onto the active layer.
    pub fn stamp_rgba(&mut self, x: i32, y: i32, w: u32, h: u32, data: Vec<u8>) -> Result<(), JsValue> {
        let img = Image::from_rgba(w, h, data).map_err(err)?;
        self.ed.stamp_image(x, y, &img);
        Ok(())
    }

    // ----- files -----------------------------------------------------------

    pub fn new_document(&mut self, w: u32, h: u32) {
        self.ed.new_document(w, h);
    }

    pub fn open_bytes(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        self.ed.open_bytes(bytes).map_err(err)
    }

    /// Encodes the flattened picture. `format` is an extension such as "png" or "jpg".
    pub fn export(&self, format: &str, quality: u8) -> Result<Vec<u8>, JsValue> {
        let fmt = Format::from_name(format).ok_or_else(|| err(format!("unknown format {format}")))?;
        self.ed.export(fmt, quality).map_err(err)
    }

    pub fn is_modified(&self) -> bool {
        self.ed.is_modified()
    }

    pub fn mark_saved(&mut self) {
        self.ed.mark_saved();
    }

    // ----- layers ----------------------------------------------------------

    pub fn add_layer(&mut self) {
        self.ed.add_layer();
    }

    pub fn delete_layer(&mut self, i: usize) -> bool {
        self.ed.delete_layer(i)
    }

    pub fn duplicate_layer(&mut self, i: usize) -> bool {
        self.ed.duplicate_layer(i)
    }

    pub fn merge_down(&mut self, i: usize) -> bool {
        self.ed.merge_down(i)
    }

    pub fn move_layer(&mut self, from: usize, to: usize) -> bool {
        self.ed.move_layer(from, to)
    }

    pub fn set_active_layer(&mut self, i: usize) -> bool {
        self.ed.set_active_layer(i)
    }

    pub fn set_layer_visible(&mut self, i: usize, visible: bool) -> bool {
        self.ed.set_layer_visible(i, visible)
    }

    pub fn rename_layer(&mut self, i: usize, name: &str) -> bool {
        self.ed.rename_layer(i, name)
    }

    /// Small PNG preview of a layer for the layers panel.
    pub fn layer_thumbnail_png(&self, i: usize, max: u32) -> Option<Vec<u8>> {
        self.ed.layer_thumbnail(i, max).and_then(|img| codec::encode(&img, Format::Png, 100).ok())
    }

    // ----- inspection (status bar, tests) ------------------------------------

    /// Visible colour at a pixel as `#rrggbb[aa]`.
    pub fn pixel(&self, x: i32, y: i32) -> String {
        self.ed.composite_pixel(x, y).to_hex()
    }

    /// Colour of the active layer at a pixel.
    pub fn layer_pixel(&self, x: i32, y: i32) -> String {
        self.ed.layer_pixel(x, y).to_hex()
    }
}
