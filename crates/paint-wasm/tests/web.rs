//! The engine API exercised inside a real WebAssembly runtime (`wasm-pack test --node`).

use paint_wasm::Engine;
use wasm_bindgen_test::*;

fn state(e: &Engine) -> String {
    e.state_json()
}

#[wasm_bindgen_test]
fn draws_and_renders_into_wasm_memory() {
    let mut e = Engine::new(64, 48);
    assert_eq!(e.render(), vec![0, 0, 64, 48]);
    assert_eq!(e.frame_len(), 64 * 48 * 4);
    assert!(e.set_tool("pencil"));
    e.pointer_down(10.5, 10.5, 0, false, false, false);
    e.pointer_move(20.5, 10.5, false, false, false);
    e.pointer_up(20.5, 10.5);
    let r = e.render();
    assert_eq!(r.len(), 4);
    assert_eq!(e.pixel(15, 10), "#000000");
    let frame = unsafe { std::slice::from_raw_parts(e.frame_ptr(), e.frame_len()) };
    let i = (10 * 64 + 15) * 4;
    assert_eq!(&frame[i..i + 4], &[0, 0, 0, 255]);
    assert!(e.render().is_empty());
    e.invalidate();
    assert_eq!(e.render(), vec![0, 0, 64, 48]);
}

#[wasm_bindgen_test]
fn rejects_unknown_names() {
    let mut e = Engine::new(8, 8);
    assert!(!e.set_tool("laser"));
    assert!(!e.set_brush("chalk"));
    assert!(!e.set_shape("blob"));
    assert!(!e.set_color1("#zzzzzz"));
    assert!(!e.rotate(45));
    assert!(e.export("psd", 90).is_err());
    assert!(e.open_bytes(b"nope").is_err());
    assert!(e.paste_rgba(2, 2, vec![0; 3], 0, 0).is_err());
}

#[wasm_bindgen_test]
fn selection_move_and_clipboard() {
    let mut e = Engine::new(50, 50);
    e.set_tool("fill");
    e.set_color1("#ed1c24");
    e.select_rect(0, 0, 10, 10);
    e.deselect();
    e.pointer_down(1.0, 1.0, 0, false, false, false);
    e.pointer_up(1.0, 1.0);
    assert_eq!(e.pixel(49, 49), "#ed1c24");
    e.new_document(50, 50);
    // Draw a red square with the rectangle shape, filled.
    e.set_shape("rectangle");
    e.set_fill("solid");
    e.set_outline("none");
    e.set_color2("#ed1c24");
    e.pointer_down(0.0, 0.0, 0, false, false, false);
    e.pointer_move(9.0, 9.0, false, false, false);
    e.pointer_up(9.0, 9.0);
    e.enter();
    assert_eq!(e.layer_pixel(5, 5), "#ed1c24");
    e.set_color2("#ffffff");
    // Select and move it.
    e.set_tool("select-rect");
    e.pointer_down(0.0, 0.0, 0, false, false, false);
    e.pointer_move(10.0, 10.0, false, false, false);
    e.pointer_up(10.0, 10.0);
    assert!(state(&e).contains("\"selection\":{\"x\":0,\"y\":0,\"w\":10,\"h\":10"));
    let png = e.copy_png().expect("copy");
    e.pointer_down(5.0, 5.0, 0, false, false, false);
    e.pointer_move(25.0, 25.0, false, false, false);
    e.pointer_up(25.0, 25.0);
    assert!(state(&e).contains("\"floating\":true"));
    assert_eq!(e.pixel(25, 25), "#ed1c24");
    assert_eq!(e.pixel(5, 5), "#ffffff");
    // Paste the copied PNG back at the origin and commit.
    e.paste_bytes(&png, 0, 0).unwrap();
    e.deselect();
    assert_eq!(e.layer_pixel(5, 5), "#ed1c24");
    assert_eq!(e.layer_pixel(25, 25), "#ed1c24");
    // Cut leaves the secondary colour.
    e.select_rect(20, 20, 10, 10);
    assert!(e.cut_png().is_some());
    assert_eq!(e.layer_pixel(25, 25), "#ffffff");
    assert!(e.copy_png().is_none());
    assert!(e.undo());
    assert_eq!(e.layer_pixel(25, 25), "#ed1c24");
}

#[wasm_bindgen_test]
fn image_commands_and_layers() {
    let mut e = Engine::new(30, 20);
    assert!(e.rotate(90));
    assert_eq!((e.width(), e.height()), (20, 30));
    assert!(e.rotate(-90));
    e.flip_horizontal();
    e.flip_vertical();
    e.resize_image(60, 40);
    assert_eq!((e.width(), e.height()), (60, 40));
    e.skew(0.0, 10.0);
    assert!(e.height() > 40);
    e.resize_canvas(10, 10);
    assert_eq!((e.width(), e.height()), (10, 10));
    e.add_layer();
    assert!(state(&e).contains("\"active_layer\":1"));
    assert!(e.rename_layer(1, "Encre"));
    assert!(e.set_layer_visible(1, false));
    assert!(e.duplicate_layer(1));
    assert!(e.move_layer(2, 0));
    assert!(e.set_active_layer(2));
    assert!(e.merge_down(2));
    assert!(e.delete_layer(1));
    assert!(e.layer_thumbnail_png(0, 16).is_some());
    assert!(e.layer_thumbnail_png(5, 16).is_none());
    assert!(e.undo() && e.redo());
}

#[wasm_bindgen_test]
fn files_text_and_settings() {
    let mut e = Engine::new(16, 16);
    e.stamp_rgba(2, 2, 2, 2, [0, 0, 255, 255].repeat(4)).unwrap();
    assert_eq!(e.pixel(3, 3), "#0000ff");
    assert!(e.is_modified());
    for fmt in ["png", "jpg", "bmp", "gif", "tiff", "webp", "ico"] {
        let bytes = e.export(fmt, 90).unwrap();
        let mut other = Engine::new(1, 1);
        other.open_bytes(&bytes).unwrap();
        assert_eq!((other.width(), other.height()), (16, 16), "{fmt}");
    }
    e.mark_saved();
    assert!(!e.is_modified());
    e.set_size(7);
    e.set_zoom(4.0);
    e.set_transparent_selection(true);
    e.set_fill_tolerance(12);
    e.swap_colors();
    let s = state(&e);
    assert!(s.contains("\"size\":7") && s.contains("\"transparent_selection\":true") && s.contains("\"color1\":\"#ffffff\""));
    assert!(e.set_brush("watercolor"));
    assert_eq!(e.cursor_at(1.0, 1.0), "brush");
    e.pointer_down(8.0, 8.0, 2, false, false, false);
    e.tick();
    e.pointer_up(8.0, 8.0);
    e.select_all();
    e.invert_selection();
    assert!(!e.delete_selection(), "the inverse of everything selects nothing");
    e.select_rect(0, 0, 4, 4);
    e.invert_selection();
    assert!(e.delete_selection());
    assert!(!paint_wasm::Engine::size_ok(20_000, 10) && paint_wasm::Engine::size_ok(16, 16));
    assert!(!e.resize_image(16384, 16384), "over the memory budget");
    e.select_all();
    assert!(e.nudge(1, 1));
    e.escape();
    assert!(e.remove_background(10));
    e.select_rect(0, 0, 4, 4);
    assert!(e.crop());
    assert_eq!(e.width(), 4);
    e.double_click(0.0, 0.0);
    assert!(!paint_wasm::version().is_empty());
}
