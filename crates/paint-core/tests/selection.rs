//! Rectangle and free-form selection: creation, move, copy-move, resize, delete,
//! invert, transparent mode, crop, nudge, transforms, layers and undo.

mod common;
use common::*;
use paint_core::*;

fn select(e: &mut Editor, from: (f32, f32), to: (f32, f32)) {
    e.set_tool(Tool::RectSelect);
    drag(e, from, to);
}

#[test]
fn drag_creates_rectangle_selection() {
    let mut e = Editor::new(100, 80);
    select(&mut e, (10.0, 10.0), (30.0, 25.0));
    let s = e.selection().expect("selection");
    assert_eq!(s.rect, IRect::new(10, 10, 20, 15));
    assert!(!s.is_floating() && !s.is_freeform());
    let st = e.state().selection.unwrap();
    assert_eq!((st.x, st.y, st.w, st.h), (10, 10, 20, 15));
    assert!(!st.drawing);
    // Creating a selection does not modify the picture.
    assert!(!e.is_modified());
    assert!(!e.can_undo());
}

#[test]
fn selection_corners_are_the_pixels_under_the_pointer() {
    let mut e = Editor::new(100, 80);
    select(&mut e, (10.7, 10.2), (30.2, 25.9));
    assert_eq!(e.selection().unwrap().rect, IRect::new(10, 10, 20, 15));
}

#[test]
fn selection_drag_in_any_direction_and_clamped_to_canvas() {
    let mut e = Editor::new(100, 80);
    select(&mut e, (30.0, 25.0), (10.0, 10.0));
    assert_eq!(e.selection().unwrap().rect, IRect::new(10, 10, 20, 15));
    e.deselect();
    select(&mut e, (90.0, 70.0), (150.0, 120.0));
    assert_eq!(e.selection().unwrap().rect, IRect::new(90, 70, 10, 10));
    e.deselect();
    select(&mut e, (-20.0, -20.0), (5.0, 5.0));
    assert_eq!(e.selection().unwrap().rect, IRect::new(0, 0, 5, 5));
}

#[test]
fn rubber_band_is_reported_while_dragging() {
    let mut e = Editor::new(100, 80);
    e.set_tool(Tool::RectSelect);
    e.pointer_down(10.0, 10.0, L, NO);
    e.pointer_move(40.0, 30.0, NO);
    let st = e.state();
    assert!(st.dragging);
    let s = st.selection.unwrap();
    assert!(s.drawing);
    assert_eq!((s.w, s.h), (30, 20));
    // Shift makes it square.
    e.pointer_move(40.0, 20.0, SHIFT);
    let s = e.state().selection.unwrap();
    assert_eq!((s.w, s.h), (30, 30));
    e.pointer_up(40.0, 20.0);
}

#[test]
fn click_without_drag_clears_selection() {
    let mut e = Editor::new(100, 80);
    select(&mut e, (10.0, 10.0), (30.0, 25.0));
    click(&mut e, 80.0, 70.0);
    assert!(e.selection().is_none());
}

#[test]
fn escape_cancels_rubber_band() {
    let mut e = Editor::new(100, 80);
    e.set_tool(Tool::RectSelect);
    e.pointer_down(10.0, 10.0, L, NO);
    e.pointer_move(40.0, 30.0, NO);
    e.escape();
    assert!(e.selection().is_none());
    assert!(!e.state().dragging);
}

#[test]
fn moving_a_selection_lifts_pixels_and_leaves_secondary_colour() {
    let mut e = with_square(100, 80, IRect::new(10, 10, 10, 10));
    e.set_color2(Rgba::rgb(255, 255, 0));
    select(&mut e, (10.0, 10.0), (20.0, 20.0));
    drag(&mut e, (15.0, 15.0), (45.0, 35.0));
    let s = e.selection().unwrap();
    assert!(s.is_floating());
    assert_eq!(s.rect, IRect::new(40, 30, 10, 10));
    // Hole filled with the secondary colour; floating pixels visible at the new place.
    assert_eq!(e.layer_pixel(10, 10), Rgba::rgb(255, 255, 0));
    assert_eq!(e.composite_pixel(40, 30), RED);
    assert_eq!(e.composite_pixel(49, 39), RED);
    // Not yet in the layer.
    assert_eq!(e.layer_pixel(40, 30), WHITE);
    // Clicking outside commits.
    click(&mut e, 90.0, 5.0);
    assert!(e.selection().is_none());
    assert_eq!(e.layer_pixel(40, 30), RED);
    assert_eq!(e.layer_pixel(49, 39), RED);
    assert_eq!(e.layer_pixel(50, 40), WHITE);
    assert!(e.is_modified());
}

