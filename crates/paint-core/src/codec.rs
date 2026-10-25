//! Image file decoding and encoding (PNG, JPEG, BMP, GIF, TIFF, WebP, ICO).

use crate::color::Rgba;
use crate::image::Image;
use image::{codecs, ExtendedColorType, ImageEncoder, RgbaImage};
use std::io::Cursor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Png,
    Jpeg,
    Bmp,
    Gif,
    Tiff,
    Webp,
    Ico,
}

impl Format {
    pub const ALL: [Format; 7] = [Format::Png, Format::Jpeg, Format::Bmp, Format::Gif, Format::Tiff, Format::Webp, Format::Ico];

    /// From a file extension or format name, case-insensitive.
    pub fn from_name(s: &str) -> Option<Format> {
        let s = s.trim().trim_start_matches('.').to_ascii_lowercase();
        Some(match s.as_str() {
            "png" => Format::Png,
            "jpg" | "jpeg" | "jpe" | "jfif" => Format::Jpeg,
            "bmp" | "dib" => Format::Bmp,
            "gif" => Format::Gif,
            "tif" | "tiff" => Format::Tiff,
            "webp" => Format::Webp,
            "ico" => Format::Ico,
            _ => return None,
        })
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpeg => "jpg",
            Format::Bmp => "bmp",
            Format::Gif => "gif",
            Format::Tiff => "tiff",
            Format::Webp => "webp",
            Format::Ico => "ico",
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            Format::Png => "image/png",
            Format::Jpeg => "image/jpeg",
            Format::Bmp => "image/bmp",
            Format::Gif => "image/gif",
            Format::Tiff => "image/tiff",
            Format::Webp => "image/webp",
            Format::Ico => "image/x-icon",
        }
    }

    /// Whether the format keeps transparency.
    pub fn has_alpha(self) -> bool {
        !matches!(self, Format::Jpeg | Format::Bmp)
    }
}

/// Largest accepted image side, to keep memory bounded.
pub const MAX_SIDE: u32 = 16384;

pub fn decode(bytes: &[u8]) -> Result<Image, String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    if img.width() == 0 || img.height() == 0 || img.width() > MAX_SIDE || img.height() > MAX_SIDE {
        return Err(format!("unsupported image size {}x{}", img.width(), img.height()));
    }
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Image::from_rgba(w, h, rgba.into_raw())
}

fn flatten_rgb(img: &Image, bg: Rgba) -> Vec<u8> {
    img.data()
        .as_chunks::<4>().0.iter()
        .flat_map(|p| {
            let c = Rgba::from_array([p[0], p[1], p[2], p[3]]).over(bg);
            [c.r, c.g, c.b]
        })
        .collect()
}

/// Encodes `img`. JPEG and BMP have no alpha channel and are flattened onto white.
/// `quality` (1-100) only affects JPEG.
pub fn encode(img: &Image, fmt: Format, quality: u8) -> Result<Vec<u8>, String> {
    let (w, h) = (img.width(), img.height());
    let mut out = Cursor::new(Vec::new());
    let err = |e: image::ImageError| e.to_string();
    match fmt {
        Format::Png => codecs::png::PngEncoder::new(&mut out).write_image(img.data(), w, h, ExtendedColorType::Rgba8).map_err(err)?,
        Format::Jpeg => codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100))
            .write_image(&flatten_rgb(img, Rgba::WHITE), w, h, ExtendedColorType::Rgb8)
            .map_err(err)?,
        Format::Bmp => codecs::bmp::BmpEncoder::new(&mut out).write_image(&flatten_rgb(img, Rgba::WHITE), w, h, ExtendedColorType::Rgb8).map_err(err)?,
        Format::Gif => {
            let rgba = RgbaImage::from_raw(w, h, img.data().to_vec()).ok_or("bad buffer")?;
            let mut enc = codecs::gif::GifEncoder::new_with_speed(&mut out, 10);
            enc.encode_frame(image::Frame::new(rgba)).map_err(err)?;
        }
        Format::Tiff => codecs::tiff::TiffEncoder::new(&mut out).write_image(img.data(), w, h, ExtendedColorType::Rgba8).map_err(err)?,
        Format::Webp => codecs::webp::WebPEncoder::new_lossless(&mut out).write_image(img.data(), w, h, ExtendedColorType::Rgba8).map_err(err)?,
        Format::Ico => {
            let small = if w > 256 || h > 256 { img.thumbnail(256) } else { img.clone() };
            codecs::ico::IcoEncoder::new(&mut out)
                .write_image(small.data(), small.width(), small.height(), ExtendedColorType::Rgba8)
                .map_err(err)?
        }
    }
    Ok(out.into_inner())
}

/// Detects the format of encoded bytes.
pub fn sniff(bytes: &[u8]) -> Option<Format> {
    Some(match image::guess_format(bytes).ok()? {
        image::ImageFormat::Png => Format::Png,
        image::ImageFormat::Jpeg => Format::Jpeg,
        image::ImageFormat::Bmp => Format::Bmp,
        image::ImageFormat::Gif => Format::Gif,
        image::ImageFormat::Tiff => Format::Tiff,
        image::ImageFormat::WebP => Format::Webp,
        image::ImageFormat::Ico => Format::Ico,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Image {
        let mut img = Image::filled(20, 12, Rgba::WHITE);
        img.fill_rect(crate::geom::IRect::new(0, 0, 10, 12), Rgba::rgb(237, 28, 36));
        img.set(19, 11, Rgba::new(0, 0, 255, 0));
        img
    }

    #[test]
    fn roundtrip_every_format() {
        let img = sample();
        for fmt in Format::ALL {
            let bytes = encode(&img, fmt, 95).unwrap_or_else(|e| panic!("{fmt:?}: {e}"));
            assert_eq!(sniff(&bytes), Some(fmt), "{fmt:?}");
            let back = decode(&bytes).unwrap();
            assert_eq!((back.width(), back.height()), (20, 12), "{fmt:?}");
            let tol = if fmt == Format::Jpeg { 12 } else { 0 };
            assert!(back.get(2, 2).distance(Rgba::rgb(237, 28, 36)) <= tol, "{fmt:?} {:?}", back.get(2, 2));
            assert!(back.get(15, 5).distance(Rgba::WHITE) <= tol, "{fmt:?}");
            if fmt.has_alpha() {
                assert_eq!(back.get(19, 11).a, 0, "{fmt:?} lost alpha");
            } else {
                assert_eq!(back.get(19, 11), Rgba::WHITE, "{fmt:?} not flattened");
            }
        }
    }

    #[test]
    fn names_and_errors() {
        assert_eq!(Format::from_name(".JPEG"), Some(Format::Jpeg));
        assert_eq!(Format::from_name("tif"), Some(Format::Tiff));
        assert_eq!(Format::from_name("psd"), None);
        assert!(decode(b"not an image").is_err());
        for f in Format::ALL {
            assert_eq!(Format::from_name(f.extension()), Some(f));
            assert!(f.mime().starts_with("image/"));
        }
        let big = Image::new(300, 10);
        let ico = encode(&big, Format::Ico, 0).unwrap();
        let back = decode(&ico).unwrap();
        assert_eq!(back.width(), 256);
    }
}
