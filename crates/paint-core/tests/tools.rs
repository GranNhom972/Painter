//! Pencil, brushes, eraser, fill, colour picker, text stamping, magnifier, sizes.

mod common;
use common::*;
use paint_core::*;

#[test]
fn tool_names_roundtrip() {
    for t in Tool::ALL {
        assert_eq!(Tool::from_name(t.name()), Some(t));
    }
    assert_eq!(Tool::from_name("lasso"), None);
}

#[test]
fn pencil_left_and_right_buttons() {
    let mut e = Editor::new(40, 40);
    e.set_tool(Tool::Pencil);
    e.set_color2(RED);
    drag(&mut e, (5.5, 5.5), (15.5, 5.5));
    assert_eq!(count(&e, IRect::new(0, 0, 40, 40), BLACK), 11);
    drag_with(&mut e, &[(5.5, 10.5), (5.5, 20.5)], R, NO);
    assert_eq!(count(&e, IRect::new(0, 0, 40, 40), RED), 11);
    // A single click draws a dot.
    click(&mut e, 30.5, 30.5);
    assert_eq!(e.layer_pixel(30, 30), BLACK);
    assert!(e.is_modified());
}

#[test]
fn pencil_size() {
    let mut e = Editor::new(40, 40);
    e.set_tool(Tool::Pencil);
    e.set_size(3);
    click(&mut e, 10.5, 10.5);
    assert_eq!(e.layer_pixel(9, 10), BLACK);
    assert_eq!(e.layer_pixel(11, 10), BLACK);
    assert_eq!(e.layer_pixel(8, 10), WHITE);
}

#[test]
fn each_tool_family_remembers_its_size() {
    let mut e = Editor::new(10, 10);
    e.set_tool(Tool::Pencil);
    assert_eq!(e.size(), 1);
    e.set_size(4);
    e.set_tool(Tool::Brush);
    assert_eq!(e.size(), 5);
    e.set_size(200);
    assert_eq!(e.size(), 100);
    e.set_size(0);
    assert_eq!(e.size(), 1);
    e.set_tool(Tool::Eraser);
    assert_eq!(e.size(), 10);
    e.set_tool(Tool::Shape);
    assert_eq!(e.size(), 3);
    e.set_tool(Tool::Pencil);
    assert_eq!(e.size(), 4);
}

#[test]
fn every_brush_paints_along_the_path() {
    for kind in BrushKind::ALL_BRUSHES {
        let mut e = Editor::new(80, 40);
        e.set_brush(kind);
        assert_eq!(e.tool(), Tool::Brush);
        assert_eq!(e.brush(), kind);
        e.set_size(8);
        e.set_color1(BLUE);
        drag_with(&mut e, &[(10.0, 20.0), (40.0, 20.0), (70.0, 20.0)], L, NO);
        let changed = count_not(&e, WHITE);
        assert!(changed > 40, "{kind:?} painted {changed} pixels");
        // Nothing far away from the stroke.
        assert_eq!(e.composite_pixel(40, 2), WHITE, "{kind:?}");
        assert_eq!(e.state().brush, kind.name());
    }
}

#[test]
fn brush_uses_secondary_colour_with_right_button() {
    let mut e = Editor::new(40, 20);
    e.set_brush(BrushKind::Brush);
    e.set_color2(GREEN);
    drag_with(&mut e, &[(5.0, 10.0), (35.0, 10.0)], R, NO);
    assert_eq!(e.composite_pixel(20, 10), GREEN);
}

#[test]
fn marker_is_translucent_and_does_not_build_up() {
    let mut e = Editor::new(60, 20);
    e.set_brush(BrushKind::Marker);
    e.set_size(6);
    drag_with(&mut e, &[(5.0, 10.0), (55.0, 10.0), (5.0, 10.0)], L, NO);
    let p = e.composite_pixel(30, 10);
    assert!((120..=136).contains(&p.r), "half-transparent black over white: {p:?}");
}