#[test]
fn ctrl_drag_moves_a_copy() {
    let mut e = with_square(100, 80, IRect::new(10, 10, 10, 10));
    select(&mut e, (10.0, 10.0), (20.0, 20.0));
    drag_with(&mut e, &[(15.0, 15.0), (65.0, 15.0)], L, CTRL);
    e.deselect();
    assert_eq!(e.layer_pixel(10, 10), RED);
    assert_eq!(e.layer_pixel(60, 10), RED);
    assert_eq!(count(&e, IRect::new(0, 0, 100, 80), RED), 200);
}

#[test]
fn ctrl_drag_of_floating_selection_stamps_copies() {
    let mut e = with_square(100, 80, IRect::new(0, 0, 10, 10));
    select(&mut e, (0.0, 0.0), (10.0, 10.0));
    drag(&mut e, (5.0, 5.0), (25.0, 5.0)); // floating at x=20
    drag_with(&mut e, &[(25.0, 5.0), (45.0, 5.0)], L, CTRL); // stamps at 20, floats to 40
    e.deselect();
    assert_eq!(e.layer_pixel(0, 0), WHITE);
    assert_eq!(e.layer_pixel(20, 0), RED);
    assert_eq!(e.layer_pixel(40, 0), RED);
    assert_eq!(count(&e, IRect::new(0, 0, 100, 80), RED), 200);
}

#[test]
fn resize_handle_scales_content() {
    let mut e = with_square(100, 80, IRect::new(10, 10, 10, 10));
    select(&mut e, (10.0, 10.0), (20.0, 20.0));
    assert_eq!(e.cursor_at(20.0, 20.0), "nwse-resize");
    assert_eq!(e.cursor_at(15.0, 10.0), "ns-resize");
    drag(&mut e, (20.0, 20.0), (30.0, 40.0));
    assert_eq!(e.selection().unwrap().rect, IRect::new(10, 10, 20, 30));
    e.deselect();
    assert_eq!(count(&e, IRect::new(0, 0, 100, 80), RED), 600);
    assert_eq!(e.layer_pixel(29, 39), RED);
    assert_eq!(e.layer_pixel(30, 40), WHITE);
}

#[test]
fn shift_resize_keeps_aspect_ratio() {
    let mut e = with_square(100, 80, IRect::new(10, 10, 10, 10));
    select(&mut e, (10.0, 10.0), (20.0, 20.0));
    drag_with(&mut e, &[(20.0, 20.0), (40.0, 25.0)], L, SHIFT);
    assert_eq!(e.selection().unwrap().rect, IRect::new(10, 10, 30, 30));
}

#[test]
fn west_handle_resizes_from_the_left() {
    let mut e = with_square(100, 80, IRect::new(10, 10, 10, 10));
    select(&mut e, (10.0, 10.0), (20.0, 20.0));
    drag(&mut e, (10.0, 15.0), (5.0, 15.0));
    assert_eq!(e.selection().unwrap().rect, IRect::new(5, 10, 15, 10));
}

#[test]
fn handle_tolerance_follows_zoom() {
    let mut e = Editor::new(100, 80);
    select(&mut e, (10.0, 10.0), (40.0, 40.0));
    assert_eq!(e.cursor_at(44.0, 44.0), "nwse-resize");
    e.set_zoom(8.0);
    assert_eq!(e.cursor_at(44.0, 44.0), "crosshair");
    assert_eq!(e.cursor_at(40.5, 40.5), "nwse-resize");
    assert_eq!(e.cursor_at(25.0, 25.0), "move");
}

#[test]
fn freeform_selection_moves_only_the_lasso_area() {
    let mut e = with_square(120, 80, IRect::new(10, 10, 40, 40));
    e.set_tool(Tool::FreeSelect);
    drag_with(&mut e, &[(10.0, 10.0), (50.0, 10.0), (10.0, 50.0), (10.0, 10.0)], L, NO);
    let s = e.selection().unwrap();
    assert!(s.is_freeform());
    assert_eq!(s.rect, IRect::new(10, 10, 40, 40));
    assert!(s.contains(12, 12));
    assert!(!s.contains(45, 45));
    drag(&mut e, (15.0, 15.0), (75.0, 15.0));
    e.deselect();
    // Inside the triangle: moved.
    assert_eq!(e.layer_pixel(12, 12), WHITE);
    assert_eq!(e.layer_pixel(72, 12), RED);
    // Outside the triangle but inside its bounding box: untouched at both ends.
    assert_eq!(e.layer_pixel(45, 45), RED);
    assert_eq!(e.layer_pixel(105, 45), WHITE);
}

