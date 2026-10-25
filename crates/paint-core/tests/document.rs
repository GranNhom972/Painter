//! History, whole-image operations, layers, files and the serialised UI state.

mod common;
use common::*;
use paint_core::*;

// ----- history -------------------------------------------------------------

#[test]
fn undo_redo_strokes() {
    let mut e = Editor::new(30, 30);
    assert!(!e.undo() && !e.redo());
    e.set_tool(Tool::Pencil);
    drag(&mut e, (1.5, 1.5), (10.5, 1.5));
    drag(&mut e, (1.5, 5.5), (10.5, 5.5));
    assert!(e.undo());
    assert_eq!(e.layer_pixel(5, 5), WHITE);
    assert_eq!(e.layer_pixel(5, 1), BLACK);
    assert!(e.can_redo());
    assert!(e.redo());
    assert_eq!(e.layer_pixel(5, 5), BLACK);
    // A new action clears the redo stack.
    e.undo();
    click(&mut e, 20.5, 20.5);
    assert!(!e.can_redo());
    let st = e.state();
    assert!(st.can_undo && !st.can_redo);
}

#[test]
fn undo_during_a_stroke_finishes_it_first() {
    let mut e = Editor::new(30, 30);
    e.set_tool(Tool::Pencil);
    e.pointer_down(1.5, 1.5, L, NO);
    e.pointer_move(10.5, 1.5, NO);
    assert!(e.undo());
    assert_eq!(count_not(&e, WHITE), 0);
    assert!(!e.state().dragging);
}

#[test]
fn modified_flag() {
    let mut e = Editor::new(10, 10);
    assert!(!e.is_modified());
    e.set_tool(Tool::Pencil);
    click(&mut e, 1.0, 1.0);
    assert!(e.is_modified());
    e.mark_saved();
    assert!(!e.is_modified());
    e.undo();
    assert!(e.is_modified());
}

#[test]
fn long_histories_are_bounded() {
    let mut e = Editor::new(16, 16);
    e.set_tool(Tool::Pencil);
    for i in 0..150 {
        click(&mut e, (i % 16) as f32, (i / 16) as f32);
    }
    let mut n = 0;
    while e.undo() {
        n += 1;
    }
    assert_eq!(n, 100);
}

// ----- whole-image operations -----------------------------------------------

#[test]
fn rotate_flip_whole_image() {
    let mut img = Image::filled(30, 20, WHITE);
    img.set(0, 0, RED);
    let mut e = Editor::from_image(img);
    e.transform(Transform::RotateCw);
    assert_eq!((e.width(), e.height()), (20, 30));
    assert_eq!(e.layer_pixel(19, 0), RED);
    e.transform(Transform::FlipHorizontal);
    assert_eq!(e.layer_pixel(0, 0), RED);
    e.transform(Transform::FlipVertical);
    assert_eq!(e.layer_pixel(0, 29), RED);
    e.transform(Transform::Rotate180);
    assert_eq!(e.layer_pixel(19, 0), RED);
    e.transform(Transform::RotateCcw);
    assert_eq!((e.width(), e.height()), (30, 20));
    for _ in 0..5 {
        e.undo();
    }
    assert_eq!(e.layer_pixel(0, 0), RED);
    assert_eq!((e.width(), e.height()), (30, 20));
}

#[test]
fn resize_and_skew_whole_image() {
    let mut e = with_square(40, 20, IRect::new(0, 0, 20, 20));
    e.transform(Transform::Resize(80, 10));
    assert_eq!((e.width(), e.height()), (80, 10));
    assert_eq!(e.layer_pixel(10, 5), RED);
    assert_eq!(e.layer_pixel(70, 5), WHITE);
    e.set_color2(GREEN);
    e.transform(Transform::Skew(45.0, 0.0));
    assert_eq!((e.width(), e.height()), (90, 10));
    assert_eq!(e.layer_pixel(0, 9), GREEN, "uncovered corner uses the secondary colour");
}

#[test]
fn canvas_resize_keeps_pixels() {
    let mut e = with_square(20, 20, IRect::new(0, 0, 5, 5));
    e.set_color2(BLUE);
    e.resize_canvas(40, 30);
    assert_eq!((e.width(), e.height()), (40, 30));
    assert_eq!(e.layer_pixel(2, 2), RED);
    assert_eq!(e.layer_pixel(39, 29), BLUE);
    e.resize_canvas(3, 3);
    assert_eq!(count(&e, IRect::new(0, 0, 3, 3), RED), 9);
    // Same size: no history entry.
    let before = e.can_redo();
    e.resize_canvas(3, 3);
    assert_eq!(e.can_redo(), before);
    e.undo();
    e.undo();
    assert_eq!((e.width(), e.height()), (20, 20));
    e.resize_canvas(0, 100_000);
    assert_eq!((e.width(), e.height()), (1, codec::MAX_SIDE));
}

