//! Thumbnail encoding: decode, downscale (never upscale) and re-encode.

use anyhow::{anyhow, Result};
use image::{GenericImageView, ImageEncoder};
use tracing::debug;

use crate::decode::fast_jpeg_decode;
use crate::settings::{ThumbnailFormat, ThumbnailSettings};

/// File extension (and media type) of a stored thumbnail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailExt {
    Webp,
    Jpg,
    Png,
}

impl ThumbnailExt {
    /// Every extension a published thumbnail can have.
    pub const ALL: [ThumbnailExt; 3] = [Self::Webp, Self::Jpg, Self::Png];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Webp => "webp",
            Self::Jpg => "jpg",
            Self::Png => "png",
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Webp => "image/webp",
            Self::Jpg => "image/jpeg",
            Self::Png => "image/png",
        }
    }

    /// Extension of a stored thumbnail path (`.jpeg` accepted). Unknown
    /// extensions are treated as WebP, the default format.
    pub fn from_path(path: &str) -> Self {
        let lower = path.to_ascii_lowercase();
        if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
            Self::Jpg
        } else if lower.ends_with(".png") {
            Self::Png
        } else {
            Self::Webp
        }
    }

    pub(crate) fn from_file_ext(ext: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == ext)
    }
}

/// An encoded thumbnail, ready to be published.
#[derive(Debug, Clone)]
pub struct EncodedThumbnail {
    pub bytes: Vec<u8>,
    pub ext: ThumbnailExt,
}

impl EncodedThumbnail {
    pub fn content_type(&self) -> &'static str {
        self.ext.content_type()
    }
}

/// Encode `source` (a raw page image) following `settings`.
pub(crate) fn encode(source: &[u8], settings: &ThumbnailSettings) -> Result<EncodedThumbnail> {
    let t0 = std::time::Instant::now();

    let (img, orig_w, orig_h) = match fast_jpeg_decode(source, settings.width, settings.height) {
        Some(decoded) => (
            decoded.image,
            decoded.original_width,
            decoded.original_height,
        ),
        None => {
            let img = image::load_from_memory(source)
                .map_err(|e| anyhow!("failed to load image: {e}"))?;
            let (w, h) = img.dimensions();
            (img, w, h)
        }
    };
    let t_decode = t0.elapsed();

    // Never upscale.
    let resized = img.thumbnail(settings.width.min(orig_w), settings.height.min(orig_h));
    let (w, h) = resized.dimensions();

    let target = match settings.format {
        ThumbnailFormat::Original => match image::guess_format(source) {
            Ok(image::ImageFormat::Png) => ThumbnailExt::Png,
            _ => ThumbnailExt::Jpg,
        },
        ThumbnailFormat::Jpeg => ThumbnailExt::Jpg,
        ThumbnailFormat::Png => ThumbnailExt::Png,
        ThumbnailFormat::Webp => ThumbnailExt::Webp,
    };

    let bytes = match target {
        ThumbnailExt::Png => {
            let rgba = resized.to_rgba8();
            let mut buf = Vec::new();
            image::codecs::png::PngEncoder::new(&mut buf)
                .write_image(&rgba, w, h, image::ColorType::Rgba8.into())
                .map_err(|e| anyhow!("png encode failed: {e}"))?;
            buf
        }
        ThumbnailExt::Jpg => {
            let rgb = resized.to_rgb8();
            let mut buf = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, settings.quality)
                .encode(&rgb, w, h, image::ColorType::Rgb8.into())
                .map_err(|e| anyhow!("jpeg encode failed: {e}"))?;
            buf
        }
        ThumbnailExt::Webp => {
            let rgb = resized.to_rgb8();
            webp::Encoder::new(rgb.as_raw(), webp::PixelLayout::Rgb, w, h)
                .encode(f32::from(settings.quality))
                .to_vec()
        }
    };

    debug!(
        target: "thumbnail",
        "[THUMBNAIL] {}x{} -> {}x{} format={} decode={:.0}ms total={:.0}ms size={}KB",
        orig_w,
        orig_h,
        w,
        h,
        target.as_str(),
        t_decode.as_secs_f64() * 1000.0,
        t0.elapsed().as_secs_f64() * 1000.0,
        bytes.len() / 1024,
    );
    Ok(EncodedThumbnail { bytes, ext: target })
}

