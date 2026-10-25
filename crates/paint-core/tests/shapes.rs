//! Shapes: drawing, styles, colours, live editing (move/resize/handles), curve and
//! polygon stages, Shift constraint, cancel and commit.

mod common;
use common::*;
use paint_core::*;

fn shape(e: &mut Editor, kind: ShapeKind) {
    e.set_shape(kind);
    assert_eq!(e.tool(), Tool::Shape);
}

#[test]
fn every_shape_draws_inside_its_box() {
    for kind in ShapeKind::ALL {
        let mut e = Editor::new(100, 80);
        shape(&mut e, kind);
        e.set_color1(BLUE);
        e.set_color2(GREEN);
        e.set_fill(FillStyle::Solid);
        if kind == ShapeKind::Polygon {
            drag(&mut e, (20.0, 20.0), (80.0, 20.0));
            click(&mut e, 50.0, 60.0);
            click(&mut e, 20.0, 20.0);
        } else {
            drag(&mut e, (20.0, 20.0), (80.0, 60.0));
        }
        if kind == ShapeKind::Curve {
            assert_eq!(e.state().shape_edit.unwrap().stage, "bend1");
        }
        e.enter();
        assert!(!e.has_pending_shape(), "{kind:?}");
        let outline = count(&e, IRect::new(0, 0, 100, 80), BLUE);
        assert!(outline > 20, "{kind:?}: {outline} outline pixels");
        // Nothing drawn far outside the box.
        for (x, y) in [(5, 5), (95, 75), (10, 40), (90, 40), (50, 5), (50, 75)] {
            assert_eq!(e.composite_pixel(x, y), WHITE, "{kind:?} leaked at {x},{y}");
        }
        if !kind.is_open() {
            assert!(count(&e, IRect::new(0, 0, 100, 80), GREEN) > 50, "{kind:?} not filled");
        }
        e.undo();
        assert_eq!(count_not(&e, WHITE), 0, "{kind:?} undo");
    }
}

#[test]
fn crisp_one_pixel_rectangle() {
    let mut e = Editor::new(40, 30);
    shape(&mut e, ShapeKind::Rectangle);
    e.set_size(1);
    drag(&mut e, (10.0, 10.0), (20.0, 15.0));
    e.enter();
    assert_eq!(e.layer_pixel(10, 10), BLACK);
    assert_eq!(e.layer_pixel(20, 15), BLACK);
    assert_eq!(e.layer_pixel(15, 10), BLACK);
    assert_eq!(e.layer_pixel(15, 12), WHITE);
    assert_eq!(e.layer_pixel(21, 10), WHITE);
    assert_eq!(count(&e, IRect::new(0, 0, 40, 30), BLACK), 2 * 11 + 2 * 4);
}

#[test]
fn fill_without_outline_uses_secondary_colour() {
    let mut e = Editor::new(40, 30);
    shape(&mut e, ShapeKind::Rectangle);
    e.set_outline(OutlineStyle::None);
    e.set_fill(FillStyle::Solid);
    e.set_color2(GREEN);
    drag(&mut e, (10.0, 10.0), (19.0, 14.0));
    e.enter();
    assert_eq!(count(&e, IRect::new(0, 0, 40, 30), GREEN), 50);
    let st = e.state();
    assert_eq!((st.outline, st.fill), ("none", "solid"));
}

#[test]
fn right_drag_swaps_colours() {
    let mut e = Editor::new(40, 30);
    shape(&mut e, ShapeKind::Rectangle);
    e.set_fill(FillStyle::Solid);
    e.set_color1(BLUE);
    e.set_color2(GREEN);
    drag_with(&mut e, &[(5.0, 5.0), (30.0, 25.0)], R, NO);
    e.enter();
    assert_eq!(e.layer_pixel(5, 5), GREEN, "outline in colour 2");
    assert_eq!(e.layer_pixel(15, 15), BLUE, "fill in colour 1");
}

#[test]
fn shift_draws_squares_and_45_degree_lines() {
    let mut e = Editor::new(60, 60);
    shape(&mut e, ShapeKind::Rectangle);
    drag_with(&mut e, &[(10.0, 10.0), (40.0, 20.0)], L, SHIFT);
    let s = e.state().shape_edit.unwrap();
    assert_eq!((s.w, s.h), (30.0, 30.0));
    e.enter();
    shape(&mut e, ShapeKind::Line);
    drag_with(&mut e, &[(5.0, 50.0), (40.0, 52.0)], L, SHIFT);
    let s = e.state().shape_edit.unwrap();
    assert!(s.h.abs() < 0.01, "horizontal line: {s:?}");
}