#[test]
fn freeform_too_small_gives_no_selection() {
    let mut e = Editor::new(50, 50);
    e.set_tool(Tool::FreeSelect);
    drag_with(&mut e, &[(10.0, 10.0), (10.2, 10.0)], L, NO);
    assert!(e.selection().is_none());
}

#[test]
fn lasso_outline_reported_while_drawing() {
    let mut e = Editor::new(50, 50);
    e.set_tool(Tool::FreeSelect);
    e.pointer_down(5.0, 5.0, L, NO);
    e.pointer_move(30.0, 5.0, NO);
    e.pointer_move(30.0, 30.0, NO);
    let s = e.state().selection.unwrap();
    assert!(s.drawing && s.freeform);
    assert_eq!(s.outline[0].len(), 3);
    assert_eq!((s.x, s.y, s.w, s.h), (5, 5, 25, 25));
    e.pointer_up(30.0, 30.0);
    assert!(e.selection().is_some());
}

#[test]
fn select_all_and_switching_select_tools() {
    let mut e = Editor::new(64, 48);
    e.set_tool(Tool::Brush);
    e.select_all();
    assert_eq!(e.tool(), Tool::RectSelect);
    assert_eq!(e.selection().unwrap().rect, IRect::new(0, 0, 64, 48));
    e.set_tool(Tool::FreeSelect);
    assert!(e.selection().is_some(), "switching between selection tools keeps the selection");
    e.set_tool(Tool::Pencil);
    assert!(e.selection().is_none(), "other tools commit the selection");
}

#[test]
fn invert_then_delete_keeps_the_inner_area() {
    let mut img = Image::filled(40, 40, RED);
    img.set(0, 0, RED);
    let mut e = Editor::from_image(img);
    select(&mut e, (10.0, 10.0), (20.0, 20.0));
    e.invert_selection();
    let s = e.selection().unwrap();
    assert!(s.contains(0, 0) && !s.contains(15, 15));
    assert_eq!(e.state().selection.unwrap().outline.len(), 2);
    e.delete_selection();
    assert_eq!(e.layer_pixel(0, 0), WHITE);
    assert_eq!(e.layer_pixel(39, 39), WHITE);
    assert_eq!(e.layer_pixel(15, 15), RED);
    assert_eq!(count(&e, IRect::new(0, 0, 40, 40), RED), 100);
}

#[test]
fn invert_without_selection_selects_all() {
    let mut e = Editor::new(20, 20);
    e.invert_selection();
    assert_eq!(e.selection().unwrap().rect, IRect::new(0, 0, 20, 20));
}

#[test]
fn invert_of_floating_selection_commits_it_first() {
    let mut e = with_square(60, 40, IRect::new(0, 0, 10, 10));
    select(&mut e, (0.0, 0.0), (10.0, 10.0));
    drag(&mut e, (5.0, 5.0), (25.0, 5.0));
    e.invert_selection();
    assert_eq!(e.layer_pixel(20, 0), RED);
    assert!(!e.selection().unwrap().is_floating());
    assert!(!e.selection().unwrap().contains(22, 2));
}

#[test]
fn delete_fills_with_secondary_colour_and_is_undoable() {
    let mut e = with_square(40, 40, IRect::new(0, 0, 40, 40));
    e.set_color2(BLUE);
    select(&mut e, (5.0, 5.0), (15.0, 15.0));
    assert!(e.delete_selection());
    assert!(e.selection().is_none());
    assert_eq!(count(&e, IRect::new(0, 0, 40, 40), BLUE), 100);
    e.undo();
    assert_eq!(count(&e, IRect::new(0, 0, 40, 40), RED), 1600);
    assert!(e.selection().is_some(), "undo restores the selection");
    assert!(!Editor::new(5, 5).delete_selection());
}

#[test]
fn deleting_a_floating_selection_discards_it() {
    let mut e = with_square(40, 40, IRect::new(0, 0, 10, 10));
    select(&mut e, (0.0, 0.0), (10.0, 10.0));
    drag(&mut e, (5.0, 5.0), (25.0, 25.0));
    e.delete_selection();
    assert_eq!(count(&e, IRect::new(0, 0, 40, 40), RED), 0);
}

