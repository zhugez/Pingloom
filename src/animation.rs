//! Bounded, shared decoding for native desktop renderers.
use crate::assets::ASSETS;
use image::RgbaImage;
use webp_animation::Decoder;

#[derive(Clone)]
pub(crate) struct AnimatedFrame {
    pub at_ms: u32,
    pub image: RgbaImage,
}

pub(crate) fn decode_emote(slug: &str) -> Option<Vec<AnimatedFrame>> {
    let bytes = ASSETS.get_file(format!("emotes/{slug}.webp"))?.contents();
    let decoder = Decoder::new(bytes).ok()?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 || width > 2048 || height > 2048 {
        return None;
    }
    let mut frames = Vec::new();
    let mut decoded_bytes = 0usize;
    let mut previous_end_ms = 0;
    for frame in decoder {
        if frames.len() >= 300 {
            return None;
        }
        let (width, height) = frame.dimensions();
        let size = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;
        decoded_bytes = decoded_bytes.checked_add(size)?;
        if decoded_bytes > 128 * 1024 * 1024 || frame.data().len() != size {
            return None;
        }
        frames.push(AnimatedFrame {
            at_ms: previous_end_ms,
            image: RgbaImage::from_raw(width, height, frame.data().to_vec())?,
        });
        previous_end_ms = frame.timestamp().max(0) as u32;
    }
    (!frames.is_empty()).then_some(frames)
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
}