#[test]
fn pending_shape_follows_colour_size_and_style_changes() {
    let mut e = Editor::new(60, 40);
    shape(&mut e, ShapeKind::Rectangle);
    drag(&mut e, (10.0, 10.0), (50.0, 30.0));
    assert!(e.has_pending_shape());
    assert_eq!(e.layer_pixel(10, 10), BLACK);
    e.set_color1(RED);
    assert_eq!(e.layer_pixel(10, 10), RED);
    e.set_size(1);
    assert_eq!(e.layer_pixel(11, 11), WHITE);
    e.set_size(5);
    assert_eq!(e.layer_pixel(11, 11), RED);
    e.set_fill(FillStyle::Solid);
    e.set_color2(GREEN);
    assert_eq!(e.layer_pixel(30, 20), GREEN);
    e.set_outline(OutlineStyle::None);
    assert_eq!(e.layer_pixel(10, 10), GREEN);
    // One undo step for the whole shape.
    e.undo();
    assert_eq!(count_not(&e, WHITE), 0);
    assert!(!e.can_undo());
}

#[test]
fn pending_shape_can_be_moved_and_resized() {
    let mut e = Editor::new(100, 80);
    shape(&mut e, ShapeKind::Rectangle);
    e.set_size(1);
    drag(&mut e, (10.0, 10.0), (30.0, 30.0));
    let st = e.state().shape_edit.unwrap();
    assert_eq!(st.stage, "editing");
    assert_eq!(st.handles.len(), 8);
    assert_eq!(e.cursor_at(20.0, 20.0), "move");
    assert_eq!(e.cursor_at(30.0, 30.0), "nwse-resize");
    // Move.
    drag(&mut e, (20.0, 20.0), (60.0, 40.0));
    assert_eq!(e.layer_pixel(10, 10), WHITE);
    assert_eq!(e.layer_pixel(50, 30), BLACK);
    // Resize with the bottom-right handle.
    drag(&mut e, (70.0, 50.0), (90.0, 70.0));
    assert_eq!(e.layer_pixel(90, 70), BLACK);
    assert_eq!(e.layer_pixel(70, 50), WHITE);
    let st = e.state().shape_edit.unwrap();
    assert_eq!((st.x, st.y, st.w, st.h), (50.0, 30.0, 40.0, 40.0));
    // Switching tools commits; the pixels stay.
    e.set_tool(Tool::Pencil);
    assert!(!e.has_pending_shape());
    assert_eq!(e.layer_pixel(90, 70), BLACK);
    e.undo();
    assert_eq!(count_not(&e, WHITE), 0);
}

#[test]
fn clicking_outside_commits_and_starts_a_new_shape() {
    let mut e = Editor::new(100, 50);
    shape(&mut e, ShapeKind::Oval);
    drag(&mut e, (5.0, 5.0), (30.0, 30.0));
    drag(&mut e, (60.0, 10.0), (90.0, 40.0));
    assert!(e.has_pending_shape());
    e.enter();
    e.undo();
    assert!(count(&e, IRect::new(0, 0, 50, 50), BLACK) > 20, "first shape stays");
    assert_eq!(count(&e, IRect::new(50, 0, 50, 50), BLACK), 0);
}

#[test]
fn a_click_without_drag_draws_nothing() {
    let mut e = Editor::new(40, 40);
    shape(&mut e, ShapeKind::Rectangle);
    click(&mut e, 10.0, 10.0);
    assert!(!e.has_pending_shape());
    assert!(!e.can_undo());
    assert!(!e.is_modified());
    assert_eq!(count_not(&e, WHITE), 0);
}

#[test]
fn line_endpoints_are_editable() {
    let mut e = Editor::new(60, 60);
    shape(&mut e, ShapeKind::Line);
    drag(&mut e, (10.0, 10.0), (50.0, 10.0));
    assert_eq!(e.layer_pixel(30, 10), BLACK);
    let st = e.state().shape_edit.unwrap();
    assert_eq!(st.handles.len(), 2);
    assert_eq!(e.cursor_at(50.0, 10.0), "pointer");
    drag(&mut e, (50.0, 10.0), (10.0, 50.0));
    assert_eq!(e.layer_pixel(30, 10), WHITE);
    assert_eq!(e.layer_pixel(10, 30), BLACK);
    // Dragging the line body moves it.
    drag(&mut e, (10.0, 30.0), (40.0, 30.0));
    assert_eq!(e.layer_pixel(10, 30), WHITE);
    assert_eq!(e.layer_pixel(40, 30), BLACK);
}