#[test]
fn transparent_selection_skips_secondary_colour() {
    // Left: white with a red dot. Right: blue.
    let mut img = Image::filled(100, 40, WHITE);
    img.fill_rect(IRect::new(50, 0, 50, 40), BLUE);
    img.set(12, 12, RED);
    for transparent in [false, true] {
        let mut e = Editor::from_image(img.clone());
        e.set_transparent_selection(transparent);
        select(&mut e, (10.0, 10.0), (20.0, 20.0));
        drag(&mut e, (15.0, 15.0), (65.0, 15.0));
        assert_eq!(e.composite_pixel(62, 12), RED);
        let expected = if transparent { BLUE } else { WHITE };
        assert_eq!(e.composite_pixel(60, 10), expected, "transparent={transparent}");
        e.deselect();
        assert_eq!(e.layer_pixel(60, 10), expected);
        assert_eq!(e.layer_pixel(62, 12), RED);
    }
}

#[test]
fn crop_to_rectangle_selection() {
    let mut img = Image::filled(50, 40, WHITE);
    img.set(10, 10, RED);
    img.set(29, 24, BLUE);
    let mut e = Editor::from_image(img);
    select(&mut e, (10.0, 10.0), (30.0, 25.0));
    assert!(e.crop());
    assert_eq!((e.width(), e.height()), (20, 15));
    assert_eq!(e.layer_pixel(0, 0), RED);
    assert_eq!(e.layer_pixel(19, 14), BLUE);
    assert!(e.selection().is_none());
    e.undo();
    assert_eq!((e.width(), e.height()), (50, 40));
    assert!(!e.crop() || e.width() == 20);
    assert!(!Editor::new(5, 5).crop());
}

#[test]
fn crop_to_freeform_selection_clears_outside() {
    let mut e = with_square(40, 40, IRect::new(0, 0, 40, 40));
    e.set_tool(Tool::FreeSelect);
    drag_with(&mut e, &[(0.0, 0.0), (20.0, 0.0), (0.0, 20.0)], L, NO);
    e.crop();
    assert_eq!((e.width(), e.height()), (20, 20));
    assert_eq!(e.layer_pixel(1, 1), RED);
    assert_eq!(e.layer_pixel(18, 18), WHITE);
}

#[test]
fn crop_includes_floating_pixels() {
    let mut e = with_square(60, 40, IRect::new(0, 0, 10, 10));
    select(&mut e, (0.0, 0.0), (10.0, 10.0));
    drag(&mut e, (5.0, 5.0), (35.0, 25.0));
    e.crop();
    assert_eq!((e.width(), e.height()), (10, 10));
    assert_eq!(count(&e, IRect::new(0, 0, 10, 10), RED), 100);
}

#[test]
fn arrow_keys_nudge_selection() {
    let mut e = with_square(40, 40, IRect::new(10, 10, 5, 5));
    select(&mut e, (10.0, 10.0), (15.0, 15.0));
    for _ in 0..3 {
        assert!(e.nudge(1, 0));
    }
    assert!(e.nudge(0, -2));
    assert_eq!(e.selection().unwrap().rect, IRect::new(13, 8, 5, 5));
    e.deselect();
    assert_eq!(e.layer_pixel(13, 8), RED);
    assert_eq!(e.layer_pixel(10, 14), WHITE);
    // One undo step covers all the nudges.
    e.undo();
    assert_eq!(e.layer_pixel(10, 14), RED);
    assert!(!Editor::new(5, 5).nudge(1, 1));
}

#[test]
fn rotate_and_flip_selection_only() {
    let mut img = Image::filled(60, 60, WHITE);
    img.fill_rect(IRect::new(10, 10, 20, 10), RED);
    img.set(10, 10, BLUE);
    let mut e = Editor::from_image(img);
    select(&mut e, (10.0, 10.0), (30.0, 20.0));
    e.transform(Transform::RotateCw);
    let s = e.selection().unwrap();
    assert!(s.is_floating());
    assert_eq!(s.rect, IRect::new(10, 10, 10, 20));
    // Top-left pixel goes to the top-right after a clockwise rotation.
    assert_eq!(e.composite_pixel(19, 10), BLUE);
    e.transform(Transform::FlipHorizontal);
    assert_eq!(e.composite_pixel(10, 10), BLUE);
    e.transform(Transform::FlipVertical);
    assert_eq!(e.composite_pixel(10, 29), BLUE);
    e.transform(Transform::Rotate180);
    assert_eq!(e.composite_pixel(19, 10), BLUE);
    e.transform(Transform::RotateCcw);
    assert_eq!(e.selection().unwrap().rect, IRect::new(10, 10, 20, 10));
    // The canvas did not change size.
    assert_eq!((e.width(), e.height()), (60, 60));
}

