//! Regression tests for bugs found by the code review and the exploratory QA
//! (each one failed before its fix).

mod common;
use common::*;
use paint_core::*;

fn drag(e: &mut Editor, pts: &[(f32, f32)]) {
    drag_with(e, pts, L, NO);
}

// ---------------------------------------------------------------------------
// Skew interpolates straight (non-premultiplied) RGBA: dark fringes.
// ---------------------------------------------------------------------------
#[test]
fn skew_does_not_darken_edges_next_to_transparency() {
    // Upper layer: red square on transparency.
    let mut e = Editor::new(20, 20);
    e.add_layer();
    e.paste(Image::filled(8, 8, RED), 6, 6);
    e.deselect(); // stamped on layer 1
    e.transform(Transform::Skew(30.0, 0.0));
    let layer = &e.doc().layers[1].image;
    let mut worst = 255u8;
    for y in 0..layer.height() as i32 {
        for x in 0..layer.width() as i32 {
            let p = layer.get(x, y);
            if p.a > 0 && p.a < 255 {
                worst = worst.min(p.r);
            }
        }
    }
    // Every partially transparent edge pixel is still pure red (237) in a
    // premultiplied resampler; here it is mixed with the black of (0,0,0,0).
    assert!(worst >= 230, "edge pixel darkened to r={worst} (expected ~237)");
}

// ---------------------------------------------------------------------------
// Skew results are not limited to MAX_SIDE (dialog allows +-89 degrees).
// ---------------------------------------------------------------------------
#[test]
fn skew_result_respects_max_side() {
    let mut e = Editor::new(1152, 648); // default document size of the app
    e.transform(Transform::Skew(89.0, 0.0));
    assert!(e.width() <= codec::MAX_SIDE, "skewed canvas is {}x{} (> MAX_SIDE {})", e.width(), e.height(), codec::MAX_SIDE);
}

// ---------------------------------------------------------------------------
// A click without drag with the shape tool wipes the redo stack.
// ---------------------------------------------------------------------------
#[test]
fn shape_click_without_drag_keeps_redo() {
    let mut e = Editor::new(40, 40);
    e.set_shape(ShapeKind::Line);
    drag(&mut e, &[(5.0, 5.0), (30.0, 30.0)]);
    e.enter();
    assert!(e.undo());
    assert!(e.can_redo());
    // Accidental click: draws nothing (shape cancelled)...
    e.pointer_down(10.0, 10.0, L, NO);
    e.pointer_up(10.0, 10.0);
    assert!(!e.has_pending_shape());
    // ...but the redo history is gone.
    assert!(e.can_redo(), "a cancelled (no-op) shape click destroyed the redo stack");
}

#[test]
fn failed_remove_background_keeps_redo() {
    let mut e = Editor::new(10, 10);
    e.add_layer(); // transparent layer 1, active
    e.set_tool(Tool::Pencil);
    drag(&mut e, &[(2.0, 2.0), (8.0, 2.0)]);
    assert!(e.undo());
    assert!(e.can_redo());
    assert!(!e.remove_background(32), "nothing to remove on a transparent layer");
    assert!(e.can_redo(), "a remove-background that changed nothing destroyed the redo stack");
}

// ---------------------------------------------------------------------------
// Pending shape: collapsing it to zero width makes it impossible to re-expand.
// ---------------------------------------------------------------------------
#[test]
fn shape_collapsed_to_zero_width_can_be_reexpanded() {
    let mut e = Editor::new(60, 60);
    e.set_shape(ShapeKind::Rectangle);
    drag(&mut e, &[(10.0, 10.0), (30.0, 30.0)]);
    // Drag the E handle onto the W edge.
    drag(&mut e, &[(30.0, 20.0), (10.0, 20.0)]);
    let s = e.state().shape_edit.unwrap();
    assert_eq!(s.w, 0.0);
    // Drag it back out to x = 40.
    drag(&mut e, &[(10.0, 20.0), (40.0, 20.0)]);
    let s = e.state().shape_edit.unwrap();
    assert!(s.w > 0.0, "shape stays 0 px wide after dragging its E handle to x=40: {s:?}");
}

// ---------------------------------------------------------------------------
// `modified` is not set by edits of a pending shape / floating selection.
// After Ctrl+S (mark_saved) these edits are lost on close without a prompt.
// ---------------------------------------------------------------------------
#[test]
fn editing_pending_shape_after_save_marks_modified() {
    let mut e = Editor::new(60, 60);
    e.set_shape(ShapeKind::Rectangle);
    drag(&mut e, &[(10.0, 10.0), (30.0, 30.0)]);
    let saved = e.flatten();
    e.mark_saved(); // what App.write() does after export (shape still pending)
    // Move the still-editable shape.
    drag(&mut e, &[(20.0, 20.0), (40.0, 40.0)]);
    assert_ne!(e.flatten(), saved, "picture changed");
    assert!(e.is_modified(), "moved pending shape after save, but is_modified() == false");
}