#[test]
fn airbrush_keeps_spraying_while_held() {
    let mut e = Editor::new(60, 60);
    e.set_brush(BrushKind::Airbrush);
    e.pointer_down(30.0, 30.0, L, NO);
    let first = count_not(&e, WHITE);
    for _ in 0..30 {
        e.tick();
    }
    let later = count_not(&e, WHITE);
    e.pointer_up(30.0, 30.0);
    assert!(later > first, "{first} -> {later}");
    // Ticks without a stroke do nothing.
    e.tick();
    assert_eq!(count_not(&e, WHITE), later);
}

#[test]
fn eraser_paints_secondary_colour_on_background() {
    let mut e = with_square(40, 40, IRect::new(0, 0, 40, 40));
    e.set_tool(Tool::Eraser);
    e.set_color2(GREEN);
    drag(&mut e, (5.0, 20.0), (35.0, 20.0));
    assert_eq!(e.layer_pixel(20, 20), GREEN);
    assert_eq!(e.layer_pixel(20, 2), RED);
}

#[test]
fn eraser_makes_upper_layers_transparent() {
    let mut e = Editor::new(40, 40);
    e.add_layer();
    e.set_tool(Tool::Brush);
    e.set_size(20);
    drag(&mut e, (5.0, 20.0), (35.0, 20.0));
    assert_eq!(e.layer_pixel(20, 20), BLACK);
    e.set_tool(Tool::Eraser);
    drag(&mut e, (5.0, 20.0), (35.0, 20.0));
    assert_eq!(e.layer_pixel(20, 20).a, 0);
    assert_eq!(e.composite_pixel(20, 20), WHITE);
}

#[test]
fn right_button_eraser_replaces_primary_by_secondary() {
    let mut img = Image::filled(40, 10, RED);
    img.fill_rect(IRect::new(20, 0, 20, 10), BLUE);
    let mut e = Editor::from_image(img);
    e.set_tool(Tool::Eraser);
    e.set_color1(RED);
    e.set_color2(GREEN);
    e.set_size(10);
    drag_with(&mut e, &[(2.0, 5.0), (38.0, 5.0)], R, NO);
    assert_eq!(e.layer_pixel(10, 5), GREEN);
    assert_eq!(e.layer_pixel(30, 5), BLUE, "other colours are kept");
}

#[test]
fn fill_bucket_left_and_right() {
    let mut e = with_square(40, 40, IRect::new(10, 10, 10, 10));
    e.set_tool(Tool::Fill);
    e.set_color1(BLUE);
    e.set_color2(GREEN);
    click(&mut e, 15.0, 15.0);
    assert_eq!(count(&e, IRect::new(0, 0, 40, 40), BLUE), 100);
    e.pointer_down(1.0, 1.0, R, NO);
    e.pointer_up(1.0, 1.0);
    assert_eq!(count(&e, IRect::new(0, 0, 40, 40), GREEN), 1500);
    // Filling with the same colour is a no-op without history.
    let steps = e.can_undo();
    click(&mut e, 15.0, 15.0);
    assert!(steps);
    e.undo();
    e.undo();
    assert_eq!(count(&e, IRect::new(0, 0, 40, 40), RED), 100);
    assert!(!e.can_undo());
    // Outside the canvas: nothing.
    click(&mut e, -5.0, 5.0);
    assert!(!e.can_undo());
}

#[test]
fn fill_tolerance() {
    let mut img = Image::filled(10, 1, Rgba::rgb(100, 100, 100));
    img.set(5, 0, Rgba::rgb(104, 100, 100));
    let mut e = Editor::from_image(img);
    e.set_tool(Tool::Fill);
    click(&mut e, 0.0, 0.0);
    assert_eq!(e.layer_pixel(9, 0), Rgba::rgb(100, 100, 100));
    e.undo();
    e.set_fill_tolerance(8);
    click(&mut e, 0.0, 0.0);
    assert_eq!(e.layer_pixel(9, 0), BLACK);
    assert_eq!(e.state().fill_tolerance, 8);
}

