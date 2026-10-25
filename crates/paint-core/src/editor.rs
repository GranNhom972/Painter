//! The interactive editor: current tool and colours, pointer handling, commands,
//! clipboard, layers and a serialisable UI state. The UI only forwards events here.

use crate::brush::{BrushKind, Stroke};
use crate::codec::{self, Format};
use crate::color::{blend_over, Rgba};
use crate::document::Document;
use crate::fill;
use crate::geom::{union_opt, IRect, Pt};
use crate::history::{History, State};
use crate::image::{DrawOptions, Image, Mask, Transform};
use crate::raster::dist_to_segment;
use crate::selection::Selection;
use crate::shapes::{self, FillStyle, OutlineStyle, ShapeGeom, ShapeKind};
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    RectSelect,
    FreeSelect,
    Pencil,
    Fill,
    Text,
    Eraser,
    Picker,
    Magnifier,
    Brush,
    Shape,
}

impl Tool {
    pub const ALL: [Tool; 10] =
        [Tool::RectSelect, Tool::FreeSelect, Tool::Pencil, Tool::Fill, Tool::Text, Tool::Eraser, Tool::Picker, Tool::Magnifier, Tool::Brush, Tool::Shape];

    pub fn name(self) -> &'static str {
        match self {
            Tool::RectSelect => "select-rect",
            Tool::FreeSelect => "select-free",
            Tool::Pencil => "pencil",
            Tool::Fill => "fill",
            Tool::Text => "text",
            Tool::Eraser => "eraser",
            Tool::Picker => "picker",
            Tool::Magnifier => "magnifier",
            Tool::Brush => "brush",
            Tool::Shape => "shape",
        }
    }

    pub fn from_name(s: &str) -> Option<Tool> {
        Tool::ALL.iter().copied().find(|t| t.name() == s)
    }

    pub fn is_select(self) -> bool {
        matches!(self, Tool::RectSelect | Tool::FreeSelect)
    }

    /// Each tool family remembers its own size, like Paint.
    fn size_slot(self) -> usize {
        match self {
            Tool::Pencil => 0,
            Tool::Eraser => 2,
            Tool::Shape => 3,
            _ => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// Resize handles of a selection or shape bounding box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    NW,
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
}

impl Handle {
    pub const ALL: [Handle; 8] = [Handle::NW, Handle::N, Handle::NE, Handle::E, Handle::SE, Handle::S, Handle::SW, Handle::W];

    pub fn cursor(self) -> &'static str {
        match self {
            Handle::N | Handle::S => "ns-resize",
            Handle::E | Handle::W => "ew-resize",
            Handle::NW | Handle::SE => "nwse-resize",
            Handle::NE | Handle::SW => "nesw-resize",
        }
    }

    pub fn position(self, lo: Pt, hi: Pt) -> Pt {
        let mx = (lo.x + hi.x) / 2.0;
        let my = (lo.y + hi.y) / 2.0;
        match self {
            Handle::NW => lo,
            Handle::N => Pt::new(mx, lo.y),
            Handle::NE => Pt::new(hi.x, lo.y),
            Handle::E => Pt::new(hi.x, my),
            Handle::SE => hi,
            Handle::S => Pt::new(mx, hi.y),
            Handle::SW => Pt::new(lo.x, hi.y),
            Handle::W => Pt::new(lo.x, my),
        }
    }

    /// Handle under `p`. Inside small boxes the grab radius shrinks so the
    /// middle of the box stays available for moving.
    pub fn hit(lo: Pt, hi: Pt, p: Pt, tol: f32) -> Option<Handle> {
        let inside = p.x > lo.x && p.x < hi.x && p.y > lo.y && p.y < hi.y;
        let limit = if inside { tol.min((hi.x - lo.x).min(hi.y - lo.y) / 4.0) } else { tol };
        Handle::ALL
            .into_iter()
            .map(|h| (h, h.position(lo, hi).dist(p)))
            .filter(|(_, d)| *d <= limit)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(h, _)| h)
    }

    fn moves_left(self) -> bool {
        matches!(self, Handle::NW | Handle::W | Handle::SW)
    }
    fn moves_right(self) -> bool {
        matches!(self, Handle::NE | Handle::E | Handle::SE)
    }
    fn moves_top(self) -> bool {
        matches!(self, Handle::NW | Handle::N | Handle::NE)
    }
    fn moves_bottom(self) -> bool {
        matches!(self, Handle::SW | Handle::S | Handle::SE)
    }

    /// New (unnormalised) box corners after dragging this handle to `p`.
    /// With `keep_aspect`, corner handles preserve the original proportions.
    pub fn drag(self, lo: Pt, hi: Pt, p: Pt, keep_aspect: bool) -> (Pt, Pt) {
        let (mut l, mut t, mut r, mut b) = (lo.x, lo.y, hi.x, hi.y);
        if self.moves_left() {
            l = p.x;
        }
        if self.moves_right() {
            r = p.x;
        }
        if self.moves_top() {
            t = p.y;
        }
        if self.moves_bottom() {
            b = p.y;
        }
        let corner = (self.moves_left() || self.moves_right()) && (self.moves_top() || self.moves_bottom());
        let (w0, h0) = (hi.x - lo.x, hi.y - lo.y);
        if keep_aspect && corner && w0 > 0.0 && h0 > 0.0 {
            let (w, h) = (r - l, b - t);
            let k = (w.abs() / w0).max(h.abs() / h0);
            let (nw, nh) = (w.signum() * k * w0, h.signum() * k * h0);
            if self.moves_left() {
                l = r - nw;
            } else {
                r = l + nw;
            }
            if self.moves_top() {
                t = b - nh;
            } else {
                b = t + nh;
            }
        }
        (Pt::new(l, t), Pt::new(r, b))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShapeStage {
    /// Finished drawing; handles allow moving and resizing until committed.
    Editing,
    /// Curve waiting for its first bend.
    Bend1,
    /// Curve waiting for its second bend.
    Bend2,
    /// Polygon still receiving points.
    PolygonOpen,
}

impl ShapeStage {
    fn name(self) -> &'static str {
        match self {
            ShapeStage::Editing => "editing",
            ShapeStage::Bend1 => "bend1",
            ShapeStage::Bend2 => "bend2",
            ShapeStage::PolygonOpen => "polygon",
        }
    }
}

#[derive(Clone, Debug)]
struct PendingShape {
    kind: ShapeKind,
    geom: ShapeGeom,
    stage: ShapeStage,
    /// Layer pixels before the shape was drawn.
    base: Arc<Image>,
    /// Drawn with the right button: colours swapped.
    swapped: bool,
    /// Area currently covered by the shape preview.
    drawn: IRect,
    /// Unsaved-changes flag before the shape started (restored if it is cancelled).
    was_modified: bool,
}