#[test]
fn recolouring_pending_shape_after_save_marks_modified() {
    let mut e = Editor::new(60, 60);
    e.set_shape(ShapeKind::Rectangle);
    drag(&mut e, &[(10.0, 10.0), (30.0, 30.0)]);
    e.mark_saved();
    let saved = e.flatten();
    e.set_color1(RED);
    assert_ne!(e.flatten(), saved);
    assert!(e.is_modified(), "recoloured pending shape after save, but is_modified() == false");
}

#[test]
fn nudging_floating_selection_after_save_marks_modified() {
    let mut e = Editor::new(60, 60);
    e.paste(Image::filled(10, 10, RED), 5, 5);
    e.mark_saved();
    let saved = e.flatten();
    assert!(e.nudge(10, 0));
    assert_ne!(e.flatten(), saved);
    assert!(e.is_modified(), "nudged floating selection after save, but is_modified() == false");
}

// ---------------------------------------------------------------------------
// Floating free-form selection after Image > Resize (or Skew): the mask gets
// partial coverage; the preview thresholds at 128 but committing blends by
// coverage, so the picture changes when the selection is deselected.
// ---------------------------------------------------------------------------
#[test]
fn committing_resized_freeform_selection_matches_preview() {
    let mut e = with_square(60, 60, IRect::new(10, 10, 30, 30));
    e.set_tool(Tool::FreeSelect);
    drag(&mut e, &[(5.0, 25.0), (25.0, 5.0), (45.0, 25.0), (25.0, 45.0), (5.0, 25.0)]);
    assert!(e.selection().is_some_and(|s| s.is_freeform()));
    e.transform(Transform::Resize(27, 23));
    let m = e.selection().unwrap().current_mask().unwrap();
    let partial = m.data().iter().filter(|v| **v > 0 && **v < 255).count();
    let preview = e.flatten();
    e.deselect();
    let committed = e.flatten();
    let mut diff = 0;
    for y in 0..60 {
        for x in 0..60 {
            if preview.get(x, y) != committed.get(x, y) {
                diff += 1;
            }
        }
    }
    assert_eq!(diff, 0, "{diff} pixels change on commit ({partial} partial-coverage mask pixels)");
}

// ---------------------------------------------------------------------------
// Remove background with a free-form selection ignores the lasso shape.
// ---------------------------------------------------------------------------
#[test]
fn remove_background_respects_freeform_selection() {
    let mut e = Editor::new(20, 20);
    e.set_tool(Tool::FreeSelect);
    // Triangle covering the upper-left half.
    drag(&mut e, &[(0.0, 0.0), (20.0, 0.0), (0.0, 20.0), (0.0, 0.0)]);
    let sel = e.selection().unwrap().clone();
    assert!(!sel.contains(18, 18));
    assert!(e.remove_background(32));
    assert_eq!(e.layer_pixel(2, 2).a, 0); // inside the lasso: removed
    assert_eq!(e.layer_pixel(18, 18), Rgba::WHITE, "pixel outside the lasso was made transparent");
}

// ---------------------------------------------------------------------------
// Invert of an inverted selection / of Select All.
// ---------------------------------------------------------------------------
#[test]
fn double_invert_then_crop_crops_to_original_rect() {
    let mut e = with_square(40, 30, IRect::new(10, 10, 20, 10));
    e.select_rect(IRect::new(10, 10, 20, 10));
    e.invert_selection();
    e.invert_selection();
    assert!(e.crop());
    assert_eq!((e.width(), e.height()), (20, 10), "crop after double invert");
}

#[test]
fn double_invert_then_copy_returns_selection_sized_image() {
    let mut e = with_square(40, 30, IRect::new(10, 10, 20, 10));
    e.select_rect(IRect::new(10, 10, 20, 10));
    e.invert_selection();
    e.invert_selection();
    let img = e.copy().unwrap();
    assert_eq!((img.width(), img.height()), (20, 10), "copy after double invert");
}

#[test]
fn invert_of_select_all_then_crop_keeps_picture() {
    let mut e = with_square(40, 30, IRect::new(10, 10, 20, 10));
    e.set_color2(Rgba::rgb(0, 0, 255));
    e.select_all();
    e.invert_selection(); // nothing selected any more
    // An empty selection should not exist, or crop should be refused.
    let cropped = e.crop();
    assert!(
        !cropped || e.layer_pixel(15, 15) == RED,
        "crop of the (empty) inverse of Select All wiped the picture: {:?}",
        e.layer_pixel(15, 15)
    );
}