#[test]
fn canvas_resize_from_the_top_left_handles() {
    let mut e = with_square(20, 20, IRect::new(0, 0, 5, 5));
    e.resize_canvas_at(30, 25, 10, 5);
    assert_eq!((e.width(), e.height()), (30, 25));
    assert_eq!(e.layer_pixel(10, 5), RED);
    assert_eq!(e.layer_pixel(0, 0), WHITE);
    // Shrinking from the left crops that side.
    e.resize_canvas_at(27, 25, -3, 0);
    assert_eq!(e.layer_pixel(7, 5), RED);
    assert_eq!(e.layer_pixel(6, 5), WHITE);
    e.undo();
    e.undo();
    assert_eq!((e.width(), e.height()), (20, 20));
}

#[test]
fn background_removal_whole_and_selection() {
    let mut e = with_square(40, 40, IRect::new(10, 10, 20, 20));
    assert!(e.remove_background(32));
    assert_eq!(e.layer_pixel(0, 0).a, 0);
    assert_eq!(e.layer_pixel(15, 15), RED);
    assert!(!e.remove_background(32), "nothing left to remove");
    e.undo();
    assert_eq!(e.layer_pixel(0, 0), WHITE);
    // Restricted to a selection.
    e.set_tool(Tool::RectSelect);
    drag(&mut e, (0.0, 0.0), (20.0, 40.0));
    assert!(e.remove_background(32));
    assert_eq!(e.layer_pixel(1, 1).a, 0);
    assert_eq!(e.layer_pixel(35, 35), WHITE);
}

#[test]
fn background_removal_of_floating_selection_commits_it() {
    let mut e = with_square(40, 40, IRect::new(0, 0, 10, 10));
    e.set_tool(Tool::RectSelect);
    drag(&mut e, (0.0, 0.0), (20.0, 20.0));
    drag(&mut e, (10.0, 10.0), (25.0, 25.0));
    e.remove_background(10);
    assert!(!e.selection().unwrap().is_floating());
    assert_eq!(e.layer_pixel(15, 15), RED);
}

// ----- layers ---------------------------------------------------------------

#[test]
fn layers_are_independent() {
    let mut e = Editor::new(30, 30);
    e.add_layer();
    assert_eq!(e.state().active_layer, 1);
    e.set_tool(Tool::Fill);
    e.set_color1(RED);
    click(&mut e, 5.0, 5.0);
    assert!(e.doc().layers[0].image.is_uniform(WHITE));
    assert_eq!(e.composite_pixel(5, 5), RED);
    e.set_layer_visible(1, false);
    assert_eq!(e.composite_pixel(5, 5), WHITE);
    assert!(!e.set_layer_visible(1, false));
    e.set_layer_visible(1, true);
    e.undo();
    e.undo();
    assert_eq!(e.composite_pixel(5, 5), RED);
}

#[test]
fn layer_order_duplicate_merge_delete_rename() {
    let mut e = Editor::new(10, 10);
    e.add_layer();
    e.set_tool(Tool::Fill);
    e.set_color1(RED);
    click(&mut e, 1.0, 1.0);
    // Moving the red layer under the white background hides it.
    assert!(e.move_layer(1, 0));
    assert_eq!(e.composite_pixel(1, 1), WHITE);
    assert_eq!(e.state().active_layer, 0, "active layer follows its content");
    assert!(e.move_layer(0, 1));
    assert!(e.duplicate_layer(1));
    assert_eq!(e.doc().layers.len(), 3);
    assert_eq!(e.state().layers[2].name, "Layer 2 (copy)");
    assert!(e.rename_layer(2, "  Ciel  "));
    assert_eq!(e.state().layers[2].name, "Ciel");
    assert!(!e.rename_layer(2, " "));
    assert!(!e.rename_layer(9, "x"));
    assert!(e.merge_down(2));
    assert_eq!(e.doc().layers.len(), 2);
    assert!(!e.merge_down(0));
    assert!(e.delete_layer(0));
    assert_eq!(e.composite_pixel(1, 1), RED);
    assert!(!e.delete_layer(0), "the last layer cannot be deleted");
    assert!(!e.duplicate_layer(5) && !e.move_layer(0, 3));
    let thumb = e.layer_thumbnail(0, 4).unwrap();
    assert_eq!((thumb.width(), thumb.height()), (4, 4));
    assert!(e.layer_thumbnail(3, 4).is_none());
}

#[test]
fn switching_layer_commits_the_selection_on_its_layer() {
    let mut e = with_square(30, 30, IRect::new(0, 0, 10, 10));
    e.add_layer();
    assert!(e.set_active_layer(0));
    e.set_tool(Tool::RectSelect);
    drag(&mut e, (0.0, 0.0), (10.0, 10.0));
    drag(&mut e, (5.0, 5.0), (20.0, 20.0));
    assert!(e.set_active_layer(1));
    assert!(e.selection().is_none());
    assert_eq!(e.doc().layers[0].image.get(15, 15), RED);
    assert!(!e.set_active_layer(7));
    assert!(e.set_active_layer(1));
}

