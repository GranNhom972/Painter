//! Native services of the desktop app: files, command line and system clipboard.

use paint_app::clipboard::{decode_png, encode_png};
use paint_app::files::{file_from_args, percent_decode, write_atomic};
use paint_core::{codec, Format, IRect, Image, Rgba};

fn sample_png() -> Vec<u8> {
    let mut img = Image::filled(7, 5, Rgba::WHITE);
    img.fill_rect(IRect::new(0, 0, 3, 5), Rgba::rgb(237, 28, 36));
    img.set(6, 4, Rgba::new(0, 0, 255, 128));
    codec::encode(&img, Format::Png, 100).unwrap()
}

#[test]
fn percent_decoding_matches_encode_uri_component() {
    // encodeURIComponent("/home/dév/Mes images/chat & chien.png")
    let enc = "%2Fhome%2Fd%C3%A9v%2FMes%20images%2Fchat%20%26%20chien.png";
    assert_eq!(percent_decode(enc).unwrap(), "/home/dév/Mes images/chat & chien.png");
    assert_eq!(percent_decode("plain").unwrap(), "plain");
    assert!(percent_decode("%2").is_err());
    assert!(percent_decode("%zz").is_err());
    assert!(percent_decode("%ff").is_err(), "invalid UTF-8");
}

#[test]
fn atomic_write_creates_and_replaces() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dessin.png");
    write_atomic(&path, b"first").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"first");
    write_atomic(&path, b"second").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"second");
    let leftovers: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(leftovers.len(), 1, "no temporary file left: {leftovers:?}");
    let err = write_atomic(&dir.path().join("missing/dir.png"), b"x").unwrap_err();
    assert!(err.contains("missing"));
}

#[test]
fn command_line_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("photo d'été.png");
    std::fs::write(&path, sample_png()).unwrap();
    let p = path.to_string_lossy().to_string();
    assert_eq!(file_from_args(vec!["--flag".into(), p.clone()]), Some(p.clone()));
    let uri = format!("file://{}", p.replace(' ', "%20").replace('é', "%C3%A9"));
    assert_eq!(file_from_args(vec![uri]), Some(p));
    assert_eq!(file_from_args(vec!["/nope/missing.png".into()]), None);
    assert_eq!(file_from_args(Vec::<String>::new()), None);
}

#[test]
fn clipboard_png_conversions() {
    let png = sample_png();
    let img = decode_png(&png).unwrap();
    assert_eq!((img.width(), img.height()), (7, 5));
    assert_eq!(img.get(0, 0), Rgba::rgb(237, 28, 36));
    assert_eq!(img.get(6, 4).a, 128);
    assert_eq!(decode_png(&encode_png(&img).unwrap()).unwrap(), img);
    assert!(decode_png(b"not a png").is_err());
}
