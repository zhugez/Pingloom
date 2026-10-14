//! Shared native artwork sampling. Filter premultiplied color, not hidden RGB.
use image::{imageops, Rgba, Rgba32FImage, RgbaImage};
use std::borrow::Cow;

pub(crate) fn resize_art(image: &RgbaImage, width: u32) -> Cow<'_, RgbaImage> {
    if image.width() == width || image.width() == 0 || image.height() == 0 {
        return Cow::Borrowed(image);
    }
    let width = width.max(1);
    let height = ((u64::from(width) * u64::from(image.height()) + u64::from(image.width()) / 2)
        / u64::from(image.width()))
    .max(1) as u32;
    let premultiplied = Rgba32FImage::from_fn(image.width(), image.height(), |x, y| {
        let pixel = image.get_pixel(x, y);
        let a = f32::from(pixel[3]) / 255.0;
        Rgba([
            f32::from(pixel[0]) / 255.0 * a,
            f32::from(pixel[1]) / 255.0 * a,
            f32::from(pixel[2]) / 255.0 * a,
            a,
        ])
    });
    // Cubic reconstruction retains small-art detail without nearest-neighbor stair steps.
    let filtered = imageops::resize(
        &premultiplied,
        width,
        height,
        imageops::FilterType::CatmullRom,
    );
    Cow::Owned(RgbaImage::from_fn(width, height, |x, y| {
        let pixel = filtered.get_pixel(x, y);
        let a = pixel[3].clamp(0.0, 1.0);
        if a <= 1e-6 {
            return Rgba([0; 4]);
        }
        Rgba([
            (pixel[0] / a * 255.0).clamp(0.0, 255.0).round() as u8,
            (pixel[1] / a * 255.0).clamp(0.0, 255.0).round() as u8,
            (pixel[2] / a * 255.0).clamp(0.0, 255.0).round() as u8,
            (a * 255.0).round() as u8,
        ])
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sampling_preserves_native_pixels_and_ignores_hidden_rgb() {
        let art = RgbaImage::from_fn(2, 1, |x, _| {
            if x == 0 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 255, 255, 0])
            }
        });
        assert!(matches!(resize_art(&art, 2), Cow::Borrowed(_)));
        for width in [1, 8] {
            let scaled = resize_art(&art, width);
            assert!(scaled.pixels().any(|p| p[3] > 0 && p[3] < 255));
            for p in scaled.pixels().filter(|p| p[3] > 0) {
                assert_eq!([p[0], p[1], p[2]], [255, 0, 0]);
            }
        }
    }
}
