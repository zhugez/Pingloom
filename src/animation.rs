//! Bounded, shared decoding for bundled WebP and custom desktop emotes.
use crate::assets;
use image::{AnimationDecoder, ImageDecoder, ImageFormat, ImageReader, Limits, RgbaImage};
use std::{io::Cursor, sync::Arc};
use webp_animation::Decoder;

#[derive(Clone)]
pub(crate) struct AnimatedFrame {
    pub at_ms: u32,
    pub end_ms: u32,
    pub image: Arc<RgbaImage>,
}

pub(crate) fn has_animation(slug: &str) -> bool {
    if pingloom_core::settings::is_custom_emote(slug) {
        crate::custom_emotes::animated(slug)
    } else {
        assets::get(format!("emotes/{slug}.webp")).is_some()
    }
}

pub(crate) fn decode_emote(slug: &str) -> Option<Vec<AnimatedFrame>> {
    if pingloom_core::settings::is_custom_emote(slug) {
        let (bytes, _) = crate::custom_emotes::media(slug)?;
        let frames = decode_media(&bytes, false).ok()?;
        return (frames.len() > 1).then(|| fit_custom(frames));
    }
    decode_media(&assets::get(format!("emotes/{slug}.webp"))?, false).ok()
}

/// Decode/resize off the owner thread; never upscale or retain a decoded-frame cache.
pub(crate) fn decode_emote_sized(slug: &str, width: u32) -> Option<Vec<AnimatedFrame>> {
    let mut frames = decode_emote(slug)?;
    let workers = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(4);
    let chunk = frames.len().div_ceil(workers).max(1);
    std::thread::scope(|scope| {
        for frames in frames.chunks_mut(chunk) {
            scope.spawn(move || {
                for frame in frames {
                    if width > 0 && width < frame.image.width() {
                        frame.image =
                            Arc::new(crate::raster::resize_art(&frame.image, width).into_owned());
                    }
                }
            });
        }
    });
    Some(frames)
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
        image: Arc::new(image),
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
                // A static WebP can have an unspecified zero timestamp.
                let end = (frame.timestamp().max(0) as u32).max(
                    frames
                        .last()
                        .map_or(1, |f: &AnimatedFrame| f.end_ms.saturating_add(1)),
                );
                let image = frame.into_rgba_image().map_err(|e| e.to_string())?;
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
            frame.image = Arc::new(
                crate::raster::resize_art_to(
                    &frame.image,
                    (frame.image.width() as f32 * scale).round().max(1.0) as u32,
                    (frame.image.height() as f32 * scale).round().max(1.0) as u32,
                )
                .into_owned(),
            );
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
    fn worker_sizing_preserves_resampling_and_shared_frame_storage() {
        let full = decode_emote("facepalm").unwrap();
        let sized = decode_emote_sized("facepalm", 187).unwrap();
        assert_eq!(full.len(), sized.len());
        for (a, b) in full.iter().zip(&sized) {
            assert_eq!((a.at_ms, a.end_ms), (b.at_ms, b.end_ms));
            assert_eq!(
                crate::raster::resize_art(&a.image, 187).as_ref(),
                b.image.as_ref()
            );
        }
        assert!(
            sized.iter().map(|f| f.image.as_raw().len()).sum::<usize>()
                < full.iter().map(|f| f.image.as_raw().len()).sum::<usize>() / 2
        );
        let cloned = sized[0].clone();
        assert!(Arc::ptr_eq(&cloned.image, &sized[0].image));
        assert_eq!(
            decode_emote_sized("facepalm", 1024).unwrap()[0]
                .image
                .dimensions(),
            full[0].image.dimensions()
        );
    }

    /// Release decode benchmark read by `tools/perf_gate.py`:
    /// `cargo nextest run --release --run-ignored only -E 'test(decode_benchmark)'`.
    #[test]
    #[ignore]
    fn decode_benchmark() {
        let mut ms: Vec<f64> = assets::files("emotes")
            .iter()
            .filter(|file| file.extension().is_some_and(|ext| ext == "webp"))
            .map(|file| {
                let slug = file.file_stem().unwrap().to_str().unwrap();
                let start = std::time::Instant::now();
                assert!(decode_emote(slug).unwrap().len() > 1);
                start.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        ms.sort_by(f64::total_cmp);
        let at = |q: f64| ms[((ms.len() - 1) as f64 * q).round() as usize];
        let report = serde_json::json!({
            "emotes": ms.len(),
            "decode_ms_p50": at(0.5),
            "decode_ms_p95": at(0.95),
            "peak_rss_mib": peak_rss_mib(),
        });
        println!("{report}");
        if let Some(path) = std::env::var_os("PINGLOOM_BENCH_OUT") {
            std::fs::write(path, report.to_string()).unwrap();
        }
    }

    #[cfg(unix)]
    fn peak_rss_mib() -> f64 {
        // ponytail: 64-bit rusage layout (two 16-byte timevals, then ru_maxrss).
        #[repr(C)]
        struct Rusage {
            times: [i64; 4],
            maxrss: i64,
            rest: [i64; 13],
        }
        extern "C" {
            fn getrusage(who: i32, usage: *mut Rusage) -> i32;
        }
        let mut usage = Rusage {
            times: [0; 4],
            maxrss: 0,
            rest: [0; 13],
        };
        assert_eq!(unsafe { getrusage(0, &mut usage) }, 0);
        // Linux reports KiB, macOS bytes.
        usage.maxrss as f64
            / if cfg!(target_os = "macos") {
                1048576.0
            } else {
                1024.0
            }
    }

    #[cfg(windows)]
    fn peak_rss_mib() -> f64 {
        use windows_sys::Win32::System::{
            ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            Threading::GetCurrentProcess,
        };
        let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        assert_ne!(
            unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) },
            0
        );
        counters.PeakWorkingSetSize as f64 / 1048576.0
    }

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