// ---------------------------------------------------------------------------
// Ctrl+click on a floating selection stamps a copy but does not mark the area
// dirty: the frame shows a stale picture until something else repaints it.
// ---------------------------------------------------------------------------
#[test]
fn ctrl_click_stamp_refreshes_frame() {
    let mut e = Editor::new(30, 30);
    e.render();
    e.paste(Image::filled(10, 10, Rgba::new(255, 0, 0, 128)), 5, 5);
    e.render();
    let ctrl = Mods { shift: false, ctrl: true, alt: false };
    e.pointer_down(8.0, 8.0, L, ctrl); // stamps a copy in place, starts a move
    e.pointer_up(8.0, 8.0);
    e.render();
    assert_eq!(e.frame().get(8, 8), e.flatten().get(8, 8), "frame not repainted after Ctrl+click stamp");
}

// ---------------------------------------------------------------------------
// A plain click inside a floating selection (no move) records an undo step
// that changes nothing: the next Ctrl+Z appears to do nothing.
// ---------------------------------------------------------------------------
#[test]
fn click_inside_floating_selection_adds_no_empty_undo_step() {
    let mut e = Editor::new(30, 30);
    e.paste(Image::filled(10, 10, RED), 5, 5);
    e.pointer_down(8.0, 8.0, L, NO);
    e.pointer_up(8.0, 8.0);
    assert!(e.undo());
    assert!(e.selection().is_none(), "first Ctrl+Z after a plain click did nothing visible (paste still there)");
}

// ---------------------------------------------------------------------------
// Hiding the active layer does not hide its floating selection; deselecting
// then makes those pixels vanish (preview != result).
// ---------------------------------------------------------------------------
#[test]
fn hidden_layer_hides_its_floating_selection() {
    let mut e = Editor::new(30, 30);
    e.add_layer();
    e.paste(Image::filled(10, 10, RED), 5, 5);
    e.set_layer_visible(1, false);
    let shown = e.composite_pixel(8, 8);
    e.deselect();
    let after = e.composite_pixel(8, 8);
    assert_eq!(shown, after, "floating pixels shown on a hidden layer, then vanish on deselect");
}


fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

/// PNG whose IHDR declares `w`x`h` RGBA8 (pixel data irrelevant: the decoder stops at the header).
fn png_header_only(w: u32, h: u32) -> Vec<u8> {
    let mut v = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut v, b"IHDR", &ihdr);
    chunk(&mut v, b"IDAT", &[0x78, 0x9c, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01]);
    chunk(&mut v, b"IEND", &[]);
    v
}

#[test]
fn image_within_max_side_is_not_rejected_by_hidden_limit() {
    // 16000 x 4000 (64 Mpx budget, sides <= MAX_SIDE): the image crate's own 512 MiB cap must not apply.
    let bytes = png_header_only(16000, 4000);
    let err = codec::decode(&bytes).err().unwrap_or_default();
    assert!(!err.to_lowercase().contains("limit"), "decode refused a 16000x4000 image: {err}");
    // Beyond the budget: refused up front with a clear message, without decoding.
    let err = codec::decode(&png_header_only(12000, 12000)).unwrap_err();
    assert!(err.contains("too large"), "{err}");
}

/// Inserts an EXIF APP1 segment with the given Orientation right after SOI.
fn with_exif_orientation(jpeg: &[u8], orientation: u16) -> Vec<u8> {
    let mut tiff = b"II*\0".to_vec();
    tiff.extend_from_slice(&8u32.to_le_bytes());
    tiff.extend_from_slice(&1u16.to_le_bytes());
    tiff.extend_from_slice(&0x0112u16.to_le_bytes());
    tiff.extend_from_slice(&3u16.to_le_bytes());
    tiff.extend_from_slice(&1u32.to_le_bytes());
    tiff.extend_from_slice(&orientation.to_le_bytes());
    tiff.extend_from_slice(&0u16.to_le_bytes());
    tiff.extend_from_slice(&0u32.to_le_bytes());
    let mut payload = b"Exif\0\0".to_vec();
    payload.extend_from_slice(&tiff);
    let mut out = jpeg[..2].to_vec();
    out.extend_from_slice(&[0xff, 0xe1]);
    out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&payload);
    out.extend_from_slice(&jpeg[2..]);
    out
}

