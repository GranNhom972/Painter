//! Copy / cut / paste, including the PNG round-trip used with the system clipboard.

mod common;
use common::*;
use paint_core::*;

fn select(e: &mut Editor, from: (f32, f32), to: (f32, f32)) {
    e.set_tool(Tool::RectSelect);
    drag(e, from, to);
}

#[test]
fn copy_needs_a_selection() {
    let e = Editor::new(10, 10);
    assert!(e.copy().is_none());
    let mut e = Editor::new(10, 10);
    assert!(e.cut().is_none());
    assert!(!e.can_undo());
}

#[test]
fn copy_returns_selected_pixels_without_changing_anything() {
    let mut e = with_square(50, 50, IRect::new(10, 10, 5, 5));
    select(&mut e, (8.0, 8.0), (18.0, 13.0));
    let img = e.copy().unwrap();
    assert_eq!((img.width(), img.height()), (10, 5));
    assert_eq!(img.get(2, 2), RED);
    assert_eq!(img.get(0, 0), WHITE);
    assert!(!e.is_modified());
    assert!(!e.selection().unwrap().is_floating());
}

#[test]
fn copy_freeform_is_transparent_outside_the_lasso() {
    let mut e = with_square(40, 40, IRect::new(0, 0, 40, 40));
    e.set_tool(Tool::FreeSelect);
    drag_with(&mut e, &[(0.0, 0.0), (20.0, 0.0), (0.0, 20.0)], L, NO);
    let img = e.copy().unwrap();
    assert_eq!(img.get(1, 1), RED);
    assert_eq!(img.get(img.width() as i32 - 1, img.height() as i32 - 1).a, 0);
}

#[test]
fn copy_of_a_resized_floating_selection() {
    let mut e = with_square(50, 50, IRect::new(0, 0, 10, 10));
    select(&mut e, (0.0, 0.0), (10.0, 10.0));
    drag(&mut e, (10.0, 10.0), (20.0, 20.0));
    let img = e.copy().unwrap();
    assert_eq!((img.width(), img.height()), (20, 20));
    assert!(img.is_uniform(RED));
}

#[test]
fn cut_removes_and_returns_pixels() {
    let mut e = with_square(50, 50, IRect::new(10, 10, 10, 10));
    e.set_color2(BLUE);
    select(&mut e, (10.0, 10.0), (20.0, 20.0));
    let img = e.cut().unwrap();
    assert!(img.is_uniform(RED));
    assert!(e.selection().is_none());
    assert_eq!(e.layer_pixel(15, 15), BLUE);
    e.undo();
    assert_eq!(e.layer_pixel(15, 15), RED);
}

#[test]
fn paste_creates_a_floating_selection() {
    let mut e = Editor::new(50, 50);
    e.set_tool(Tool::Brush);
    e.paste(Image::filled(8, 6, RED), 5, 7);
    assert_eq!(e.tool(), Tool::RectSelect);
    let s = e.selection().unwrap();
    assert!(s.is_floating());
    assert_eq!(s.rect, IRect::new(5, 7, 8, 6));
    assert_eq!(e.composite_pixel(5, 7), RED);
    assert_eq!(e.layer_pixel(5, 7), WHITE, "not committed yet");
    // The pasted image can be moved before committing.
    drag(&mut e, (9.0, 10.0), (29.0, 30.0));
    e.escape();
    assert_eq!(e.layer_pixel(25, 27), RED);
    assert_eq!(e.layer_pixel(5, 7), WHITE, "pasting leaves no hole");
    assert_eq!(count(&e, IRect::new(0, 0, 50, 50), RED), 48);
}

#[test]
fn paste_grows_the_canvas_when_needed() {
    let mut e = Editor::new(20, 10);
    e.set_color2(BLUE);
    e.paste(Image::filled(30, 25, RED), 0, 0);
    assert_eq!((e.width(), e.height()), (30, 25));
    e.deselect();
    assert_eq!(e.layer_pixel(29, 24), RED);
    e.undo();
    assert_eq!((e.width(), e.height()), (20, 10));
    assert!(e.selection().is_none());
    // Offset paste near the border.
    e.paste(Image::filled(10, 10, RED), 15, 5);
    assert_eq!((e.width(), e.height()), (25, 15));
    assert_eq!(e.doc().layers[0].image.get(22, 2), BLUE, "new area uses the secondary colour");
}

#[test]
fn paste_clamps_negative_positions() {
    let mut e = Editor::new(20, 20);
    e.paste(Image::filled(4, 4, RED), -10, -3);
    assert_eq!(e.selection().unwrap().rect, IRect::new(0, 0, 4, 4));
}

#[test]
fn paste_with_alpha_blends() {
    let mut e = Editor::new(10, 10);
    e.paste(Image::filled(4, 4, RED.with_alpha(128)), 0, 0);
    e.deselect();
    let p = e.layer_pixel(1, 1);
    assert_eq!(p.a, 255);
    assert!(p.g > 100 && p.r > 240, "{p:?}");
}

#[test]
fn paste_commits_previous_selection() {
    let mut e = with_square(40, 40, IRect::new(0, 0, 10, 10));
    select(&mut e, (0.0, 0.0), (10.0, 10.0));
    drag(&mut e, (5.0, 5.0), (25.0, 5.0));
    e.paste(Image::filled(2, 2, BLUE), 30, 30);
    assert_eq!(e.layer_pixel(20, 0), RED);
    assert_eq!(e.selection().unwrap().rect, IRect::new(30, 30, 2, 2));
}

#[test]
fn png_round_trip_like_the_system_clipboard() {
    let mut src = with_square(30, 30, IRect::new(5, 5, 10, 10));
    select(&mut src, (5.0, 5.0), (15.0, 15.0));
    let bytes = codec::encode(&src.copy().unwrap(), Format::Png, 100).unwrap();
    let mut dst = Editor::new(40, 40);
    dst.paste(codec::decode(&bytes).unwrap(), 20, 20);
    dst.deselect();
    assert_eq!(count(&dst, IRect::new(0, 0, 40, 40), RED), 100);
    assert_eq!(dst.layer_pixel(20, 20), RED);
}

#[test]
fn copy_then_paste_duplicates() {
    let mut e = with_square(40, 40, IRect::new(0, 0, 10, 10));
    select(&mut e, (0.0, 0.0), (10.0, 10.0));
    let img = e.copy().unwrap();
    e.paste(img, 0, 0);
    drag(&mut e, (5.0, 5.0), (25.0, 25.0));
    e.deselect();
    assert_eq!(count(&e, IRect::new(0, 0, 40, 40), RED), 200);
}