#[test]
fn curve_is_bent_twice() {
    let mut e = Editor::new(100, 60);
    shape(&mut e, ShapeKind::Curve);
    e.set_size(2);
    drag(&mut e, (10.0, 50.0), (90.0, 50.0));
    assert_eq!(e.state().shape_edit.unwrap().stage, "bend1");
    assert_eq!(e.layer_pixel(50, 50), BLACK);
    drag(&mut e, (50.0, 30.0), (50.0, 0.0));
    assert_eq!(e.state().shape_edit.unwrap().stage, "bend2");
    assert_eq!(e.layer_pixel(50, 50), WHITE, "the curve left the straight line");
    click(&mut e, 50.0, 0.0);
    let st = e.state().shape_edit.unwrap();
    assert_eq!(st.stage, "editing");
    assert_eq!(st.handles.len(), 4);
    let top = (0..50).find(|y| e.layer_pixel(50, *y) == BLACK).expect("curve apex");
    assert!(top < 20, "apex at {top}");
    e.enter();
    assert!(!e.has_pending_shape());
}

#[test]
fn polygon_by_clicks_closed_on_first_point() {
    let mut e = Editor::new(80, 80);
    shape(&mut e, ShapeKind::Polygon);
    e.set_fill(FillStyle::Solid);
    e.set_color2(GREEN);
    drag(&mut e, (10.0, 10.0), (70.0, 10.0));
    assert_eq!(e.state().shape_edit.unwrap().stage, "polygon");
    click(&mut e, 70.0, 70.0);
    click(&mut e, 10.0, 70.0);
    assert_eq!(e.layer_pixel(40, 40), WHITE, "open polygon is not filled");
    click(&mut e, 11.0, 11.0);
    let st = e.state().shape_edit.unwrap();
    assert_eq!(st.stage, "editing");
    assert_eq!(e.layer_pixel(40, 40), GREEN);
    assert_eq!(st.points.len(), 4);
}

#[test]
fn polygon_closed_by_double_click_and_enter() {
    let mut e = Editor::new(80, 80);
    shape(&mut e, ShapeKind::Polygon);
    e.set_fill(FillStyle::Solid);
    e.set_color2(GREEN);
    drag(&mut e, (10.0, 10.0), (70.0, 10.0));
    click(&mut e, 40.0, 70.0);
    click(&mut e, 40.0, 70.0);
    e.double_click(40.0, 70.0);
    let st = e.state().shape_edit.unwrap();
    assert_eq!(st.stage, "editing");
    assert_eq!(st.points.len(), 3);
    assert_eq!(e.layer_pixel(40, 30), GREEN);
    e.undo();
    // Enter closes an open polygon too.
    drag(&mut e, (10.0, 10.0), (70.0, 10.0));
    click(&mut e, 40.0, 70.0);
    e.enter();
    assert!(!e.has_pending_shape());
    assert_eq!(e.layer_pixel(40, 30), GREEN);
}

#[test]
fn polygon_segment_shift_constraint() {
    let mut e = Editor::new(80, 80);
    shape(&mut e, ShapeKind::Polygon);
    drag(&mut e, (10.0, 10.0), (50.0, 10.0));
    e.pointer_down(52.0, 40.0, L, NO);
    e.pointer_move(51.0, 50.0, SHIFT);
    e.pointer_up(51.0, 50.0);
    let p = e.state().shape_edit.unwrap().points;
    assert!((p[2][0] - 50.0).abs() < 0.01, "vertical segment: {p:?}");
}

#[test]
fn changing_shape_kind_commits_the_pending_one() {
    let mut e = Editor::new(60, 60);
    shape(&mut e, ShapeKind::Star5);
    drag(&mut e, (5.0, 5.0), (50.0, 50.0));
    shape(&mut e, ShapeKind::Heart);
    assert!(!e.has_pending_shape());
    assert_eq!(e.state().shape, "heart");
    assert!(count_not(&e, WHITE) > 50);
}

#[test]
fn shape_names() {
    for k in ShapeKind::ALL {
        assert_eq!(ShapeKind::from_name(k.name()), Some(k));
    }
}