#[test]
fn jpeg_exif_orientation_is_applied() {
    // A camera photo stored landscape (32x16) with Orientation=6 (display rotated 90° CW).
    let mut img = Image::filled(32, 16, Rgba::WHITE);
    img.fill_rect(paint_core::IRect::new(0, 0, 8, 16), Rgba::rgb(255, 0, 0));
    let jpeg = codec::encode(&img, Format::Jpeg, 95).unwrap();
    let photo = with_exif_orientation(&jpeg, 6);
    let back = codec::decode(&photo).unwrap();
    assert_eq!((back.width(), back.height()), (16, 32), "EXIF orientation ignored: phone photos open sideways");
}

// ---------------------------------------------------------------------------
// Exploratory QA findings.
// ---------------------------------------------------------------------------
#[test]
fn huge_resize_or_skew_is_refused_instead_of_crashing() {
    let mut e = Editor::new(1152, 648);
    assert!(!e.transform(Transform::Resize(16384, 9216)), "over 64 megapixels");
    assert!(!e.transform(Transform::Skew(89.0, 89.0)));
    assert_eq!((e.width(), e.height()), (1152, 648));
    assert!(!e.can_undo(), "a refused command leaves no history entry");
    // Within the limits it still works.
    assert!(e.transform(Transform::Resize(4000, 2250)));
    assert_eq!((e.width(), e.height()), (4000, 2250));
    // Selections are checked too.
    e.select_rect(IRect::new(0, 0, 100, 100));
    assert!(!e.transform(Transform::Resize(16000, 16000)));
    assert_eq!(e.selection().unwrap().rect.w, 100);
}

#[test]
fn paste_beyond_the_limits_is_refused() {
    let mut e = Editor::new(100, 100);
    assert!(!e.paste(Image::new(9000, 9000), 0, 0));
    assert!(!e.paste(Image::new(0, 0), 0, 0));
    assert_eq!((e.width(), e.height()), (100, 100));
    assert!(e.selection().is_none());
    assert!(e.paste(Image::new(10, 10), 0, 0));
}

#[test]
fn new_document_shrinks_to_the_pixel_budget() {
    let mut e = Editor::new(10, 10);
    e.new_document(16384, 16384);
    assert!(codec::size_ok(e.width(), e.height()));
    assert!(e.width() > 7000);
}

#[test]
fn large_downscale_stays_correct() {
    let mut img = Image::filled(3000, 2000, WHITE);
    img.fill_rect(IRect::new(0, 0, 1500, 2000), RED);
    let small = img.resized(30, 20);
    assert_eq!(small.get(2, 10), RED);
    assert_eq!(small.get(27, 10), WHITE);
}

#[test]
fn delete_removes_an_editable_shape_and_arrows_move_it() {
    let mut e = Editor::new(60, 60);
    e.set_shape(ShapeKind::Rectangle);
    drag(&mut e, &[(10.0, 10.0), (30.0, 30.0)]);
    assert!(e.nudge(5, 0));
    assert_eq!(e.layer_pixel(15, 10), BLACK);
    assert_eq!(e.layer_pixel(10, 10), WHITE);
    assert!(e.delete_selection());
    assert!(!e.has_pending_shape());
    assert_eq!(count_not(&e, WHITE), 0);
    assert!(!e.can_undo());
}

#[test]
fn select_all_then_rotate_turns_the_whole_canvas() {
    let mut e = with_square(40, 20, IRect::new(0, 0, 5, 5));
    e.select_all();
    assert!(e.transform(Transform::RotateCw));
    assert_eq!((e.width(), e.height()), (20, 40));
    assert!(e.selection().is_none());
    assert_eq!(e.layer_pixel(19, 0), RED);
}

#[test]
fn dragging_a_handle_past_the_opposite_edge_mirrors_the_selection() {
    let mut img = Image::filled(60, 40, WHITE);
    img.fill_rect(IRect::new(20, 10, 10, 10), RED);
    img.set(20, 10, BLUE);
    let mut e = Editor::from_image(img);
    e.set_tool(Tool::RectSelect);
    drag(&mut e, &[(20.0, 10.0), (30.0, 20.0)]);
    // East handle dragged 20 px to the left: the selection now spans 10..20, mirrored.
    drag(&mut e, &[(30.0, 15.0), (10.0, 15.0)]);
    let s = e.selection().unwrap();
    assert_eq!(s.rect, IRect::new(10, 10, 10, 10));
    assert_eq!(e.composite_pixel(19, 10), BLUE, "the left edge became the right edge");
    assert_eq!(e.composite_pixel(10, 10), RED);
}

#[test]
fn merged_layer_stays_visible_when_lower_was_hidden() {
    let mut e = Editor::new(10, 10);
    e.add_layer();
    e.paste(Image::filled(4, 4, RED), 0, 0);
    e.deselect();
    e.set_layer_visible(0, false);
    e.merge_down(1);
    assert!(e.doc().layers[0].visible);
    assert_eq!(e.composite_pixel(1, 1), RED);
}