#[derive(Clone, Copy, Debug)]
enum StrokeMode {
    Paint(Rgba, f32),
    /// Erase to transparency.
    Erase,
    /// Replace one colour by another (eraser with the right button).
    Replace(Rgba, Rgba),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShapeGrab {
    Move,
    Box(Handle),
    Point(usize),
}

enum Drag {
    Stroke { stroke: Stroke, mask: Mask, base: Arc<Image>, mode: StrokeMode },
    NewSelection { start: Pt, cur: Pt },
    Lasso { points: Vec<Pt> },
    MoveSelection { grab: Pt, orig: IRect, was_modified: bool },
    /// `flip`: the handle went past the opposite edge (the content is mirrored).
    ResizeSelection { handle: Handle, orig: IRect, flip: (bool, bool), was_modified: bool },
    NewShape { start: Pt },
    EditShape { grab: ShapeGrab, start: Pt, orig: ShapeGeom },
    CurveControl { second: bool },
    PolygonPoint,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct SelectionInfo {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub floating: bool,
    pub freeform: bool,
    /// Still being drawn (rubber band or lasso).
    pub drawing: bool,
    pub outline: Vec<Vec<[f32; 2]>>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct HandleInfo {
    pub x: f32,
    pub y: f32,
    pub cursor: &'static str,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct ShapeInfo {
    pub kind: &'static str,
    pub stage: &'static str,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub handles: Vec<HandleInfo>,
    pub points: Vec<[f32; 2]>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct LayerInfo {
    pub id: u32,
    pub name: String,
    pub visible: bool,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct EditorState {
    pub width: u32,
    pub height: u32,
    pub tool: &'static str,
    pub brush: &'static str,
    pub shape: &'static str,
    pub color1: String,
    pub color2: String,
    pub size: u32,
    pub outline: &'static str,
    pub fill: &'static str,
    pub transparent_selection: bool,
    pub fill_tolerance: u8,
    pub can_undo: bool,
    pub can_redo: bool,
    pub modified: bool,
    pub dragging: bool,
    pub selection: Option<SelectionInfo>,
    pub shape_edit: Option<ShapeInfo>,
    pub layers: Vec<LayerInfo>,
    pub active_layer: usize,
}

/// Screen pixels within which a handle is grabbed (divided by the zoom).
const HANDLE_RADIUS: f32 = 6.0;

pub struct Editor {
    doc: Document,
    history: History,
    selection: Option<Selection>,
    pending: Option<PendingShape>,
    drag: Option<Drag>,
    frame: Image,
    dirty: Option<IRect>,
    tool: Tool,
    previous_tool: Tool,
    brush: BrushKind,
    shape: ShapeKind,
    color1: Rgba,
    color2: Rgba,
    sizes: [u32; 4],
    outline: OutlineStyle,
    fill: FillStyle,
    transparent_selection: bool,
    fill_tolerance: u8,
    zoom: f32,
    modified: bool,
    seed: u32,
    last_pointer: Pt,
}

/// Pixel containing the pointer (selection corners: the end corner is exclusive, as in Paint).
fn snap(p: Pt) -> Pt {
    Pt::new(p.x.floor(), p.y.floor())
}

fn apply_stroke(layer: &mut Image, base: &Image, mask: &Mask, rect: IRect, mode: StrokeMode) {
    for y in rect.y..rect.bottom() {
        for x in rect.x..rect.right() {
            let cov = mask.get(x, y);
            let b = base.get_raw(x, y);
            let out = match mode {
                StrokeMode::Paint(c, opacity) => {
                    if cov == 0 {
                        b
                    } else {
                        blend_over(b, c.scale_alpha((cov as f32 * opacity) as u8).to_array())
                    }
                }
                StrokeMode::Erase => {
                    let a = (b[3] as u32 * (255 - cov as u32) / 255) as u8;
                    if a == 0 {
                        [0, 0, 0, 0]
                    } else {
                        [b[0], b[1], b[2], a]
                    }
                }
                StrokeMode::Replace(from, to) => {
                    if cov >= 128 && Rgba::from_array(b).distance(from) <= 16 {
                        to.to_array()
                    } else {
                        b
                    }
                }
            };
            layer.set_raw(x, y, out);
        }
    }
}

impl Editor {
    pub fn new(width: u32, height: u32) -> Self {
        Editor::with_document(Document::new(width, height, Rgba::WHITE))
    }

    pub fn from_image(img: Image) -> Self {
        Editor::with_document(Document::from_image(img))
    }

    fn with_document(doc: Document) -> Self {
        let frame = Image::new(doc.width(), doc.height());
        let dirty = Some(doc.bounds());
        Editor {
            doc,
            history: History::default(),
            selection: None,
            pending: None,
            drag: None,
            frame,
            dirty,
            tool: Tool::Pencil,
            previous_tool: Tool::Pencil,
            brush: BrushKind::Brush,
            shape: ShapeKind::Rectangle,
            color1: Rgba::BLACK,
            color2: Rgba::WHITE,
            sizes: [1, 5, 10, 3],
            outline: OutlineStyle::Solid,
            fill: FillStyle::None,
            transparent_selection: false,
            fill_tolerance: 0,
            zoom: 1.0,
            modified: false,
            seed: 0,
            last_pointer: Pt::default(),
        }
    }

    // ----- accessors -------------------------------------------------------

    pub fn doc(&self) -> &Document {
        &self.doc
    }
    pub fn width(&self) -> u32 {
        self.doc.width()
    }
    pub fn height(&self) -> u32 {
        self.doc.height()
    }
    pub fn selection(&self) -> Option<&Selection> {
        self.selection.as_ref()
    }
    pub fn tool(&self) -> Tool {
        self.tool
    }
    pub fn color1(&self) -> Rgba {
        self.color1
    }
    pub fn color2(&self) -> Rgba {
        self.color2
    }
    pub fn size(&self) -> u32 {
        self.sizes[self.tool.size_slot()]
    }
    pub fn is_modified(&self) -> bool {
        self.modified
    }
    pub fn mark_saved(&mut self) {
        self.modified = false;
    }
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }
    pub fn has_pending_shape(&self) -> bool {
        self.pending.is_some()
    }
    /// The composited frame (valid after [`Editor::render`]).
    pub fn frame(&self) -> &Image {
        &self.frame
    }

    /// Pixel of the active layer.
    pub fn layer_pixel(&self, x: i32, y: i32) -> Rgba {
        self.doc.active_image().get(x, y)
    }

    /// Visible colour at (x, y): all visible layers plus a floating selection.
    pub fn composite_pixel(&self, x: i32, y: i32) -> Rgba {
        if !self.doc.bounds().contains(x, y) {
            return Rgba::TRANSPARENT;
        }
        let mut acc = [0u8; 4];
        for (i, l) in self.doc.layers.iter().enumerate() {
            if l.visible {
                acc = blend_over(acc, l.image.get_raw(x, y));
            }
            if i == self.doc.active && l.visible {
                if let Some(sel) = self.selection.as_ref().filter(|s| s.is_floating()) {
                    if sel.rect.contains(x, y) && sel.current_mask().is_none_or(|m| m.get(x - sel.rect.x, y - sel.rect.y) >= 128) {
                        let p = sel.current_image().unwrap().get(x - sel.rect.x, y - sel.rect.y);
                        if self.key() != Some(p) {
                            acc = blend_over(acc, p.to_array());
                        }
                    }
                }
            }
        }
        Rgba::from_array(acc)
    }

    // ----- rendering -------------------------------------------------------

    fn mark(&mut self, r: IRect) {
        self.dirty = union_opt(self.dirty, r.clamp_to(self.doc.width(), self.doc.height()));
    }

    fn mark_all(&mut self) {
        self.dirty = Some(self.doc.bounds());
    }

    /// Forces a full redraw on the next [`Editor::render`].
    pub fn invalidate(&mut self) {
        self.mark_all();
    }

    fn sync_frame(&mut self) {
        if self.frame.width() != self.doc.width() || self.frame.height() != self.doc.height() {
            self.frame = Image::new(self.doc.width(), self.doc.height());
        }
        self.mark_all();
    }

    fn key(&self) -> Option<Rgba> {
        self.transparent_selection.then_some(self.color2)
    }

    /// Updates the frame for everything that changed; returns the updated rectangle.
    pub fn render(&mut self) -> Option<IRect> {
        let r = self.dirty.take()?.clamp_to(self.doc.width(), self.doc.height());
        if r.is_empty() {
            return None;
        }
        let key = self.key();
        let floating = self.selection.as_ref().filter(|s| s.is_floating()).map(|s| (s, key));
        self.doc.composite_into(&mut self.frame, r, floating);
        Some(r)
    }

    /// The whole picture as it would be saved.
    pub fn flatten(&self) -> Image {
        let key = self.key();
        self.doc.flatten(self.selection.as_ref().filter(|s| s.is_floating()).map(|s| (s, key)))
    }

    // ----- history ---------------------------------------------------------

    fn snapshot(&self) -> State {
        State { doc: self.doc.clone(), selection: self.selection.clone() }
    }

    /// Records the current state before a modification.
    fn record(&mut self) {
        let s = self.snapshot();
        self.history.push(s);
        self.modified = true;
    }

    fn restore(&mut self, s: State) {
        self.doc = s.doc;
        self.selection = s.selection;
        self.pending = None;
        self.drag = None;
        self.sync_frame();
    }

    pub fn undo(&mut self) -> bool {
        self.finish_drag();
        self.pending = None;
        let cur = self.snapshot();
        match self.history.undo(cur) {
            Some(prev) => {
                self.restore(prev);
                self.modified = true;
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        self.finish_drag();
        self.pending = None;
        let cur = self.snapshot();
        match self.history.redo(cur) {
            Some(next) => {
                self.restore(next);
                self.modified = true;
                true
            }
            None => false,
        }
    }

    // ----- committing transient state ----------------------------------------

    fn finish_drag(&mut self) {
        if self.drag.is_some() {
            let p = self.last_pointer;
            self.pointer_up(p.x, p.y);
        }
    }

    fn commit_shape(&mut self) {
        if self.pending.as_ref().is_some_and(|p| p.stage == ShapeStage::PolygonOpen) {
            self.close_polygon();
        }
        if self.pending.take().is_some() {
            self.modified = true;
        }
    }

    fn commit_selection(&mut self) {
        if let Some(sel) = self.selection.take() {
            if sel.is_floating() {
                let key = self.key();
                sel.stamp(self.doc.active_image_mut(), key);
                self.mark(sel.rect);
                self.modified = true;
            }
        }
    }

    /// Finishes any drag, shape or floating selection.
    pub fn commit_all(&mut self) {
        self.finish_drag();
        self.commit_shape();
        self.commit_selection();
    }

    // ----- tool settings ---------------------------------------------------

    pub fn set_tool(&mut self, tool: Tool) {
        if tool == self.tool {
            return;
        }
        // Keep a selection when switching between the two selection tools.
        if tool.is_select() && self.tool.is_select() {
            self.finish_drag();
        } else {
            self.commit_all();
        }
        if tool == Tool::Picker {
            self.previous_tool = self.tool;
        }
        self.tool = tool;
    }

    pub fn set_brush(&mut self, b: BrushKind) {
        self.brush = b;
        self.set_tool(Tool::Brush);
    }

    pub fn brush(&self) -> BrushKind {
        self.brush
    }

    pub fn set_shape(&mut self, s: ShapeKind) {
        self.commit_shape();
        self.shape = s;
        self.set_tool(Tool::Shape);
    }

    pub fn shape(&self) -> ShapeKind {
        self.shape
    }

    pub fn set_color1(&mut self, c: Rgba) {
        self.color1 = c;
        self.render_pending();
    }

    pub fn set_color2(&mut self, c: Rgba) {
        self.color2 = c;
        self.render_pending();
        if self.transparent_selection {
            if let Some(r) = self.selection.as_ref().map(|s| s.rect) {
                self.mark(r);
            }
        }
    }

    pub fn swap_colors(&mut self) {
        let (a, b) = (self.color1, self.color2);
        self.set_color1(b);
        self.set_color2(a);
    }

    pub fn set_size(&mut self, size: u32) {
        self.sizes[self.tool.size_slot()] = size.clamp(1, 100);
        if self.tool == Tool::Shape {
            self.render_pending();
        }
    }

    pub fn set_outline(&mut self, o: OutlineStyle) {
        self.outline = o;
        self.render_pending();
    }

    pub fn set_fill(&mut self, f: FillStyle) {
        self.fill = f;
        self.render_pending();
    }

    pub fn set_transparent_selection(&mut self, on: bool) {
        if self.transparent_selection != on && self.selection.as_ref().is_some_and(|s| s.is_floating()) {
            self.modified = true;
        }
        self.transparent_selection = on;
        if let Some(r) = self.selection.as_ref().map(|s| s.rect) {
            self.mark(r);
        }
    }

    pub fn set_fill_tolerance(&mut self, t: u8) {
        self.fill_tolerance = t;
    }

    pub fn set_zoom(&mut self, z: f32) {
        self.zoom = z.clamp(0.01, 100.0);
    }

    fn handle_tolerance(&self) -> f32 {
        HANDLE_RADIUS / self.zoom
    }

    fn button_color(&self, b: Button) -> Rgba {
        match b {
            Button::Left => self.color1,
            Button::Right => self.color2,
        }
    }

    // ----- pointer events --------------------------------------------------

    pub fn pointer_down(&mut self, x: f32, y: f32, button: Button, mods: Mods) {
        let p = Pt::new(x, y);
        self.last_pointer = p;
        if self.drag.is_some() {
            return;
        }
        match self.tool {
            Tool::Pencil | Tool::Brush | Tool::Eraser => self.begin_stroke(p, button),
            Tool::Fill => self.bucket(p, button),
            Tool::Picker => self.pick(p, button),
            Tool::RectSelect | Tool::FreeSelect => self.select_down(p, button, mods),
            Tool::Shape => self.shape_down(p, button),
            Tool::Text | Tool::Magnifier => self.commit_all(),
        }
    }

    pub fn pointer_move(&mut self, x: f32, y: f32, mods: Mods) {
        let p = Pt::new(x, y);
        self.last_pointer = p;
        let Some(drag) = self.drag.as_mut() else { return };
        match drag {
            Drag::Stroke { .. } => self.continue_stroke(p),
            Drag::NewSelection { start, cur } => {
                let mut c = snap(p);
                if mods.shift {
                    c = shapes::constrain(ShapeKind::Rectangle, *start, c);
                }
                *cur = c;
            }
            Drag::Lasso { points } => {
                if points.last().is_none_or(|l| l.dist(p) >= 0.5) {
                    points.push(p);
                }
            }
            Drag::MoveSelection { grab, orig, .. } => {
                let (dx, dy) = ((p.x - grab.x).round() as i32, (p.y - grab.y).round() as i32);
                let (ox, oy) = (orig.x, orig.y);
                if let Some(sel) = self.selection.as_mut() {
                    let old = sel.rect;
                    sel.move_to(ox + dx, oy + dy);
                    let new = sel.rect;
                    self.mark(old.union(&new));
                }
            }
            Drag::ResizeSelection { handle, orig, flip, .. } => {
                let (h, o) = (*handle, *orig);
                let lo = Pt::new(o.x as f32, o.y as f32);
                let hi = Pt::new(o.right() as f32, o.bottom() as f32);
                let (a, b) = h.drag(lo, hi, snap(p), mods.shift);
                let mut r = IRect::from_corners(a.x.round() as i32, a.y.round() as i32, b.x.round() as i32, b.y.round() as i32);
                r.w = r.w.max(1);
                r.h = r.h.max(1);
                // Refuse sizes the memory budget cannot hold (far drags at low zoom).
                if codec::size_ok(r.w as u32, r.h as u32) {
                    *flip = (b.x < a.x, b.y < a.y);
                    let f = *flip;
                    if let Some(sel) = self.selection.as_mut() {
                        let old = sel.rect;
                        sel.resize_preview(r, f.0, f.1);
                        self.mark(old.union(&r));
                    }
                }
            }
            Drag::NewShape { start } => {
                let start = *start;
                if let Some(ps) = self.pending.as_mut() {
                    let q = if mods.shift { shapes::constrain(ps.kind, start, p) } else { p };
                    match &mut ps.geom {
                        ShapeGeom::Box(_, b) | ShapeGeom::Line(_, b) => *b = q,
                        ShapeGeom::Curve(pts) => {
                            pts[2] = q;
                            pts[3] = q;
                        }
                        ShapeGeom::Polygon { points, .. } => *points.last_mut().unwrap() = q,
                    }
                }
                self.render_pending();
            }
            Drag::EditShape { grab, start, orig } => {
                let (grab, start) = (*grab, *start);
                let mut g = orig.clone();
                match grab {
                    ShapeGrab::Move => g.translate(p - start),
                    ShapeGrab::Point(i) => match &mut g {
                        ShapeGeom::Line(a, b) => *(if i == 0 { a } else { b }) = p,
                        ShapeGeom::Curve(pts) => pts[i] = p,
                        _ => {}
                    },
                    ShapeGrab::Box(h) => {
                        let (lo, hi) = orig.bounds();
                        let to = h.drag(lo, hi, p, mods.shift);
                        g.remap((lo, hi), to);
                    }
                }
                if let Some(ps) = self.pending.as_mut() {
                    ps.geom = g;
                }
                self.render_pending();
            }
            Drag::CurveControl { second } => {
                let second = *second;
                self.set_curve_control(p, second);
            }
            Drag::PolygonPoint => {
                if let Some(ShapeGeom::Polygon { points, .. }) = self.pending.as_mut().map(|ps| &mut ps.geom) {
                    let n = points.len();
                    let q = if mods.shift && n >= 2 { shapes::constrain(ShapeKind::Line, points[n - 2], p) } else { p };
                    points[n - 1] = q;
                }
                self.render_pending();
            }
        }
    }

    pub fn pointer_up(&mut self, x: f32, y: f32) {
        let p = Pt::new(x, y);
        self.last_pointer = p;
        let Some(drag) = self.drag.take() else { return };
        let (w, h) = (self.doc.width(), self.doc.height());
        match drag {
            Drag::NewSelection { start, cur } => {
                let r = IRect::from_corners(start.x as i32, start.y as i32, cur.x as i32, cur.y as i32).clamp_to(w, h);
                if !r.is_empty() {
                    self.selection = Some(Selection::rect(r));
                }
            }
            Drag::Lasso { points } => {
                self.selection = Selection::freeform(&points, w, h);
            }
            Drag::NewShape { start } => {
                let Some(ps) = self.pending.as_mut() else { return };
                let end = match &ps.geom {
                    ShapeGeom::Box(_, b) | ShapeGeom::Line(_, b) => *b,
                    ShapeGeom::Curve(pts) => pts[3],
                    ShapeGeom::Polygon { points, .. } => *points.last().unwrap(),
                };
                if end.dist(start) < 1.0 {
                    self.cancel_shape();
                    return;
                }
                ps.stage = match ps.kind {
                    ShapeKind::Curve => ShapeStage::Bend1,
                    ShapeKind::Polygon => ShapeStage::PolygonOpen,
                    _ => ShapeStage::Editing,
                };
            }
            Drag::CurveControl { second } => {
                if let Some(ps) = self.pending.as_mut() {
                    ps.stage = if second { ShapeStage::Editing } else { ShapeStage::Bend2 };
                }
            }
            Drag::ResizeSelection { orig, flip, was_modified, .. } => {
                if self.selection.as_ref().is_some_and(|s| s.rect == orig) && flip == (false, false) {
                    // A click on a handle without dragging: no undo step, nothing lifted.
                    self.unrecord(was_modified);
                } else if let Some(sel) = self.selection.as_mut() {
                    // Replace the fast preview by the smooth (and mirrored) result.
                    sel.finish_resize(flip.0, flip.1);
                    let r = sel.rect;
                    self.mark(r);
                }
            }
            Drag::MoveSelection { orig, was_modified, .. } => {
                if self.selection.as_ref().is_some_and(|s| s.rect == orig) {
                    self.unrecord(was_modified);
                }
            }
            Drag::Stroke { .. } | Drag::EditShape { .. } | Drag::PolygonPoint => {}
        }
    }

    /// Double click: closes a polygon being drawn.
    pub fn double_click(&mut self, _x: f32, _y: f32) {
        self.finish_drag();
        let zoom = self.zoom;
        if let Some(ps) = self.pending.as_mut() {
            if ps.stage == ShapeStage::PolygonOpen {
                if let ShapeGeom::Polygon { points, .. } = &mut ps.geom {
                    // The second click of the double click added a duplicate point.
                    let near = (2.0 / zoom).max(1.0);
                    while points.len() > 3 && points[points.len() - 1].dist(points[points.len() - 2]) < near {
                        points.pop();
                    }
                }
                self.close_polygon();
            }
        }
    }

    /// Timer tick while the pointer is held (airbrush keeps spraying).
    pub fn tick(&mut self) {
        if let Some(Drag::Stroke { stroke, mask, base, mode }) = &mut self.drag {
            let r = stroke.tick(mask);
            if !r.is_empty() {
                let layer = Arc::make_mut(&mut self.doc.layers[self.doc.active].image);
                apply_stroke(layer, base, mask, r, *mode);
                self.dirty = union_opt(self.dirty, r);
            }
        }
    }

    /// CSS cursor (or tool cursor name) to show at a position.
    pub fn cursor_at(&self, x: f32, y: f32) -> &'static str {
        let p = Pt::new(x, y);
        match self.tool {
            Tool::RectSelect | Tool::FreeSelect => match &self.drag {
                Some(Drag::MoveSelection { .. }) => "move",
                Some(Drag::ResizeSelection { handle, .. }) => handle.cursor(),
                Some(_) => "crosshair",
                None => {
                    if let Some(h) = self.selection_handle_at(p) {
                        h.cursor()
                    } else if self.selection.as_ref().is_some_and(|s| s.rect.contains_pt(p)) {
                        "move"
                    } else {
                        "crosshair"
                    }
                }
            },
            Tool::Shape => match self.shape_grab_at(p) {
                Some(ShapeGrab::Move) => "move",
                Some(ShapeGrab::Box(h)) => h.cursor(),
                Some(ShapeGrab::Point(_)) => "pointer",
                None => "crosshair",
            },
            Tool::Pencil => "pencil",
            Tool::Brush => "brush",
            Tool::Eraser => "eraser",
            Tool::Fill => "fill",
            Tool::Picker => "picker",
            Tool::Text => "text",
            Tool::Magnifier => "zoom-in",
        }
    }

    // ----- painting tools --------------------------------------------------

    fn begin_stroke(&mut self, p: Pt, button: Button) {
        self.commit_all();
        self.record();
        let (kind, mode) = match self.tool {
            Tool::Pencil => (BrushKind::Pencil, StrokeMode::Paint(self.button_color(button), 1.0)),
            Tool::Brush => (self.brush, StrokeMode::Paint(self.button_color(button), self.brush.opacity())),
            _ => (
                BrushKind::Eraser,
                match button {
                    Button::Right => StrokeMode::Replace(self.color1, self.color2),
                    Button::Left if self.doc.active == 0 => StrokeMode::Paint(self.color2, 1.0),
                    Button::Left => StrokeMode::Erase,
                },
            ),
        };
        self.seed = self.seed.wrapping_add(1);
        let stroke = Stroke::new(kind, self.size(), self.seed.wrapping_mul(2_654_435_761));
        let mask = Mask::new(self.doc.width(), self.doc.height());
        let base = self.doc.active_arc();
        self.drag = Some(Drag::Stroke { stroke, mask, base, mode });
        self.continue_stroke(p);
    }

    fn continue_stroke(&mut self, p: Pt) {
        if let Some(Drag::Stroke { stroke, mask, base, mode }) = &mut self.drag {
            let r = stroke.to(mask, p);
            let layer = Arc::make_mut(&mut self.doc.layers[self.doc.active].image);
            apply_stroke(layer, base, mask, r, *mode);
            self.dirty = union_opt(self.dirty, r);
        }
    }

    fn bucket(&mut self, p: Pt, button: Button) {
        self.commit_all();
        let (x, y) = (p.x.floor() as i32, p.y.floor() as i32);
        let color = self.button_color(button);
        let img = self.doc.active_image();
        if !img.in_bounds(x, y) || (img.get(x, y) == color && self.fill_tolerance == 0) {
            return;
        }
        self.record();
        let tol = self.fill_tolerance;
        let r = fill::flood_fill(self.doc.active_image_mut(), x, y, color, tol);
        self.mark(r);
    }

    fn pick(&mut self, p: Pt, button: Button) {
        let (x, y) = (p.x.floor() as i32, p.y.floor() as i32);
        if !self.doc.bounds().contains(x, y) {
            return;
        }
        let c = self.composite_pixel(x, y).over(Rgba::WHITE);
        match button {
            Button::Left => self.set_color1(c),
            Button::Right => self.set_color2(c),
        }
        self.tool = if self.previous_tool == Tool::Picker { Tool::Pencil } else { self.previous_tool };
    }

    /// Stamps an RGBA image (rendered text) onto the active layer.
    pub fn stamp_image(&mut self, x: i32, y: i32, img: &Image) {
        self.commit_all();
        self.record();
        self.doc.active_image_mut().draw(img, x, y, &DrawOptions::default());
        self.mark(IRect::new(x, y, img.width() as i32, img.height() as i32));
    }

    // ----- selection -------------------------------------------------------

    fn selection_handle_at(&self, p: Pt) -> Option<Handle> {
        let s = self.selection.as_ref()?;
        let lo = Pt::new(s.rect.x as f32, s.rect.y as f32);
        let hi = Pt::new(s.rect.right() as f32, s.rect.bottom() as f32);
        Handle::hit(lo, hi, p, self.handle_tolerance())
    }

    fn lift(&mut self, keep_source: bool) {
        let hole = self.doc.hole_color(self.color2);
        let active = self.doc.active;
        if let Some(sel) = self.selection.as_mut() {
            if !sel.is_floating() {
                sel.lift(Arc::make_mut(&mut self.doc.layers[active].image), hole, keep_source);
                let r = sel.rect;
                self.mark(r);
            }
        }
    }

    fn select_down(&mut self, p: Pt, button: Button, mods: Mods) {
        if button == Button::Right {
            return; // context menu
        }
        if self.selection.is_some() {
            if let Some(handle) = self.selection_handle_at(p) {
                let was_modified = self.modified;
                self.record();
                self.lift(false);
                let orig = self.selection.as_ref().unwrap().rect;
                self.drag = Some(Drag::ResizeSelection { handle, orig, flip: (false, false), was_modified });
                return;
            }
            let (inside, floating, rect) = {
                let s = self.selection.as_ref().unwrap();
                (s.rect.contains_pt(p), s.is_floating(), s.rect)
            };
            if inside {
                let was_modified = self.modified;
                self.record();
                if !floating {
                    // Ctrl+drag moves a copy and leaves the original pixels in place.
                    self.lift(mods.ctrl);
                } else if mods.ctrl {
                    let key = self.key();
                    let active = self.doc.active;
                    if let Some(sel) = self.selection.as_ref() {
                        sel.stamp(Arc::make_mut(&mut self.doc.layers[active].image), key);
                    }
                    self.mark(rect);
                }
                self.drag = Some(Drag::MoveSelection { grab: p, orig: rect, was_modified });
                return;
            }
            self.commit_selection();
        }
        self.commit_shape();
        self.drag = Some(match self.tool {
            Tool::FreeSelect => Drag::Lasso { points: vec![p] },
            _ => {
                let s = snap(p);
                Drag::NewSelection { start: s, cur: s }
            }
        });
    }

    pub fn select_all(&mut self) {
        self.commit_all();
        if !self.tool.is_select() {
            self.tool = Tool::RectSelect;
        }
        self.selection = Some(Selection::rect(self.doc.bounds()));
    }

    /// Programmatic rectangular selection (clipped to the canvas).
    pub fn select_rect(&mut self, r: IRect) {
        self.commit_all();
        let r = r.clamp_to(self.doc.width(), self.doc.height());
        self.selection = (!r.is_empty()).then(|| Selection::rect(r));
    }

    pub fn invert_selection(&mut self) {
        self.finish_drag();
        self.commit_shape();
        let Some(sel) = self.selection.clone() else {
            self.select_all();
            return;
        };
        self.commit_selection();
        self.selection = sel.inverted(self.doc.width(), self.doc.height());
        if !self.tool.is_select() {
            self.tool = Tool::RectSelect;
        }
    }

    pub fn deselect(&mut self) {
        self.commit_all();
    }

    pub fn delete_selection(&mut self) -> bool {
        self.finish_drag();
        // Delete on a shape that is still editable removes it.
        if self.pending.is_some() {
            self.cancel_shape();
            return true;
        }
        if self.selection.is_none() {
            return false;
        }
        self.record();
        let sel = self.selection.take().unwrap();
        if !sel.is_floating() {
            let hole = self.doc.hole_color(self.color2);
            sel.clear_area(self.doc.active_image_mut(), hole);
        }
        self.mark(sel.rect);
        true
    }

    /// Crops the canvas to the selection.
    pub fn crop(&mut self) -> bool {
        self.finish_drag();
        self.commit_shape();
        let Some(sel) = self.selection.clone() else { return false };
        let rect = sel.rect.clamp_to(self.doc.width(), self.doc.height());
        if rect.is_empty() {
            return false;
        }
        self.record();
        if sel.is_floating() {
            let key = self.key();
            sel.stamp(self.doc.active_image_mut(), key);
        }
        let mask = sel.current_mask().map(|m| m.sub_mask(IRect::new(rect.x - sel.rect.x, rect.y - sel.rect.y, rect.w, rect.h)));
        self.doc.crop(rect, mask.as_ref(), self.color2);
        self.selection = None;
        self.sync_frame();
        true
    }

    /// Moves the selection by whole pixels (arrow keys).
    pub fn nudge(&mut self, dx: i32, dy: i32) -> bool {
        self.finish_drag();
        if let Some(ps) = self.pending.as_mut().filter(|ps| ps.stage == ShapeStage::Editing) {
            ps.geom.translate(Pt::new(dx as f32, dy as f32));
            self.render_pending();
            return true;
        }
        let Some(floating) = self.selection.as_ref().map(|s| s.is_floating()) else { return false };
        self.modified = true;
        if !floating {
            self.record();
            self.lift(false);
        }
        let sel = self.selection.as_mut().unwrap();
        let old = sel.rect;
        sel.move_to(old.x + dx, old.y + dy);
        let new = sel.rect;
        self.mark(old.union(&new));
        true
    }

    // ----- shapes ----------------------------------------------------------

    fn shape_grab_at(&self, p: Pt) -> Option<ShapeGrab> {
        let ps = self.pending.as_ref().filter(|ps| ps.stage == ShapeStage::Editing)?;
        let tol = self.handle_tolerance();
        match &ps.geom {
            ShapeGeom::Line(a, b) => {
                if p.dist(*a) <= tol {
                    Some(ShapeGrab::Point(0))
                } else if p.dist(*b) <= tol {
                    Some(ShapeGrab::Point(1))
                } else if dist_to_segment(p, *a, *b) <= tol + self.sizes[3] as f32 / 2.0 {
                    Some(ShapeGrab::Move)
                } else {
                    None
                }
            }
            ShapeGeom::Curve(pts) => {
                if let Some(i) = pts.iter().position(|q| q.dist(p) <= tol) {
                    return Some(ShapeGrab::Point(i));
                }
                let (lo, hi) = ps.geom.bounds();
                (p.x >= lo.x - tol && p.x <= hi.x + tol && p.y >= lo.y - tol && p.y <= hi.y + tol).then_some(ShapeGrab::Move)
            }
            _ => {
                let (lo, hi) = ps.geom.bounds();
                if let Some(h) = Handle::hit(lo, hi, p, tol) {
                    return Some(ShapeGrab::Box(h));
                }
                (p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y).then_some(ShapeGrab::Move)
            }
        }
    }

    fn shape_down(&mut self, p: Pt, button: Button) {
        if let Some(ps) = self.pending.as_mut() {
            match ps.stage {
                ShapeStage::Bend1 | ShapeStage::Bend2 => {
                    let second = ps.stage == ShapeStage::Bend2;
                    self.drag = Some(Drag::CurveControl { second });
                    self.set_curve_control(p, second);
                    return;
                }
                ShapeStage::PolygonOpen => {
                    let close = match &ps.geom {
                        ShapeGeom::Polygon { points, .. } => points.len() >= 3 && points[0].dist(p) <= HANDLE_RADIUS * 1.5 / self.zoom,
                        _ => false,
                    };
                    if close {
                        self.close_polygon();
                    } else {
                        if let ShapeGeom::Polygon { points, .. } = &mut ps.geom {
                            points.push(p);
                        }
                        self.drag = Some(Drag::PolygonPoint);
                        self.render_pending();
                    }
                    return;
                }
                ShapeStage::Editing => {
                    if let Some(grab) = self.shape_grab_at(p) {
                        let orig = self.pending.as_ref().unwrap().geom.clone();
                        self.drag = Some(Drag::EditShape { grab, start: p, orig });
                        return;
                    }
                    self.commit_shape();
                }
            }
        }
        self.commit_all();
        let was_modified = self.modified;
        self.record();
        let geom = match self.shape {
            ShapeKind::Line => ShapeGeom::Line(p, p),
            ShapeKind::Curve => ShapeGeom::Curve([p, p, p, p]),
            ShapeKind::Polygon => ShapeGeom::Polygon { points: vec![p, p], closed: false },
            _ => ShapeGeom::Box(p, p),
        };
        let base = self.doc.active_arc();
        self.pending = Some(PendingShape { kind: self.shape, geom, stage: ShapeStage::Editing, base, swapped: button == Button::Right, drawn: IRect::default(), was_modified });
        self.drag = Some(Drag::NewShape { start: p });
    }

    fn set_curve_control(&mut self, p: Pt, second: bool) {
        if let Some(ShapeGeom::Curve(pts)) = self.pending.as_mut().map(|ps| &mut ps.geom) {
            if second {
                pts[2] = p;
            } else {
                pts[1] = p;
                pts[2] = p;
            }
        }
        self.render_pending();
    }

    fn close_polygon(&mut self) {
        if let Some(ps) = self.pending.as_mut() {
            if let ShapeGeom::Polygon { points, closed } = &mut ps.geom {
                if points.len() >= 3 {
                    *closed = true;
                }
            }
            ps.stage = ShapeStage::Editing;
        }
        self.render_pending();
    }

    /// Removes a degenerate shape and forgets its history entry.
    fn cancel_shape(&mut self) {
        if let Some(ps) = self.pending.take() {
            let layer = self.doc.active_image_mut();
            let r = ps.drawn;
            for y in r.y..r.bottom() {
                for x in r.x..r.right() {
                    layer.set_raw(x, y, ps.base.get_raw(x, y));
                }
            }
            self.mark(r);
            self.history.discard_last();
            self.modified = ps.was_modified;
        }
    }

    /// Redraws the pending shape from the layer pixels underneath it.
    fn render_pending(&mut self) {
        let Some(ps) = self.pending.as_mut() else { return };
        // Editing a pending shape changes the picture (also after a save).
        self.modified = true;
        let (w, h) = (self.doc.width(), self.doc.height());
        let (line_c, fill_c) = if ps.swapped { (self.color2, self.color1) } else { (self.color1, self.color2) };
        let size = self.sizes[Tool::Shape.size_slot()];
        let cov = shapes::render(ps.kind, &ps.geom, self.outline, self.fill, size, w, h);
        let r = cov.rect;
        let layer = Arc::make_mut(&mut self.doc.layers[self.doc.active].image);
        let prev = ps.drawn;
        // Restore the previous preview area (row copies), then blend only covered pixels.
        let iw = w as usize;
        for area in [prev, r] {
            let n = area.w.max(0) as usize * 4;
            for y in area.y..area.bottom() {
                let i = (y as usize * iw + area.x as usize) * 4;
                layer.data_mut()[i..i + n].copy_from_slice(&ps.base.data()[i..i + n]);
            }
        }
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                let (f, l) = (cov.fill_at(x, y), cov.line_at(x, y));
                if f == 0 && l == 0 {
                    continue;
                }
                let mut px = ps.base.get_raw(x, y);
                if f > 0 {
                    px = blend_over(px, fill_c.scale_alpha(f).to_array());
                }
                if l > 0 {
                    px = blend_over(px, line_c.scale_alpha(l).to_array());
                }
                layer.set_raw(x, y, px);
            }
        }
        ps.drawn = r;
        self.dirty = union_opt(union_opt(self.dirty, prev), r);
    }

    // ----- image commands --------------------------------------------------

    /// Rotates/flips/resizes/skews the selection, or the whole image without one
    /// (or when everything is selected). Returns false when the result would be
    /// larger than the size limits ([`codec::size_ok`]); nothing changes then.
    pub fn transform(&mut self, t: Transform) -> bool {
        self.finish_drag();
        self.commit_shape();
        let whole = self.doc.bounds();
        if self.selection.as_ref().is_some_and(|s| !s.is_floating() && !s.is_freeform() && s.rect == whole) {
            // Select All + rotate turns the picture, like Paint.
            self.selection = None;
        }
        let (w, h) = match &self.selection {
            Some(s) => (s.rect.w.max(1) as u32, s.rect.h.max(1) as u32),
            None => (self.doc.width(), self.doc.height()),
        };
        let (ow, oh) = t.output_size(w, h);
        if ow > codec::MAX_SIDE as u64 || oh > codec::MAX_SIDE as u64 || !codec::size_ok(ow as u32, oh as u32) {
            return false;
        }
        self.record();
        if self.selection.is_some() {
            self.lift(false);
            let sel = self.selection.as_mut().unwrap();
            let old = sel.rect;
            sel.transform(t);
            let new = sel.rect;
            self.mark(old.union(&new));
        } else {
            self.doc.transform_all(t, self.color2);
            self.sync_frame();
        }
        true
    }

    /// Takes back the last history entry for an operation that changed nothing in
    /// the end (a click without drag): state, redo stack and unsaved flag.
    fn unrecord(&mut self, was_modified: bool) {
        if let Some(prev) = self.history.discard_last() {
            self.doc = prev.doc;
            self.selection = prev.selection;
            self.sync_frame();
        }
        self.modified = was_modified;
    }

    /// Resizes the canvas (no scaling), anchored top-left.
    pub fn resize_canvas(&mut self, w: u32, h: u32) -> bool {
        self.resize_canvas_at(w, h, 0, 0)
    }

    /// Resizes the canvas with the old pixels placed at (`dx`, `dy`): dragging the
    /// left or top canvas handles grows or shrinks the picture on that side.
    /// Returns false (nothing changes) when the size is beyond the limits.
    pub fn resize_canvas_at(&mut self, w: u32, h: u32, dx: i32, dy: i32) -> bool {
        self.commit_all();
        let (w, h) = (w.max(1), h.max(1));
        if !codec::size_ok(w, h) {
            return false;
        }
        if w == self.doc.width() && h == self.doc.height() && dx == 0 && dy == 0 {
            return true;
        }
        self.record();
        self.doc.resize_canvas_at(w, h, dx, dy, self.color2);
        self.sync_frame();
        true
    }

    /// Makes the background of the active layer (or of the selected area) transparent.
    pub fn remove_background(&mut self, tolerance: u8) -> bool {
        self.finish_drag();
        self.commit_shape();
        if let Some(settled) = self.selection.as_ref().filter(|s| s.is_floating()).map(|s| s.settled()) {
            // Drop the floating pixels in place and keep the same (possibly free-form) area.
            self.commit_selection();
            self.selection = Some(settled);
        }
        let area = self.selection.as_ref().map_or(self.doc.bounds(), |s| s.rect).clamp_to(self.doc.width(), self.doc.height());
        if area.is_empty() {
            self.selection = None;
            return false;
        }
        // A free-form selection limits the removal to the lasso.
        let allowed = self.selection.as_ref().filter(|s| s.is_freeform()).map(|s| {
            let mut m = Mask::new(self.doc.width(), self.doc.height());
            for y in area.y..area.bottom() {
                for x in area.x..area.right() {
                    if s.contains(x, y) {
                        m.set(x, y, 255);
                    }
                }
            }
            m
        });
        let was_modified = self.modified;
        self.record();
        let r = fill::remove_background(self.doc.active_image_mut(), area, tolerance, allowed.as_ref());
        if r.is_empty() {
            self.unrecord(was_modified);
            return false;
        }
        self.mark(r);
        true
    }

    // ----- clipboard -------------------------------------------------------

    /// Selected pixels of the active layer (free-form outside is transparent).
    pub fn copy(&self) -> Option<Image> {
        let sel = self.selection.as_ref()?;
        Some(sel.content(self.doc.active_image()))
    }

    pub fn cut(&mut self) -> Option<Image> {
        self.finish_drag();
        let img = self.copy()?;
        self.delete_selection();
        Some(img)
    }

    /// Pastes `img` as a floating selection at (x, y), growing the canvas if needed.
    /// Returns false (nothing pasted) when the result would exceed the size limits.
    pub fn paste(&mut self, img: Image, x: i32, y: i32) -> bool {
        self.commit_all();
        let (x, y) = (x.max(0), y.max(0));
        if img.width() == 0 || img.height() == 0 {
            return false;
        }
        let need_w = (x as u64 + img.width() as u64).max(self.doc.width() as u64);
        let need_h = (y as u64 + img.height() as u64).max(self.doc.height() as u64);
        if need_w > codec::MAX_SIDE as u64 || need_h > codec::MAX_SIDE as u64 || !codec::size_ok(need_w as u32, need_h as u32) {
            return false;
        }
        let (need_w, need_h) = (need_w as u32, need_h as u32);
        self.record();
        if need_w != self.doc.width() || need_h != self.doc.height() {
            self.doc.resize_canvas(need_w, need_h, self.color2);
            self.sync_frame();
        }
        let sel = Selection::pasted(img, x, y);
        self.mark(sel.rect);
        self.selection = Some(sel);
        if !self.tool.is_select() {
            self.tool = Tool::RectSelect;
        }
        true
    }

    // ----- layers ----------------------------------------------------------

    pub fn add_layer(&mut self) {
        self.commit_all();
        self.record();
        self.doc.add_layer();
        self.mark_all();
    }

    pub fn delete_layer(&mut self, i: usize) -> bool {
        self.commit_all();
        if self.doc.layers.len() <= 1 || i >= self.doc.layers.len() {
            return false;
        }
        self.record();
        self.doc.delete_layer(i);
        self.mark_all();
        true
    }

    pub fn duplicate_layer(&mut self, i: usize) -> bool {
        self.commit_all();
        if i >= self.doc.layers.len() {
            return false;
        }
        self.record();
        self.doc.duplicate_layer(i);
        self.mark_all();
        true
    }

    pub fn merge_down(&mut self, i: usize) -> bool {
        self.commit_all();
        if i == 0 || i >= self.doc.layers.len() {
            return false;
        }
        self.record();
        self.doc.merge_down(i);
        self.mark_all();
        true
    }

    pub fn move_layer(&mut self, from: usize, to: usize) -> bool {
        self.commit_all();
        let n = self.doc.layers.len();
        if from >= n || to >= n || from == to {
            return false;
        }
        self.record();
        self.doc.move_layer(from, to);
        self.mark_all();
        true
    }

    pub fn set_active_layer(&mut self, i: usize) -> bool {
        if i == self.doc.active {
            return true;
        }
        self.commit_all();
        self.doc.set_active(i)
    }

    pub fn set_layer_visible(&mut self, i: usize, visible: bool) -> bool {
        if i >= self.doc.layers.len() || self.doc.layers[i].visible == visible {
            return false;
        }
        self.finish_drag();
        self.record();
        self.doc.layers[i].visible = visible;
        self.mark_all();
        true
    }

    pub fn rename_layer(&mut self, i: usize, name: &str) -> bool {
        let name = name.trim();
        if i >= self.doc.layers.len() || name.is_empty() || self.doc.layers[i].name == name {
            return false;
        }
        self.record();
        self.doc.layers[i].name = name.to_string();
        true
    }

    pub fn layer_thumbnail(&self, i: usize, max: u32) -> Option<Image> {
        self.doc.layers.get(i).map(|l| l.image.thumbnail(max))
    }

    // ----- files -----------------------------------------------------------

    fn replace_document(&mut self, doc: Document) {
        self.drag = None;
        self.pending = None;
        self.selection = None;
        self.doc = doc;
        self.history.clear();
        self.modified = false;
        self.sync_frame();
    }

    pub fn new_document(&mut self, w: u32, h: u32) {
        let (mut w, mut h) = (w.clamp(1, codec::MAX_SIDE), h.clamp(1, codec::MAX_SIDE));
        while !codec::size_ok(w, h) {
            // Too many pixels: shrink proportionally until it fits.
            w = (w * 9 / 10).max(1);
            h = (h * 9 / 10).max(1);
        }
        self.replace_document(Document::new(w, h, Rgba::WHITE));
    }

    pub fn load_image(&mut self, img: Image) {
        self.replace_document(Document::from_image(img));
    }

    pub fn open_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        let img = codec::decode(bytes)?;
        self.load_image(img);
        Ok(())
    }

    pub fn export(&self, fmt: Format, quality: u8) -> Result<Vec<u8>, String> {
        codec::encode(&self.flatten(), fmt, quality)
    }

    // ----- keyboard helpers ------------------------------------------------

    /// Escape: cancels a selection being drawn, otherwise commits everything.
    pub fn escape(&mut self) {
        if matches!(self.drag, Some(Drag::NewSelection { .. }) | Some(Drag::Lasso { .. })) {
            self.drag = None;
        }
        self.commit_all();
    }

    /// Enter: finishes a polygon or commits a pending shape/selection.
    pub fn enter(&mut self) {
        self.commit_all();
    }

    // ----- UI state --------------------------------------------------------

    pub fn state(&self) -> EditorState {
        let pts = |v: &[Pt]| v.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>();
        let selection = match &self.drag {
            Some(Drag::NewSelection { start, cur }) => {
                let r = IRect::from_corners(start.x as i32, start.y as i32, cur.x as i32, cur.y as i32).clamp_to(self.width(), self.height());
                let (l, t, rr, b) = (r.x as f32, r.y as f32, r.right() as f32, r.bottom() as f32);
                Some(SelectionInfo { x: r.x, y: r.y, w: r.w, h: r.h, floating: false, freeform: false, drawing: true, outline: vec![vec![[l, t], [rr, t], [rr, b], [l, b]]] })
            }
            Some(Drag::Lasso { points }) => {
                let (lo, hi) = ShapeGeom::Polygon { points: points.clone(), closed: false }.bounds();
                let r = IRect::from_bounds(lo.x, lo.y, hi.x, hi.y);
                Some(SelectionInfo { x: r.x, y: r.y, w: r.w, h: r.h, floating: false, freeform: true, drawing: true, outline: vec![pts(points)] })
            }
            _ => self.selection.as_ref().map(|s| SelectionInfo {
                x: s.rect.x,
                y: s.rect.y,
                w: s.rect.w,
                h: s.rect.h,
                floating: s.is_floating(),
                freeform: s.is_freeform(),
                drawing: false,
                outline: s.outline_canvas().iter().map(|p| pts(p)).collect(),
            }),
        };
        let shape_edit = self.pending.as_ref().map(|ps| {
            let (lo, hi) = ps.geom.bounds();
            let handles = if ps.stage != ShapeStage::Editing {
                Vec::new()
            } else {
                match &ps.geom {
                    ShapeGeom::Line(..) | ShapeGeom::Curve(..) => ps.geom.points().iter().map(|p| HandleInfo { x: p.x, y: p.y, cursor: "pointer" }).collect(),
                    _ => Handle::ALL
                        .iter()
                        .map(|h| {
                            let p = h.position(lo, hi);
                            HandleInfo { x: p.x, y: p.y, cursor: h.cursor() }
                        })
                        .collect(),
                }
            };
            ShapeInfo { kind: ps.kind.name(), stage: ps.stage.name(), x: lo.x, y: lo.y, w: hi.x - lo.x, h: hi.y - lo.y, handles, points: pts(&ps.geom.points()) }
        });
        EditorState {
            width: self.width(),
            height: self.height(),
            tool: self.tool.name(),
            brush: self.brush.name(),
            shape: self.shape.name(),
            color1: self.color1.to_hex(),
            color2: self.color2.to_hex(),
            size: self.size(),
            outline: match self.outline {
                OutlineStyle::None => "none",
                OutlineStyle::Solid => "solid",
            },
            fill: match self.fill {
                FillStyle::None => "none",
                FillStyle::Solid => "solid",
            },
            transparent_selection: self.transparent_selection,
            fill_tolerance: self.fill_tolerance,
            can_undo: self.can_undo(),
            can_redo: self.can_redo(),
            modified: self.modified,
            dragging: self.drag.is_some(),
            selection,
            shape_edit,
            layers: self.doc.layers.iter().map(|l| LayerInfo { id: l.id, name: l.name.clone(), visible: l.visible }).collect(),
            active_layer: self.doc.active,
        }
    }

    pub fn state_json(&self) -> String {
        serde_json::to_string(&self.state()).unwrap_or_else(|_| "{}".into())
    }
}
