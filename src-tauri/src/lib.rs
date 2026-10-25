//! Desktop shell of Paint: a Tauri window hosting the UI (`ui/`) and its
//! WebAssembly engine, plus native services the browser sandbox lacks:
//! the system clipboard (images), file access and the command-line file.

pub mod clipboard;
pub mod files;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            clipboard::clipboard_write_png,
            clipboard::clipboard_read_png,
            files::read_file,
            files::write_file,
            files::initial_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Paint");
}