#[test]
fn resize_and_skew_selection() {
    let mut e = with_square(80, 80, IRect::new(0, 0, 10, 10));
    select(&mut e, (0.0, 0.0), (10.0, 10.0));
    e.transform(Transform::Resize(20, 5));
    assert_eq!(e.selection().unwrap().rect, IRect::new(0, 0, 20, 5));
    e.transform(Transform::Skew(45.0, 0.0));
    assert_eq!(e.selection().unwrap().rect.w, 25);
    e.deselect();
    assert_eq!(e.layer_pixel(0, 0), RED);
}

#[test]
fn freeform_outline_follows_transforms() {
    let mut e = with_square(40, 40, IRect::new(0, 0, 40, 40));
    e.set_tool(Tool::FreeSelect);
    drag_with(&mut e, &[(0.0, 0.0), (20.0, 0.0), (0.0, 10.0)], L, NO);
    e.transform(Transform::FlipHorizontal);
    let st = e.state().selection.unwrap();
    // The lasso started at the top-left corner, which a horizontal flip sends to the top-right.
    assert_eq!(st.outline[0][0], [(st.x + st.w) as f32, 0.0]);
    e.transform(Transform::Skew(10.0, 0.0));
    assert_eq!(e.state().selection.unwrap().outline[0].len(), 4, "skew falls back to the bounds");
}

#[test]
fn selection_on_upper_layer_leaves_transparency() {
    let mut e = Editor::new(40, 40);
    e.add_layer();
    e.set_tool(Tool::Pencil);
    e.set_size(4);
    drag(&mut e, (10.0, 10.0), (20.0, 10.0));
    let painted = e.layer_pixel(15, 10);
    assert_eq!(painted, BLACK);
    select(&mut e, (5.0, 5.0), (25.0, 15.0));
    drag(&mut e, (15.0, 10.0), (15.0, 30.0));
    assert_eq!(e.layer_pixel(15, 10), Rgba::TRANSPARENT);
    assert_eq!(e.composite_pixel(15, 10), WHITE);
    assert_eq!(e.composite_pixel(15, 30), BLACK);
    // The background layer is untouched.
    assert!(e.doc().layers[0].image.is_uniform(WHITE));
}

#[test]
fn undo_redo_of_selection_moves() {
    let mut e = with_square(60, 40, IRect::new(0, 0, 10, 10));
    select(&mut e, (0.0, 0.0), (10.0, 10.0));
    drag(&mut e, (5.0, 5.0), (25.0, 5.0));
    drag(&mut e, (25.0, 5.0), (45.0, 5.0));
    assert_eq!(e.selection().unwrap().rect.x, 40);
    e.undo();
    assert_eq!(e.selection().unwrap().rect.x, 20);
    assert!(e.selection().unwrap().is_floating());
    e.undo();
    assert!(!e.selection().unwrap().is_floating());
    assert_eq!(e.layer_pixel(0, 0), RED);
    e.redo();
    e.redo();
    assert_eq!(e.selection().unwrap().rect.x, 40);
    assert_eq!(e.composite_pixel(40, 0), RED);
}

#[test]
fn cursor_feedback() {
    let mut e = Editor::new(100, 100);
    e.set_tool(Tool::RectSelect);
    assert_eq!(e.cursor_at(50.0, 50.0), "crosshair");
    select(&mut e, (20.0, 20.0), (60.0, 60.0));
    assert_eq!(e.cursor_at(40.0, 40.0), "move");
    assert_eq!(e.cursor_at(60.0, 20.0), "nesw-resize");
    assert_eq!(e.cursor_at(20.0, 40.0), "ew-resize");
    e.pointer_down(40.0, 40.0, L, NO);
    assert_eq!(e.cursor_at(90.0, 90.0), "move");
    e.pointer_up(40.0, 40.0);
}

#[test]
fn right_click_on_selection_does_nothing() {
    let mut e = with_square(40, 40, IRect::new(0, 0, 10, 10));
    select(&mut e, (0.0, 0.0), (10.0, 10.0));
    e.pointer_down(5.0, 5.0, R, NO);
    e.pointer_up(5.0, 5.0);
    assert!(!e.selection().unwrap().is_floating());
    assert!(!e.can_undo());
}

#[test]
fn programmatic_select_rect() {
    let mut e = Editor::new(30, 30);
    e.select_rect(IRect::new(20, 20, 50, 50));
    assert_eq!(e.selection().unwrap().rect, IRect::new(20, 20, 10, 10));
    e.select_rect(IRect::new(40, 40, 5, 5));
    assert!(e.selection().is_none());
}
