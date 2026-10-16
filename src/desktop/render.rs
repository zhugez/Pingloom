use super::{audio::Audio, QueuedAction};
#[cfg(test)]
use crate::animation::decode_emote;
use crate::{
    animation::AnimatedFrame,
    assets,
    logic::{effect_fade, emote_envelope, ping_art_motion, ring_coverage, selection_blend},
};
use image::{imageops, Pixel, Rgba, RgbaImage};
use pingloom_core::{Action, Point, Settings, WheelKind};
use std::{
    collections::HashMap,
    sync::{
        mpsc::{self, Receiver, TryRecvError},
        Arc,
    },
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
    surface: crate::wheel::Surface,
    scaled_art: crate::raster::ArtCache,
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
    /// Off-thread decode, so the owner loop keeps ticking input meanwhile.
    pending: Option<Receiver<Vec<AnimatedFrame>>>,
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
    pub fn is_active(&self) -> bool {
        self.wheel.is_some() || !self.pings.is_empty() || self.emote.is_some()
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
        let bytes = crate::custom_emotes::asset(path)
            .map(std::borrow::Cow::Owned)
            .or_else(|| assets::get(path))?;
        let image = Arc::new(image::load_from_memory(&bytes).ok()?.into_rgba8());
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
                if !settings.muted {
                    audio.warm();
                }
                self.wheel = Some(Wheel {
                    kind: wheel,
                    origin,
                    hot: None,
                    blend: [0.0; 8],
                    updated: Instant::now(),
                    scale: queued.scale,
                    surface: crate::wheel::Surface::new(queued.scale),
                    scaled_art: Vec::new(),
                });
                None
            }
            Action::WheelMove { slot, .. } => {
                if let Some(wheel) = &mut self.wheel {
                    if wheel.hot != slot && slot.is_some() && settings.tick_sound {
                        audio.play("sounds/button.ogg", volume * 0.5);
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
                    audio.play(format!("sounds/{}.ogg", id.asset().sound), volume);
                }
                WheelKind::Emote => {
                    let slug = slot
                        .map_or(&settings.click_emote_id, |slot| &settings.emote_wheel[slot])
                        .clone();
                    let draw_width = (f64::from(settings.emote_size_px) * 1.25) as u32;
                    let pending =
                        (!self.reduced && crate::animation::has_animation(&slug)).then(|| {
                            let (tx, rx) = mpsc::sync_channel(1);
                            let decoding = slug.clone();
                            // A failed spawn or decode disconnects: the still art then shows.
                            let _ = std::thread::Builder::new()
                                .name("pingloom-emote-decode".into())
                                .spawn(move || {
                                    let _ = tx.send(
                                        crate::animation::decode_emote_sized(&decoding, draw_width)
                                            .unwrap_or_default(),
                                    );
                                });
                            rx
                        });
                    let started = Instant::now();
                    self.emote = Some(Emote {
                        point,
                        started,
                        frames_started: started,
                        slug: slug.clone(),
                        frames: Vec::new(),
                        pending,
                    });
                    if settings.emote_sound {
                        audio.play(format!("emotes/{slug}.ogg"), volume);
                    }
                }
            }
        }
    }
    pub fn frame(&mut self, settings: &Settings) -> Option<Frame> {
        self.frame_at(settings, Instant::now())
    }
    fn frame_at(&mut self, settings: &Settings, now: Instant) -> Option<Frame> {
        let _trace = crate::trace::probe(crate::trace::Metric::Render);
        if let Some(emote) = &mut self.emote {
            let ready = emote.pending.as_ref().and_then(|rx| match rx.try_recv() {
                Ok(frames) => Some(frames),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(Vec::new()),
            });
            // Like Windows: the effect's lifetime starts when its art is ready.
            if let Some(frames) = ready {
                emote.frames = frames;
                emote.started = now;
                emote.frames_started = now;
                emote.pending = None;
            }
        }
        self.pings.retain(|p| {
            now.duration_since(p.started).as_secs_f64() < settings.ping_duration_s as f64
        });
        if self.emote.as_ref().is_some_and(|e| {
            e.pending.is_none() && now.duration_since(e.started) >= Duration::from_secs(3)
        }) {
            self.emote = None;
        }
        let mut areas = Vec::new();
        if let Some(w) = &self.wheel {
            areas.push((w.origin, (crate::wheel::EXTENT * w.scale).ceil() as i32 + 1));
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
        #[cfg(test)]
        crate::render_metrics::clear(width as usize * height as usize * 4);
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
            wheel.surface.refresh(wheel.scale);
            wheel.surface.paint(&wheel.blend, |dx, dy, rgba| {
                point(
                    &mut image,
                    wheel.origin.x - left + dx,
                    wheel.origin.y - top + dy,
                    rgba,
                );
            });
        }
        if let Some(wheel) = &self.wheel {
            let kind = wheel.kind;
            let scale = wheel.scale;
            let origin = wheel.origin;
            let blends = wheel.blend;
            if let Some(labels) = &wheel.surface.labels {
                blit(
                    &mut image,
                    labels,
                    origin.x - left,
                    origin.y - top,
                    (crate::wheel::EXTENT * 2.0 * scale).round() as u32,
                    1.0,
                );
            }
            for (slot, blend) in blends.into_iter().enumerate() {
                let angle = (slot as f64 * 45.0 - 90.0).to_radians();
                let path = match kind {
                    WheelKind::Ping => {
                        format!("textures/{}.png", settings.wheel[slot].asset().icon)
                    }
                    WheelKind::Emote => format!("emotes/{}.png", settings.emote_wheel[slot]),
                };
                if let Some(art) = self.asset(&path) {
                    let size = (46.0 * scale * (1.0 + 0.18 * blend)) as u32;
                    let art = crate::raster::cached_art(
                        &mut self.wheel.as_mut().unwrap().scaled_art,
                        &path,
                        &art,
                        size,
                    );
                    blit(
                        &mut image,
                        &art,
                        origin.x - left + (angle.cos() * crate::wheel::ICON_RADIUS * scale) as i32,
                        origin.y - top + (angle.sin() * crate::wheel::ICON_RADIUS * scale) as i32,
                        (46.0 * scale * (1.0 + 0.18 * blend)) as u32,
                        0.72 + 0.28 * blend,
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
            for (index, art) in id.asset().art.iter().enumerate() {
                let (x, y, scale, alpha) =
                    ping_art_motion(age, index, id.asset().art.len(), self.reduced);
                let art_size = (size * scale) as u32;
                if let Some(art) = self.asset(&format!("textures/{art}.png")) {
                    blit(
                        &mut image,
                        &art,
                        at.x - left + (size * x) as i32,
                        at.y - top - (size * 0.23) as i32 + (size * y) as i32,
                        art_size,
                        fade * alpha,
                    );
                }
            }
        }
        if let Some(emote) = &self.emote {
            let age = now.duration_since(emote.started).as_secs_f64();
            let elapsed = crate::animation::playback_ms(
                &emote.slug,
                &emote.frames,
                now.duration_since(emote.frames_started).as_millis() as u32,
            );
            let art = emote
                .frames
                .iter()
                .rev()
                .find(|f| !self.reduced && f.at_ms <= elapsed)
                .map(|f| Arc::clone(&f.image));
            let (scale, opacity) = emote_envelope(
                age,
                !emote.frames.is_empty(),
                self.reduced,
                emote.pending.is_some(),
            );
            let point = emote.point;
            let slug = emote.slug.clone();
            let art = art.or_else(|| self.asset(&format!("emotes/{slug}.png")));
            if let Some(art) = art {
                blit(
                    &mut image,
                    &art,
                    point.x - left,
                    point.y - top,
                    (settings.emote_size_px as f64 * 1.25 * scale) as u32,
                    opacity,
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
        #[cfg(test)]
        crate::render_metrics::write(4);
        image.get_pixel_mut(x as u32, y as u32).blend(&Rgba(color));
    }
}
fn ring(image: &mut RgbaImage, x: i32, y: i32, radius: f64, color: [u8; 4]) {
    let outer = radius + 2.0;
    let inner = (radius - 2.0).max(0.0);
    for dy in -(outer.ceil() as i32)..=outer.ceil() as i32 {
        let y2 = f64::from(dy).powi(2);
        let end = (outer * outer - y2).max(0.0).sqrt().floor() as i32;
        let start = (inner * inner - y2).max(0.0).sqrt().floor() as i32;
        for dx in start..=end {
            let coverage = ring_coverage(f64::from(dx).hypot(f64::from(dy)), radius, 3.0);
            let mut rgba = color;
            rgba[3] = (f64::from(color[3]) * coverage) as u8;
            point(image, x + dx, y + dy, rgba);
            if dx != 0 {
                point(image, x - dx, y + dy, rgba);
            }
        }
    }
}
fn blit(target: &mut RgbaImage, image: &RgbaImage, x: i32, y: i32, width: u32, opacity: f64) {
    let image = crate::raster::resize_art(image, width.max(1));
    let height = image.height();
    let image = if opacity == 1.0 {
        image
    } else {
        let mut owned = image.into_owned();
        for pixel in owned.pixels_mut() {
            pixel.0[3] = (pixel.0[3] as f64 * opacity.clamp(0.0, 1.0)) as u8;
        }
        std::borrow::Cow::Owned(owned)
    };
    #[cfg(test)]
    crate::render_metrics::write(image.width() as usize * image.height() as usize * 4);
    imageops::overlay(
        target,
        image.as_ref(),
        (x - width as i32 / 2) as i64,
        (y - height as i32 / 2) as i64,
    );
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod render_evidence;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scaled_transparent_art_keeps_its_edge_color() {
        let art = RgbaImage::from_fn(2, 1, |x, _| {
            if x == 0 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
        let mut target = RgbaImage::new(8, 4);
        blit(&mut target, &art, 4, 2, 8, 1.0);
        let edges: Vec<_> = target.pixels().filter(|p| p[3] > 0 && p[3] < 255).collect();
        assert!(!edges.is_empty(), "expected an antialiased edge");
        for pixel in edges {
            assert_eq!(
                pixel[0], 255,
                "transparent black must not darken the red edge: {pixel:?}"
            );
        }
    }

    #[test]
    fn native_motion_frames_are_visible_then_expire() {
        let settings = Settings::default();
        let start = Instant::now();
        let mut renderer = Renderer::new();
        renderer.wheel = Some(Wheel {
            kind: WheelKind::Ping,
            origin: Point::new(170, 210),
            hot: None,
            blend: [0.0; 8],
            updated: start,
            scale: 0.65,
            surface: crate::wheel::Surface::new(0.65),
            scaled_art: Vec::new(),
        });
        renderer.pings.push(Ping {
            id: pingloom_core::PingId::AllIn,
            point: Point::new(500, 220),
            started: start,
        });
        let frames = decode_emote(&settings.click_emote_id).unwrap();
        assert!(frames.len() > 1);
        renderer.emote = Some(Emote {
            point: Point::new(820, 210),
            started: start,
            frames_started: start,
            slug: settings.click_emote_id.clone(),
            frames,
            pending: None,
        });
        // Optional review frames are emitted by this exact native rasterizer, not a JS imitation.
        let export = std::env::var_os("PINGLOOM_MOTION_PREVIEW_DIR").map(std::path::PathBuf::from);
        if let Some(dir) = &export {
            std::fs::create_dir_all(dir).unwrap();
        }
        let count = if export.is_some() { 48 } else { 4 };
        for i in 0..count {
            let elapsed = Duration::from_millis(i * 3000 / count);
            renderer.wheel.as_mut().unwrap().hot = Some((i * 8 / count) as usize);
            let frame = renderer.frame_at(&settings, start + elapsed).unwrap();
            assert!(frame.image.pixels().any(|p| p.0[3] > 0));
            if let Some(dir) = &export {
                let mut canvas = RgbaImage::from_pixel(1000, 420, Rgba([34, 33, 39, 255]));
                imageops::overlay(&mut canvas, &frame.image, frame.x as i64, frame.y as i64);
                canvas.save(dir.join(format!("frame-{i:03}.png"))).unwrap();
            }
        }
        renderer.wheel = None;
        assert!(renderer
            .frame_at(&settings, start + Duration::from_secs(9))
            .is_none());
    }

    #[test]
    fn imported_custom_media_renders_animates_loops_and_respects_reduced_motion() {
        let directory = tempfile::tempdir().unwrap();
        crate::custom_emotes::initialize(&directory.path().join("settings.json"));
        let image = RgbaImage::from_pixel(4, 4, Rgba([12, 200, 34, 255]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let id = crate::custom_emotes::import(bytes.get_ref()).unwrap();
        let mut settings = Settings {
            click_emote_id: id.clone(),
            ..Settings::default()
        };
        settings.emote_wheel[0] = id.clone();
        settings.validate().unwrap();
        settings
            .save_atomic(directory.path().join("settings.json"))
            .unwrap();
        assert_eq!(
            Settings::load(directory.path().join("settings.json")).unwrap(),
            settings
        );
        let start = Instant::now();
        let mut renderer = Renderer::new();
        renderer.emote = Some(Emote {
            point: Point::new(200, 200),
            started: start,
            frames_started: start,
            slug: id,
            frames: Vec::new(),
            pending: None,
        });
        let frame = renderer
            .frame_at(&settings, start + Duration::from_millis(250))
            .unwrap();
        assert!(frame
            .image
            .pixels()
            .any(|p| p[0] == 12 && p[1] == 200 && p[2] == 34 && p[3] > 0));
        let mut gif = Vec::new();
        image::codecs::gif::GifEncoder::new(&mut gif)
            .encode_frames([
                image::Frame::from_parts(
                    RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 255])),
                    0,
                    0,
                    image::Delay::from_numer_denom_ms(70, 1),
                ),
                image::Frame::from_parts(
                    RgbaImage::from_pixel(4, 4, Rgba([0, 0, 255, 255])),
                    0,
                    0,
                    image::Delay::from_numer_denom_ms(90, 1),
                ),
            ])
            .unwrap();
        let slug = crate::custom_emotes::import(&gif).unwrap();
        let frames = decode_emote(&slug).unwrap();
        renderer.emote = Some(Emote {
            point: Point::new(200, 200),
            started: start,
            frames_started: start,
            slug,
            frames,
            pending: None,
        });
        for (ms, color) in [(50, [255, 0, 0]), (90, [0, 0, 255]), (180, [255, 0, 0])] {
            let frame = renderer
                .frame_at(&settings, start + Duration::from_millis(ms))
                .unwrap();
            assert!(frame.image.pixels().any(|p| p.0[0..3] == color && p[3] > 0));
        }
        renderer.reduced = true;
        let frame = renderer
            .frame_at(&settings, start + Duration::from_millis(90))
            .unwrap();
        assert!(frame
            .image
            .pixels()
            .any(|p| p[0] == 255 && p[1] == 0 && p[2] == 0 && p[3] > 0));
        assert!(!frame.image.pixels().any(|p| p[2] == 255 && p[3] > 0));
    }

    #[test]
    fn emote_decodes_off_thread_then_animates() {
        let settings = Settings::default();
        let mut renderer = Renderer::new();
        let click = QueuedAction {
            action: Action::Click {
                wheel: WheelKind::Emote,
                point: Point::new(200, 200),
            },
            origin: None,
            scale: 1.0,
        };
        renderer.action(click, &settings, &Audio::new());
        assert!(renderer.emote.as_ref().unwrap().pending.is_some());
        let deadline = Instant::now() + Duration::from_secs(30);
        while renderer.emote.as_ref().unwrap().pending.is_some() {
            assert!(Instant::now() < deadline, "decode never completed");
            renderer.frame_at(&settings, Instant::now());
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(renderer.emote.as_ref().unwrap().frames.len() > 1);
    }

    #[test]
    fn native_v7_wheel_preview_at_display_scales() {
        let settings = Settings::default();
        let start = Instant::now();
        let export = std::env::var_os("PINGLOOM_WHEEL_PREVIEW_DIR").map(std::path::PathBuf::from);
        if let Some(dir) = &export {
            std::fs::create_dir_all(dir).unwrap();
        }
        for scale in [1.0, 1.5, 2.0] {
            for (kind, name) in [(WheelKind::Ping, "pings"), (WheelKind::Emote, "emotes")] {
                let mut renderer = Renderer::new();
                renderer.reduced = true;
                renderer.wheel = Some(Wheel {
                    kind,
                    origin: Point::new(0, 0),
                    hot: Some(0),
                    blend: [0.0; 8],
                    updated: start,
                    scale,
                    surface: crate::wheel::Surface::new(scale),
                    scaled_art: Vec::new(),
                });
                let clock = Instant::now();
                let frame = renderer.frame_at(&settings, start).unwrap();
                eprintln!("V7 {name} scale {scale}: {:?}", clock.elapsed());
                let x = -frame.x;
                let y = -frame.y;
                assert_eq!(
                    frame
                        .image
                        .get_pixel(x as u32, (y - (94.0_f64 * scale).round() as i32) as u32)[3],
                    0
                );
                assert_eq!(
                    &frame
                        .image
                        .get_pixel(x as u32, (y - (216.5_f64 * scale).round() as i32) as u32)
                        .0[..3],
                    &[226, 180, 189]
                );
                assert_eq!(
                    frame
                        .image
                        .get_pixel(
                            (x + (30.0_f64 * scale).round() as i32) as u32,
                            (y + (40.0_f64 * scale).round() as i32) as u32
                        )
                        .0,
                    [26, 23, 27, 250]
                );
                assert!(renderer.wheel.as_ref().unwrap().surface.labels.is_some());
                if let Some(dir) = &export {
                    frame
                        .image
                        .save(dir.join(format!("{name}-{scale}.png")))
                        .unwrap();
                }
                renderer.cancel();
                assert!(renderer.frame_at(&settings, start).is_none());
            }
        }
    }

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
        assert_eq!(frame.x, -331);
        assert_eq!(frame.y, -31);
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