#[test]
fn merging_a_hidden_layer_discards_it() {
    let mut e = Editor::new(10, 10);
    e.add_layer();
    e.set_tool(Tool::Fill);
    click(&mut e, 1.0, 1.0);
    e.set_layer_visible(1, false);
    e.merge_down(1);
    assert!(e.doc().layers[0].image.is_uniform(WHITE));
}

// ----- files ----------------------------------------------------------------

#[test]
fn new_document_resets_everything() {
    let mut e = with_square(30, 30, IRect::new(0, 0, 10, 10));
    e.add_layer();
    e.set_tool(Tool::RectSelect);
    drag(&mut e, (0.0, 0.0), (5.0, 5.0));
    e.new_document(64, 48);
    assert_eq!((e.width(), e.height()), (64, 48));
    assert!(e.selection().is_none() && !e.can_undo() && !e.is_modified());
    assert_eq!(e.doc().layers.len(), 1);
    assert_eq!(count_not(&e, WHITE), 0);
    let r = e.render().unwrap();
    assert_eq!(r, IRect::new(0, 0, 64, 48));
    assert_eq!(e.frame().get(63, 47), WHITE);
}

#[test]
fn open_and_export_every_format() {
    let mut src = with_square(24, 16, IRect::new(0, 0, 12, 16));
    for fmt in Format::ALL {
        let bytes = src.export(fmt, 90).unwrap();
        let mut e = Editor::new(5, 5);
        e.open_bytes(&bytes).unwrap();
        assert_eq!((e.width(), e.height()), (24, 16), "{fmt:?}");
        assert!(e.layer_pixel(3, 3).distance(RED) <= 12, "{fmt:?}");
        assert!(!e.is_modified());
    }
    src.mark_saved();
    assert!(!src.is_modified());
}

#[test]
fn opening_garbage_keeps_the_current_picture() {
    let mut e = with_square(10, 10, IRect::new(0, 0, 5, 5));
    assert!(e.open_bytes(b"\x89PNG broken").is_err());
    assert_eq!((e.width(), e.height()), (10, 10));
    assert_eq!(e.layer_pixel(1, 1), RED);
}

#[test]
fn export_flattens_layers_and_floating_selection() {
    let mut e = Editor::new(20, 20);
    e.add_layer();
    e.paste(Image::filled(5, 5, BLUE), 10, 10);
    let flat = e.flatten();
    assert_eq!(flat.get(12, 12), BLUE);
    assert_eq!(flat.get(1, 1), WHITE);
    let png = e.export(Format::Png, 100).unwrap();
    let back = codec::decode(&png).unwrap();
    assert_eq!(back.get(12, 12), BLUE);
    assert!(e.selection().unwrap().is_floating(), "exporting does not commit");
}

// ----- rendering & state -----------------------------------------------------

#[test]
fn render_reports_dirty_areas() {
    let mut e = Editor::new(50, 50);
    assert_eq!(e.render(), Some(IRect::new(0, 0, 50, 50)));
    assert_eq!(e.render(), None);
    e.set_tool(Tool::Pencil);
    click(&mut e, 10.5, 20.5);
    let r = e.render().unwrap();
    assert!(r.contains(10, 20) && r.w < 10);
    assert_eq!(e.frame().get(10, 20), BLACK);
    e.transform(Transform::RotateCw);
    assert_eq!(e.render(), Some(IRect::new(0, 0, 50, 50)));
    assert_eq!((e.frame().width(), e.frame().height()), (50, 50));
}

#[test]
fn frame_shows_floating_selection_and_transparent_key() {
    let mut e = Editor::new(20, 20);
    e.paste(Image::filled(4, 4, WHITE), 0, 0);
    e.render();
    assert_eq!(e.frame().get(1, 1), WHITE);
    e.set_transparent_selection(true);
    e.render();
    assert_eq!(e.frame().get(1, 1), WHITE, "key colour shows the layer below (also white)");
    e.set_color2(BLACK);
    e.render();
    assert!(e.state().transparent_selection);
}

#[test]
fn state_json_describes_the_editor() {
    let mut e = Editor::new(64, 32);
    e.set_shape(ShapeKind::Rectangle);
    drag(&mut e, (4.0, 4.0), (20.0, 20.0));
    let v: serde_json::Value = serde_json::from_str(&e.state_json()).unwrap();
    assert_eq!(v["width"], 64);
    assert_eq!(v["tool"], "shape");
    assert_eq!(v["shape"], "rectangle");
    assert_eq!(v["shape_edit"]["handles"].as_array().unwrap().len(), 8);
    assert_eq!(v["layers"][0]["name"], "Layer 1");
    assert_eq!(v["can_undo"], true);
    assert!(v["selection"].is_null());
    for key in ["color1", "color2", "size", "outline", "fill", "modified", "brush", "active_layer", "dragging"] {
        assert!(!v[key].is_null(), "{key}");
    }
}
