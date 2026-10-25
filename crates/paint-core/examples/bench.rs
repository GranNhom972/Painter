//! Rough timings of interactive operations on a large picture.
//! `cargo run --release -p paint-core --example bench [width height]`

use paint_core::{BrushKind, Button, Editor, Image, Mods, Rgba, ShapeKind, Tool, Transform};
use std::time::Instant;

fn time(label: &str, n: usize, mut f: impl FnMut()) {
    let t = Instant::now();
    f();
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("{label:<44} {ms:>9.1} ms total {:>8.2} ms/event", ms / n.max(1) as f64);
}

fn main() {
    let args: Vec<u32> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let (w, h) = (args.first().copied().unwrap_or(4000), args.get(1).copied().unwrap_or(3000));
    println!("picture {w}x{h}");
    let mut e = Editor::new(w, h);
    let no = Mods::default();
    time("first full render", 1, || {
        e.render();
    });

    for (kind, size) in [(BrushKind::Brush, 30), (BrushKind::Airbrush, 30), (BrushKind::Watercolor, 60), (BrushKind::Oil, 40)] {
        e.set_brush(kind);
        e.set_size(size);
        let n = 200;
        time(&format!("brush {kind:?} size {size}: {n} moves + render"), n, || {
            e.pointer_down(100.0, 100.0, Button::Left, no);
            for i in 0..n {
                let t = i as f32 / n as f32;
                e.pointer_move(100.0 + t * (w as f32 - 200.0), 100.0 + (t * 12.0).sin() * 300.0 + h as f32 / 3.0, no);
                e.render();
            }
            e.pointer_up(0.0, 0.0);
        });
    }

    e.set_tool(Tool::Pencil);
    time("pencil 500 moves + render", 500, || {
        e.pointer_down(10.0, 10.0, Button::Left, no);
        for i in 0..500 {
            e.pointer_move(10.0 + i as f32 * 3.0, 10.0 + (i % 50) as f32 * 4.0, no);
            e.render();
        }
        e.pointer_up(0.0, 0.0);
    });

    for kind in [ShapeKind::Rectangle, ShapeKind::Oval, ShapeKind::Star5, ShapeKind::Heart] {
        e.set_shape(kind);
        e.set_size(8);
        let n = 60;
        time(&format!("shape {kind:?}: {n} drag moves + render"), n, || {
            e.pointer_down(200.0, 200.0, Button::Left, no);
            for i in 0..n {
                let t = (i + 1) as f32 / n as f32;
                e.pointer_move(200.0 + t * (w as f32 * 0.6), 200.0 + t * (h as f32 * 0.6), no);
                e.render();
            }
            e.pointer_up(0.0, 0.0);
            e.enter();
        });
    }

    e.set_tool(Tool::RectSelect);
    e.select_rect(paint_core::IRect::new(100, 100, (w / 2) as i32, (h / 2) as i32));
    let n = 60;
    time(&format!("move a {}x{} selection: {n} moves + render", w / 2, h / 2), n, || {
        e.pointer_down(500.0, 500.0, Button::Left, no);
        for i in 0..n {
            e.pointer_move(500.0 + i as f32 * 10.0, 500.0 + i as f32 * 5.0, no);
            e.render();
        }
        e.pointer_up(0.0, 0.0);
    });
    let n = 30;
    time(&format!("resize the selection: {n} moves + render"), n, || {
        let r = e.selection().unwrap().rect;
        let (x, y) = (r.right() as f32, r.bottom() as f32);
        e.pointer_down(x, y, Button::Left, no);
        for i in 0..n {
            e.pointer_move(x + i as f32 * 8.0, y + i as f32 * 4.0, no);
            e.render();
        }
        e.pointer_up(0.0, 0.0);
    });
    e.deselect();

    e.set_tool(Tool::Fill);
    e.set_color1(Rgba::rgb(0, 162, 232));
    time("bucket fill of the background", 1, || {
        e.pointer_down(1.0, 1.0, Button::Left, no);
        e.pointer_up(1.0, 1.0);
        e.render();
    });
    time("rotate whole picture", 1, || {
        e.transform(Transform::RotateCw);
        e.render();
    });
    time("undo (full redraw)", 1, || {
        e.undo();
        e.render();
    });
    e.add_layer();
    e.add_layer();
    time("full render with 3 layers", 1, || {
        e.invalidate();
        e.render();
    });
    time("copy selection of the whole picture", 1, || {
        e.select_all();
        let _ = e.copy();
    });
    time("paste a 2000x1500 picture", 1, || {
        e.paste(Image::filled(2000, 1500, Rgba::rgb(237, 28, 36)), 0, 0);
        e.render();
    });
    time("export PNG", 1, || {
        let _ = e.export(paint_core::Format::Png, 100);
    });
}
