//! Bounded, shared decoding for bundled WebP and custom desktop emotes.
use crate::assets::ASSETS;
use image::{AnimationDecoder, ImageDecoder, ImageFormat, ImageReader, Limits, RgbaImage};
use std::io::Cursor;
use webp_animation::Decoder;

#[derive(Clone)]
pub(crate) struct AnimatedFrame {
    pub at_ms: u32,
    pub end_ms: u32,
    pub image: RgbaImage,
}

pub(crate) fn decode_emote(slug: &str) -> Option<Vec<AnimatedFrame>> {
    if pingloom_core::settings::is_custom_emote(slug) {
        let (bytes, _) = crate::custom_emotes::media(slug)?;
        let frames = decode_media(&bytes, false).ok()?;
        return (frames.len() > 1).then(|| fit_custom(frames));
    }
    decode_media(
        ASSETS.get_file(format!("emotes/{slug}.webp"))?.contents(),
        false,
    )
    .ok()
}

fn limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    limits.max_alloc = Some(32 * 1024 * 1024);
    limits
}
fn dimensions(width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 || width > 2048 || height > 2048 {
        return Err("Image dimensions must be 1–2048px".into());
    }
    Ok(())
}
pub(crate) fn format(bytes: &[u8]) -> Result<ImageFormat, String> {
    match image::guess_format(bytes) {
        Ok(
            format @ (ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Gif | ImageFormat::WebP),
        ) => Ok(format),
        _ => Err("Choose a valid PNG, JPG, GIF or WebP image".into()),
    }
}
fn push(
    frames: &mut Vec<AnimatedFrame>,
    image: RgbaImage,
    end_ms: u32,
    decoded_bytes: &mut usize,
) -> Result<(), String> {
    dimensions(image.width(), image.height())?;
    *decoded_bytes = decoded_bytes
        .checked_add(image.as_raw().len())
        .ok_or("Image memory overflow")?;
    if frames.len() >= 300 || *decoded_bytes > 128 * 1024 * 1024 || end_ms > 30_000 {
        return Err("Animation exceeds 300 frames, 30 seconds or 128 MiB decoded memory".into());
    }
    let at_ms = frames.last().map_or(0, |f| f.end_ms);
    if end_ms <= at_ms {
        return Err("Invalid animation timing".into());
    }
    frames.push(AnimatedFrame {
        at_ms,
        end_ms,
        image,
    });
    Ok(())
}
pub(crate) fn decode_media(bytes: &[u8], first_only: bool) -> Result<Vec<AnimatedFrame>, String> {
    let mut frames = Vec::new();
    let mut decoded_bytes = 0;
    let format = format(bytes)?;
    // Inspect dimensions before constructing the native WebP decoder/canvas.
    let mut header = ImageReader::with_format(Cursor::new(bytes), format);
    header.limits(limits());
    let (width, height) = header.into_dimensions().map_err(|e| e.to_string())?;
    dimensions(width, height)?;
    match format {
        ImageFormat::Gif => {
            let mut decoder = image::codecs::gif::GifDecoder::new(Cursor::new(bytes))
                .map_err(|e| e.to_string())?;
            dimensions(decoder.dimensions().0, decoder.dimensions().1)?;
            decoder.set_limits(limits()).map_err(|e| e.to_string())?;
            for frame in decoder.into_frames() {
                let frame = frame.map_err(|e| e.to_string())?;
                let (num, den) = frame.delay().numer_denom_ms();
                let delay = if num == 0 {
                    100
                } else {
                    num.div_ceil(den.max(1)).max(10)
                };
                let end = frames
                    .last()
                    .map_or(0u32, |f: &AnimatedFrame| f.end_ms)
                    .checked_add(delay)
                    .ok_or("Animation duration overflow")?;
                push(&mut frames, frame.into_buffer(), end, &mut decoded_bytes)?;
                if first_only {
                    break;
                }
            }
        }
        ImageFormat::WebP => {
            let decoder = Decoder::new(bytes).map_err(|e| e.to_string())?;
            dimensions(decoder.dimensions().0, decoder.dimensions().1)?;
            for frame in decoder {
                let (width, height) = frame.dimensions();
                let image = RgbaImage::from_raw(width, height, frame.data().to_vec())
                    .ok_or("Invalid WebP frame")?;
                // A static WebP can have an unspecified zero timestamp.
                let end = (frame.timestamp().max(0) as u32).max(
                    frames
                        .last()
                        .map_or(1, |f: &AnimatedFrame| f.end_ms.saturating_add(1)),
                );
                push(&mut frames, image, end, &mut decoded_bytes)?;
                if first_only {
                    break;
                }
            }
        }
        format => {
            let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
            reader.limits(limits());
            let image = reader.decode().map_err(|e| e.to_string())?.into_rgba8();
            push(&mut frames, image, 100, &mut decoded_bytes)?;
        }
    }
    if frames.is_empty() {
        return Err("Image has no frames".into());
    }
    Ok(frames)
}
pub(crate) fn fit_custom(frames: Vec<AnimatedFrame>) -> Vec<AnimatedFrame> {
    frames
        .into_iter()
        .map(|mut frame| {
            let scale = (512.0 / frame.image.width().max(frame.image.height()) as f32).min(1.0);
            frame.image = crate::raster::resize_art_to(
                &frame.image,
                (frame.image.width() as f32 * scale).round().max(1.0) as u32,
                (frame.image.height() as f32 * scale).round().max(1.0) as u32,
            )
            .into_owned();
            frame
        })
        .collect()
}
/// Custom animations loop only within the normal temporary emote lifetime.
pub(crate) fn playback_ms(slug: &str, frames: &[AnimatedFrame], elapsed: u32) -> u32 {
    if pingloom_core::settings::is_custom_emote(slug) {
        frames.last().map_or(elapsed, |f| elapsed % f.end_ms.max(1))
    } else {
        elapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_webp_frame_starts_immediately() {
        let frames = decode_emote("facepalm").unwrap();
        assert_eq!(frames[0].at_ms, 0);
        assert_eq!(frames[1].at_ms, 33);
        assert!(frames.windows(2).all(|pair| pair[0].at_ms <= pair[1].at_ms));
    }
    #[test]
    fn gif_keeps_timing_transparency_and_loops_with_limits() {
        use image::{
            codecs::gif::{GifEncoder, Repeat},
            Delay, Frame, Rgba,
        };
        let make = |count, delay| {
            let mut output = Vec::new();
            {
                let mut encoder = GifEncoder::new(&mut output);
                encoder.set_repeat(Repeat::Infinite).unwrap();
                encoder
                    .encode_frames((0..count).map(|i| {
                        Frame::from_parts(
                            RgbaImage::from_pixel(
                                2,
                                2,
                                if i % 2 == 0 {
                                    Rgba([255, 0, 0, 255])
                                } else {
                                    Rgba([0, 0, 255, 255])
                                },
                            ),
                            0,
                            0,
                            Delay::from_numer_denom_ms(delay, 1),
                        )
                    }))
                    .unwrap();
            }
            output
        };
        let bytes = make(2, 70);
        let frames = decode_media(&bytes, false).unwrap();
        assert_eq!(
            (
                frames[0].at_ms,
                frames[0].end_ms,
                frames[1].at_ms,
                frames[1].end_ms
            ),
            (0, 70, 70, 140)
        );
        assert_eq!(frames[0].image.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(frames[1].image.get_pixel(0, 0).0, [0, 0, 255, 255]);
        assert_eq!(playback_ms("custom_0123456789abcdef", &frames, 150), 10);
        assert_eq!(playback_ms("facepalm", &frames, 150), 150);
        assert_eq!(decode_media(&bytes, true).unwrap().len(), 1);
        assert!(decode_media(&make(301, 10), false).is_err());
        assert!(decode_media(&make(2, 20_000), false).is_err());
        assert!(decode_media(b"GIF89a", false).is_err());
        let truncated = &bytes[..bytes.len() - 6];
        assert!(decode_media(truncated, true).is_ok());
        assert!(decode_media(truncated, false).is_err());
        let mut frames = Vec::new();
        let mut memory = 128 * 1024 * 1024;
        assert!(push(&mut frames, RgbaImage::new(1, 1), 100, &mut memory).is_err());
        let mut transparent = Vec::new();
        GifEncoder::new(&mut transparent)
            .encode_frames([Frame::new(RgbaImage::new(2, 2))])
            .unwrap();
        assert_eq!(
            decode_media(&transparent, false).unwrap()[0]
                .image
                .get_pixel(0, 0)[3],
            0
        );
    }
}
