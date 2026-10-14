use super::{audio::Audio, QueuedAction};
use crate::{
    animation::{decode_emote, AnimatedFrame},
    assets::ASSETS,
    logic::{effect_fade, effect_pop, selection_blend},
};
use image::{imageops, Pixel, Rgba, RgbaImage};
use pingloom_core::{geometry::pick_slice, Action, Point, Settings, WheelKind};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) struct Frame {
    pub x: i32,
    pub y: i32,
    pub image: RgbaImage,
}
struct Wheel {
    kind: WheelKind,
    origin: Point,
    hot: Option<usize>,
    blend: [f64; 8],
    updated: Instant,
    scale: f64,
}
struct Ping {
    id: pingloom_core::PingId,
    point: Point,
    started: Instant,
}
struct Emote {
    point: Point,
    started: Instant,
    frames_started: Instant,
    slug: String,
    frames: Vec<AnimatedFrame>,
}
pub(super) struct Renderer {
    wheel: Option<Wheel>,
    pings: Vec<Ping>,
    emote: Option<Emote>,
    cache: HashMap<String, Arc<RgbaImage>>,
    pub(super) reduced: bool,
}
impl Renderer {
    pub fn new() -> Self {
        Self {
            wheel: None,
            pings: Vec::new(),
            emote: None,
            cache: HashMap::new(),
            reduced: false,
        }
    }
    pub fn cancel(&mut self) {
        self.wheel = None;
        self.pings.clear();
        self.emote = None;
    }
    fn asset(&mut self, path: &str) -> Option<Arc<RgbaImage>> {
        if let Some(image) = self.cache.get(path) {
            return Some(Arc::clone(image));
        }
        let bytes = ASSETS.get_file(path)?.contents();
        let image = Arc::new(image::load_from_memory(bytes).ok()?.into_rgba8());
        self.cache.insert(path.into(), Arc::clone(&image));
        Some(image)
    }
    pub fn action(&mut self, queued: QueuedAction, settings: &Settings, audio: &Audio) {
        let volume = if settings.muted {
            0.0
        } else {
            settings.volume as f32 / 100.0
        };
        let selection = match queued.action {
            Action::WheelOpen { wheel, origin } => {
                self.wheel = Some(Wheel {
                    kind: wheel,
                    origin,
                    hot: None,
                    blend: [0.0; 8],
                    updated: Instant::now(),
                    scale: queued.scale,
                });
                None
            }
            Action::WheelMove { slot, .. } => {
                if let Some(wheel) = &mut self.wheel {
                    if wheel.hot != slot && slot.is_some() && settings.tick_sound {
                        audio.play("sounds/button.wav", volume * 0.5);
                    }
                    wheel.hot = slot;
                }
                None
            }
            Action::Cancel => {
                self.wheel = None;
                None
            }
            Action::Select { wheel, slot, .. } => {
                self.wheel = None;
                queued.origin.map(|point| (wheel, Some(slot), point))
            }
            Action::Click { wheel, point } => Some((wheel, None, point)),
            Action::Toggled { .. } => None,
        };
        if let Some((kind, slot, point)) = selection {
            match kind {
                WheelKind::Ping => {
                    let id = slot.map_or(settings.click_ping_id, |slot| settings.wheel[slot]);
                    if self.pings.len() >= 60 {
                        self.pings.remove(0);
                    }
                    self.pings.push(Ping {
                        id,
                        point,
                        started: Instant::now(),
                    });
                    audio.play(format!("sounds/{}.wav", id.asset().sound), volume);
                }
                WheelKind::Emote => {
                    let slug = slot
                        .map_or(&settings.click_emote_id, |slot| &settings.emote_wheel[slot])
                        .clone();
                    let frames = if self.reduced {
                        Vec::new()
                    } else {
                        decode_emote(&slug).unwrap_or_default()
                    };
                    let started = Instant::now();
                    self.emote = Some(Emote {
                        point,
                        started,
                        frames_started: started,
                        slug: slug.clone(),
                        frames,
                    });
                    if settings.emote_sound {
                        audio.play(format!("emotes/{slug}.ogg"), volume);
                    }
                }
            }
        }
    }
    pub fn frame(&mut self, settings: &Settings) -> Option<Frame> {
        let now = Instant::now();
        self.pings.retain(|p| {
            now.duration_since(p.started).as_secs_f64() < settings.ping_duration_s as f64
        });
        if self
            .emote
            .as_ref()
            .is_some_and(|e| now.duration_since(e.started) >= Duration::from_secs(3))
        {
            self.emote = None;
        }
        let mut areas = Vec::new();
        if let Some(w) = &self.wheel {
            areas.push((w.origin, (230.0 * w.scale).ceil() as i32));
        }
        for p in &self.pings {
            areas.push((p.point, settings.ping_size_px as i32 * 2));
        }
        if let Some(e) = &self.emote {
            areas.push((e.point, settings.emote_size_px as i32));
        }
        let left = areas.iter().map(|(p, r)| p.x - r).min()?;
        let top = areas.iter().map(|(p, r)| p.y - r).min()?;
        let right = areas.iter().map(|(p, r)| p.x + r).max()?;
        let bottom = areas.iter().map(|(p, r)| p.y + r).max()?;
        let width = (right - left) as u32;
        let height = (bottom - top) as u32;
        // ponytail: one native surface; split per-effect windows if very wide desktops exceed this bound.
        if width as u64 * height as u64 > 16_000_000 {
            self.pings.clear();
            self.emote = None;
            return None;
        }
        let mut image = RgbaImage::new(width, height);
        if let Some(wheel) = &mut self.wheel {
            let dt = now.duration_since(wheel.updated).as_secs_f64();
            wheel.updated = now;
            for (slot, blend) in wheel.blend.iter_mut().enumerate() {
                *blend = if self.reduced {
                    if wheel.hot == Some(slot) {
                        1.0
                    } else {
                        0.0
                    }
                } else {
                    selection_blend(*blend, wheel.hot == Some(slot), dt)
                };
            }
            let radius = (220.0 * wheel.scale).ceil() as i32;
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let distance = (dx as f64).hypot(dy as f64);
                    let color = if distance <= 84.0 * wheel.scale {
                        Some([34, 38, 45, 245])
                    } else if distance <= 220.0 * wheel.scale {
                        let slot = pick_slice(dx as f64, dy as f64, 0.0).unwrap();
                        let blend = wheel.blend[slot];
                        Some([
                            (35.0 + 186.0 * blend) as u8,
                            (42.0 + 164.0 * blend) as u8,
                            (50.0 + 104.0 * blend) as u8,
                            235,
                        ])
                    } else {
                        None
                    };
                    if let Some(color) = color {
                        point(
                            &mut image,
                            wheel.origin.x - left + dx,
                            wheel.origin.y - top + dy,
                            color,
                        );
                    }
                }
            }
            ring(
                &mut image,
                wheel.origin.x - left,
                wheel.origin.y - top,
                86.0 * wheel.scale,
                [255, 170, 170, 255],
            );
            ring(
                &mut image,
                wheel.origin.x - left,
                wheel.origin.y - top,
                220.0 * wheel.scale,
                [222, 226, 232, 255],
            );
            for i in -10..=10 {
                point(
                    &mut image,
                    wheel.origin.x - left + i,
                    wheel.origin.y - top + i,
                    [255, 137, 137, 255],
                );
                point(
                    &mut image,
                    wheel.origin.x - left - i,
                    wheel.origin.y - top + i,
                    [255, 137, 137, 255],
                );
            }
        }
        if let Some(wheel) = &self.wheel {
            let kind = wheel.kind;
            let scale = wheel.scale;
            let origin = wheel.origin;
            let blends = wheel.blend;
            for (slot, blend) in blends.into_iter().enumerate() {
                let angle = (slot as f64 * 45.0 - 90.0).to_radians();
                let path = match kind {
                    WheelKind::Ping => {
                        format!("textures/{}.png", settings.wheel[slot].asset().icon)
                    }
                    WheelKind::Emote => format!("emotes/{}.png", settings.emote_wheel[slot]),
                };
                if let Some(art) = self.asset(&path) {
                    blit(
                        &mut image,
                        &art,
                        origin.x - left + (angle.cos() * 153.0 * scale) as i32,
                        origin.y - top + (angle.sin() * 153.0 * scale) as i32,
                        (46.0 * scale * (1.0 + 0.18 * blend)) as u32,
                        1.0,
                    );
                }
            }
        }
        let pings: Vec<_> = self
            .pings
            .iter()
            .map(|p| (p.id, p.point, now.duration_since(p.started).as_secs_f64()))
            .collect();
        for (id, at, age) in pings {
            let size = settings.ping_size_px as f64;
            let fade = effect_fade(age, settings.ping_duration_s as f64);
            let rgb = u32::from_str_radix(id.asset().color.trim_start_matches('#'), 16)
                .unwrap_or(0x2e8bff);
            if self.reduced {
                ring(
                    &mut image,
                    at.x - left,
                    at.y - top,
                    size * 0.545,
                    [
                        (rgb >> 16) as u8,
                        (rgb >> 8) as u8,
                        rgb as u8,
                        (230.0 * fade) as u8,
                    ],
                );
            }
            for delay in if self.reduced {
                &[][..]
            } else {
                &[0.0, 0.22, 1.4][..]
            } {
                let t = age - delay;
                if (0.0..0.65).contains(&t) {
                    ring(
                        &mut image,
                        at.x - left,
                        at.y - top,
                        size * 0.545 * (0.25 + 1.10 * t / 0.65),
                        [
                            (rgb >> 16) as u8,
                            (rgb >> 8) as u8,
                            rgb as u8,
                            (230.0 * (1.0 - t / 0.65) * fade) as u8,
                        ],
                    );
                }
            }
            let art_size = (size
                * if self.reduced {
                    1.0
                } else {
                    effect_pop(age, 0.28)
                }) as u32;
            for art in id.asset().art {
                if let Some(art) = self.asset(&format!("textures/{art}.png")) {
                    blit(
                        &mut image,
                        &art,
                        at.x - left,
                        at.y - top - (art_size as f64 * 0.23) as i32,
                        art_size,
                        fade,
                    );
                }
            }
        }
        if let Some(emote) = &self.emote {
            let age = now.duration_since(emote.started).as_secs_f64();
            let elapsed = now.duration_since(emote.frames_started).as_millis() as u32;
            let art = emote
                .frames
                .iter()
                .rev()
                .find(|f| !self.reduced && f.at_ms <= elapsed)
                .map(|f| Arc::new(f.image.clone()));
            let point = emote.point;
            let slug = emote.slug.clone();
            let art = art.or_else(|| self.asset(&format!("emotes/{slug}.png")));
            if let Some(art) = art {
                blit(
                    &mut image,
                    &art,
                    point.x - left,
                    point.y - top,
                    (settings.emote_size_px as f64
                        * 1.25
                        * if self.reduced {
                            1.0
                        } else {
                            effect_pop(age, 0.25)
                        }) as u32,
                    effect_fade(age, 3.0),
                );
            }
        }
        Some(Frame {
            x: left,
            y: top,
            image,
        })
    }
}
fn point(image: &mut RgbaImage, x: i32, y: i32, color: [u8; 4]) {
    if x >= 0 && y >= 0 && x < image.width() as i32 && y < image.height() as i32 {
        image.get_pixel_mut(x as u32, y as u32).blend(&Rgba(color));
    }
}
fn ring(image: &mut RgbaImage, x: i32, y: i32, radius: f64, color: [u8; 4]) {
    let steps = (radius * std::f64::consts::TAU).ceil().max(1.0) as usize;
    for i in 0..steps {
        let angle = i as f64 * std::f64::consts::TAU / steps as f64;
        for thickness in [-1.0, 0.0, 1.0] {
            point(
                image,
                x + (angle.cos() * (radius + thickness)) as i32,
                y + (angle.sin() * (radius + thickness)) as i32,
                color,
            );
        }
    }
}
fn blit(target: &mut RgbaImage, image: &RgbaImage, x: i32, y: i32, width: u32, opacity: f64) {
    let height = (width as u64 * image.height() as u64 / image.width().max(1) as u64).max(1) as u32;
    let mut image = imageops::resize(image, width.max(1), height, imageops::FilterType::Triangle);
    for pixel in image.pixels_mut() {
        pixel.0[3] = (pixel.0[3] as f64 * opacity.clamp(0.0, 1.0)) as u8;
    }
    imageops::overlay(
        target,
        &image,
        (x - width as i32 / 2) as i64,
        (y - height as i32 / 2) as i64,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_wheel_renders_visible_transparent_pixels_and_cancels() {
        let settings = Settings::default();
        let audio = Audio::new();
        let mut renderer = Renderer::new();
        renderer.action(
            QueuedAction {
                action: Action::WheelOpen {
                    wheel: WheelKind::Ping,
                    origin: Point::new(-100, 200),
                },
                origin: None,
                scale: 1.0,
            },
            &settings,
            &audio,
        );
        let frame = renderer.frame(&settings).unwrap();
        assert_eq!(frame.x, -330);
        assert_eq!(frame.y, -30);
        assert!(frame.image.pixels().any(|p| p.0[3] > 0));
        assert_eq!(frame.image.get_pixel(0, 0).0[3], 0);
        renderer.action(
            QueuedAction {
                action: Action::Cancel,
                origin: None,
                scale: 1.0,
            },
            &settings,
            &audio,
        );
        assert!(renderer.frame(&settings).is_none());
    }
}
