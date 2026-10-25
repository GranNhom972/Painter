//! System clipboard for images. The UI exchanges PNG bytes with these commands.
//!
//! * Linux: the GTK clipboard of the window (works on X11 and Wayland, offers
//!   every format gdk-pixbuf can write, and is handed to the clipboard manager
//!   so the image survives after Paint exits). The commands are synchronous so
//!   Tauri runs them on the GTK main thread.
//! * Windows / macOS: `arboard`.

use paint_core::{codec, Format, Image};
use tauri::ipc::{InvokeBody, Request, Response};

/// Decodes PNG (or any supported format) bytes from the UI.
pub fn decode_png(bytes: &[u8]) -> Result<Image, String> {
    codec::decode(bytes)
}

pub fn encode_png(img: &Image) -> Result<Vec<u8>, String> {
    codec::encode(img, Format::Png, 100)
}

#[cfg(target_os = "linux")]
pub mod gtk_backend {
    use gtk::gdk_pixbuf::{Colorspace, Pixbuf};
    use gtk::glib::Bytes;
    use paint_core::Image;

    fn clipboard() -> gtk::Clipboard {
        gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD)
    }

    pub fn image_to_pixbuf(img: &Image) -> Pixbuf {
        let (w, h) = (img.width() as i32, img.height() as i32);
        Pixbuf::from_bytes(&Bytes::from(img.data()), Colorspace::Rgb, true, 8, w, h, w * 4)
    }

    pub fn pixbuf_to_image(p: &Pixbuf) -> Result<Image, String> {
        let (w, h) = (p.width() as usize, p.height() as usize);
        let channels = p.n_channels() as usize;
        let stride = p.rowstride() as usize;
        let bytes = p.read_pixel_bytes();
        let src: &[u8] = &bytes;
        let mut out = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let i = y * stride + x * channels;
                out.extend_from_slice(&src[i..i + 3]);
                out.push(if channels == 4 { src[i + 3] } else { 255 });
            }
        }
        Image::from_rgba(w as u32, h as u32, out)
    }

    /// Must run on the GTK main thread.
    pub fn set_image(img: &Image) {
        let cb = clipboard();
        cb.set_image(&image_to_pixbuf(img));
        cb.store();
    }

    /// Must run on the GTK main thread. `None` when the clipboard holds no image.
    pub fn get_image() -> Result<Option<Image>, String> {
        clipboard().wait_for_image().map(|p| pixbuf_to_image(&p)).transpose()
    }
}

#[cfg(not(target_os = "linux"))]
mod arboard_backend {
    use paint_core::Image;
    use std::borrow::Cow;
    use std::sync::Mutex;

    static CLIPBOARD: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);

    fn with<T>(f: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>) -> Result<T, arboard::Error> {
        let mut guard = CLIPBOARD.lock().unwrap_or_else(|p| p.into_inner());
        if guard.is_none() {
            *guard = Some(arboard::Clipboard::new()?);
        }
        f(guard.as_mut().expect("clipboard initialised"))
    }

    pub fn set_image(img: &Image) -> Result<(), String> {
        let data = arboard::ImageData { width: img.width() as usize, height: img.height() as usize, bytes: Cow::Borrowed(img.data()) };
        with(|c| c.set_image(data)).map_err(|e| e.to_string())
    }

    pub fn get_image() -> Result<Option<Image>, String> {
        match with(|c| c.get_image()) {
            Ok(d) => Image::from_rgba(d.width as u32, d.height as u32, d.bytes.into_owned()).map(Some),
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
}

fn system_set(img: &Image) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        gtk_backend::set_image(img);
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        arboard_backend::set_image(img)
    }
}

fn system_get() -> Result<Option<Image>, String> {
    #[cfg(target_os = "linux")]
    {
        gtk_backend::get_image()
    }
    #[cfg(not(target_os = "linux"))]
    {
        arboard_backend::get_image()
    }
}

/// Writes a PNG (raw request body) to the system clipboard.
#[tauri::command]
pub fn clipboard_write_png(request: Request<'_>) -> Result<(), String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected PNG bytes".into());
    };
    system_set(&decode_png(bytes)?)
}

/// The clipboard image as PNG bytes; empty when the clipboard holds no image.
#[tauri::command]
pub fn clipboard_read_png() -> Result<Response, String> {
    Ok(Response::new(match system_get()? {
        Some(img) => encode_png(&img)?,
        None => Vec::new(),
    }))
}
