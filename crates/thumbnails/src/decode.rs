//! Fast JPEG decoding with DCT scaling.

use image::DynamicImage;

/// A JPEG decoded at reduced resolution, with the source dimensions.
pub struct FastJpeg {
    pub image: DynamicImage,
    pub original_width: u32,
    pub original_height: u32,
}

/// Decode a JPEG directly at a reduced resolution (1/8, 1/4, 1/2 or full).
///
/// The decoder picks the smallest DCT scale whose output is at least
/// `target_w`×`target_h` (values above `u16::MAX` are clamped). Returns `None`
/// when the bytes are not a JPEG or the decode fails, so callers can fall back
/// to the generic `image` decoder.
pub fn fast_jpeg_decode(bytes: &[u8], target_w: u32, target_h: u32) -> Option<FastJpeg> {
    if image::guess_format(bytes).ok()? != image::ImageFormat::Jpeg {
        return None;
    }

    let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(bytes));
    decoder.read_info().ok()?;
    let info = decoder.info()?;
    let original_width = u32::from(info.width);
    let original_height = u32::from(info.height);

    let clamp = |v: u32| u16::try_from(v).unwrap_or(u16::MAX);
    decoder.scale(clamp(target_w), clamp(target_h)).ok()?;

    let pixels = decoder.decode().ok()?;
    let info = decoder.info()?;
    let width = u32::from(info.width);
    let height = u32::from(info.height);

    let image = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => {
            DynamicImage::ImageRgb8(image::RgbImage::from_raw(width, height, pixels)?)
        }
        jpeg_decoder::PixelFormat::L8 => {
            DynamicImage::ImageLuma8(image::GrayImage::from_raw(width, height, pixels)?)
        }
        _ => return None,
    };
    Some(FastJpeg {
        image,
        original_width,
        original_height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::test_images::{jpeg_bytes, png_bytes};
    use image::GenericImageView;

    #[test]
    fn scales_jpeg_and_keeps_original_dimensions() {
        let decoded = fast_jpeg_decode(&jpeg_bytes(800, 1200), 100, 150).expect("jpeg decodes");
        assert_eq!(
            (decoded.original_width, decoded.original_height),
            (800, 1200)
        );
        let (w, h) = decoded.image.dimensions();
        assert!((100..800).contains(&w), "decoded width {w}");
        assert!((150..1200).contains(&h), "decoded height {h}");
    }

    #[test]
    fn rejects_non_jpeg() {
        assert!(fast_jpeg_decode(&png_bytes(10, 10), 5, 5).is_none());
        assert!(fast_jpeg_decode(b"not an image", 5, 5).is_none());
    }

    #[test]
    fn unbounded_height_target_does_not_overflow() {
        // The API page renderer asks for `u32::MAX` as height (width-bound resize).
        let decoded = fast_jpeg_decode(&jpeg_bytes(64, 64), 32, u32::MAX).expect("jpeg decodes");
        let (w, _) = decoded.image.dimensions();
        assert!(w >= 32, "decoded width {w}");
        assert_eq!((decoded.original_width, decoded.original_height), (64, 64));
    }
}