#[cfg(test)]
pub(crate) mod test_images {
    fn rgb_image(width: u32, height: u32) -> image::RgbImage {
        image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        })
    }

    pub fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(rgb_image(width, height))
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    pub fn jpeg_bytes(width: u32, height: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 85)
            .encode_image(&rgb_image(width, height))
            .unwrap();
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::test_images::{jpeg_bytes, png_bytes};
    use super::*;

    fn settings(format: ThumbnailFormat, width: u32, height: u32) -> ThumbnailSettings {
        ThumbnailSettings {
            format,
            width,
            height,
            ..ThumbnailSettings::default()
        }
    }

    fn dims(bytes: &[u8]) -> (u32, u32) {
        image::load_from_memory(bytes).unwrap().dimensions()
    }

    #[test]
    fn defaults_to_downscaled_webp() {
        let out = encode(&png_bytes(600, 800), &ThumbnailSettings::default()).unwrap();
        assert_eq!(out.ext, ThumbnailExt::Webp);
        assert_eq!(
            image::guess_format(&out.bytes).unwrap(),
            image::ImageFormat::WebP
        );
        assert_eq!(dims(&out.bytes), (300, 400));
    }

    #[test]
    fn never_upscales() {
        let out = encode(&png_bytes(40, 60), &ThumbnailSettings::default()).unwrap();
        assert_eq!(dims(&out.bytes), (40, 60));
    }

    #[test]
    fn honors_explicit_formats() {
        let jpg = encode(
            &png_bytes(100, 100),
            &settings(ThumbnailFormat::Jpeg, 50, 50),
        )
        .unwrap();
        assert_eq!(jpg.ext, ThumbnailExt::Jpg);
        assert_eq!(
            image::guess_format(&jpg.bytes).unwrap(),
            image::ImageFormat::Jpeg
        );

        let png = encode(
            &jpeg_bytes(100, 100),
            &settings(ThumbnailFormat::Png, 50, 50),
        )
        .unwrap();
        assert_eq!(png.ext, ThumbnailExt::Png);
        assert_eq!(dims(&png.bytes), (50, 50));
    }

    #[test]
    fn original_format_keeps_png_and_turns_the_rest_into_jpeg() {
        let from_png = encode(
            &png_bytes(80, 80),
            &settings(ThumbnailFormat::Original, 40, 40),
        )
        .unwrap();
        assert_eq!(from_png.ext, ThumbnailExt::Png);

        let from_jpeg = encode(
            &jpeg_bytes(80, 80),
            &settings(ThumbnailFormat::Original, 40, 40),
        )
        .unwrap();
        assert_eq!(from_jpeg.ext, ThumbnailExt::Jpg);
        assert_eq!(from_jpeg.content_type(), "image/jpeg");
    }

    #[test]
    fn rejects_garbage() {
        assert!(encode(b"not an image", &ThumbnailSettings::default()).is_err());
    }

    #[test]
    fn extension_round_trips_through_paths() {
        assert_eq!(ThumbnailExt::from_path("/t/a.jpg"), ThumbnailExt::Jpg);
        assert_eq!(ThumbnailExt::from_path("/t/a.JPEG"), ThumbnailExt::Jpg);
        assert_eq!(ThumbnailExt::from_path("/t/a.png"), ThumbnailExt::Png);
        assert_eq!(ThumbnailExt::from_path("/t/a.webp"), ThumbnailExt::Webp);
        assert_eq!(ThumbnailExt::Png.content_type(), "image/png");
        for ext in ThumbnailExt::ALL {
            assert_eq!(ThumbnailExt::from_file_ext(ext.as_str()), Some(ext));
        }
        assert_eq!(ThumbnailExt::from_file_ext("raw"), None);
    }
}
