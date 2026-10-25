//! Linux system clipboard through GTK, against a real X server
//! (run with DISPLAY set, e.g. under Xvfb). Another X client (arboard)
//! plays the role of a different application.

#[cfg(target_os = "linux")]
fn main() {
    use paint_app::clipboard::gtk_backend::{get_image, image_to_pixbuf, pixbuf_to_image, set_image};
    use paint_core::{IRect, Image, Rgba};

    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        println!("gtk_clipboard: skipped (no graphical session)");
        return;
    }
    gtk::init().expect("GTK init");

    let mut img = Image::filled(9, 6, Rgba::WHITE);
    img.fill_rect(IRect::new(0, 0, 4, 6), Rgba::rgb(237, 28, 36));
    img.set(8, 5, Rgba::new(0, 0, 255, 128));

    // Pixbuf conversions keep every pixel, alpha included.
    assert_eq!(pixbuf_to_image(&image_to_pixbuf(&img)).unwrap(), img);

    // Paint copies; another application pastes. The reader runs on its own
    // thread while this (GTK main) thread answers the selection request.
    set_image(&img);
    let pasted = spin_until(std::thread::spawn(|| {
        let mut other = arboard::Clipboard::new().expect("second X client");
        let d = other.get_image().expect("paste in the other application");
        (d.width, d.height, d.bytes.to_vec())
    }));
    assert_eq!((pasted.0, pasted.1), (9, 6));
    assert_eq!(&pasted.2[0..4], &[237, 28, 36, 255]);
    let mut other = arboard::Clipboard::new().expect("second X client");

    // Another application copies; Paint pastes.
    let mut green = Image::filled(5, 4, Rgba::rgb(34, 177, 76));
    green.set(0, 0, Rgba::BLACK);
    other
        .set_image(arboard::ImageData { width: 5, height: 4, bytes: green.data().to_vec().into() })
        .unwrap();
    let got = get_image().expect("read").expect("an image");
    assert_eq!((got.width(), got.height()), (5, 4));
    assert_eq!(got.get(0, 0), Rgba::BLACK);
    assert_eq!(got.get(3, 2), Rgba::rgb(34, 177, 76));

    // Text is not an image.
    other.set_text("bonjour").unwrap();
    assert!(get_image().expect("read").is_none());
    println!("gtk_clipboard: ok (copy to and paste from another application)");
}

/// Runs the GTK main loop (serving selection requests) until `job` finishes.
#[cfg(target_os = "linux")]
fn spin_until<T>(job: std::thread::JoinHandle<T>) -> T {
    let start = std::time::Instant::now();
    while !job.is_finished() {
        while gtk::events_pending() {
            gtk::main_iteration();
        }
        assert!(start.elapsed().as_secs() < 10, "timed out");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    job.join().expect("reader thread")
}

#[cfg(not(target_os = "linux"))]
fn main() {}