#[test]
fn colour_picker_sets_colours_and_returns_to_previous_tool() {
    let mut e = with_square(40, 40, IRect::new(0, 0, 20, 40));
    e.set_tool(Tool::Brush);
    e.set_tool(Tool::Picker);
    click(&mut e, 5.0, 5.0);
    assert_eq!(e.color1(), RED);
    assert_eq!(e.tool(), Tool::Brush);
    e.set_tool(Tool::Picker);
    e.pointer_down(30.0, 5.0, R, NO);
    e.pointer_up(30.0, 5.0);
    assert_eq!(e.color2(), WHITE);
    // Picking a transparent pixel gives white.
    let mut t = Editor::from_image(Image::new(4, 4));
    t.set_tool(Tool::Picker);
    click(&mut t, 1.0, 1.0);
    assert_eq!(t.color1(), WHITE);
    assert_eq!(t.tool(), Tool::Pencil);
    // Outside the canvas nothing happens.
    t.set_tool(Tool::Picker);
    click(&mut t, 10.0, 10.0);
    assert_eq!(t.tool(), Tool::Picker);
}

#[test]
fn picker_sees_all_visible_layers() {
    let mut e = Editor::new(10, 10);
    e.add_layer();
    e.paste(Image::filled(5, 5, BLUE), 0, 0);
    e.set_tool(Tool::Picker);
    click(&mut e, 2.0, 2.0);
    assert_eq!(e.color1(), BLUE);
}

#[test]
fn text_is_stamped_as_an_image() {
    let mut e = Editor::new(40, 20);
    let mut glyphs = Image::new(10, 5);
    glyphs.fill_rect(IRect::new(0, 0, 3, 5), BLACK);
    e.stamp_image(4, 6, &glyphs);
    assert_eq!(e.layer_pixel(5, 8), BLACK);
    assert_eq!(e.layer_pixel(10, 8), WHITE, "transparent text background");
    e.undo();
    assert_eq!(e.layer_pixel(5, 8), WHITE);
}

#[test]
fn text_and_magnifier_clicks_do_not_paint() {
    for t in [Tool::Text, Tool::Magnifier] {
        let mut e = Editor::new(10, 10);
        e.set_tool(t);
        click(&mut e, 5.0, 5.0);
        assert!(!e.can_undo());
        assert_eq!(count_not(&e, WHITE), 0);
    }
}

#[test]
fn colours_and_swap() {
    let mut e = Editor::new(4, 4);
    assert_eq!((e.color1(), e.color2()), (BLACK, WHITE));
    e.set_color1(RED);
    e.swap_colors();
    assert_eq!((e.color1(), e.color2()), (WHITE, RED));
    let st = e.state();
    assert_eq!((st.color1.as_str(), st.color2.as_str()), ("#ffffff", "#ed1c24"));
}

#[test]
fn tool_cursors() {
    let mut e = Editor::new(10, 10);
    for (t, c) in [
        (Tool::Pencil, "pencil"),
        (Tool::Brush, "brush"),
        (Tool::Eraser, "eraser"),
        (Tool::Fill, "fill"),
        (Tool::Picker, "picker"),
        (Tool::Text, "text"),
        (Tool::Magnifier, "zoom-in"),
        (Tool::Shape, "crosshair"),
        (Tool::RectSelect, "crosshair"),
    ] {
        e.set_tool(t);
        assert_eq!(e.cursor_at(5.0, 5.0), c, "{t:?}");
    }
}

#[test]
fn second_button_during_a_drag_is_ignored() {
    let mut e = Editor::new(20, 20);
    e.set_tool(Tool::Pencil);
    e.pointer_down(2.5, 2.5, L, NO);
    e.pointer_down(10.5, 10.5, R, NO);
    e.pointer_move(5.5, 2.5, NO);
    e.pointer_up(5.5, 2.5);
    assert_eq!(count_not(&e, WHITE), 4);
    // Moves and releases without a press are harmless.
    e.pointer_move(1.0, 1.0, NO);
    e.pointer_up(1.0, 1.0);
}
