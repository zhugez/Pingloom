//! Offscreen Win32 Canvas: real draw code, no desktop window or injected input.
use super::*;
use crate::{
    appearance::Theme,
    render_metrics::{self, Report},
};
use std::collections::BTreeMap;

struct Scene {
    wheel: Option<WheelFx>,
    ping: Option<PingFx>,
    emote: Option<EmoteFx>,
    cache: HashMap<String, Arc<RgbaImage>>,
    items: Vec<String>,
    pixels: Vec<u8>,
}
impl Scene {
    fn new() -> Self {
        Self {
            wheel: None,
            ping: None,
            emote: None,
            cache: HashMap::new(),
            items: Settings::default()
                .wheel
                .iter()
                .map(|p| p.asset().icon.into())
                .collect(),
            pixels: Vec::new(),
        }
    }
    fn wheel(scale: f64, slot: usize, blend: f64, now: Instant) -> Self {
        let mut scene = Self::new();
        let mut selection = [0.0; 8];
        selection[slot] = blend;
        scene.wheel = Some(WheelFx {
            kind: WheelKind::Ping,
            origin: Point::new(960, 540),
            pointer: Point::new(960, 100),
            scale,
            hot: Some(slot),
            selection,
            opened: now,
            updated: now,
            caption_mask: None,
            surface: crate::wheel::Surface::with_theme(scale, Theme::DefaultDark),
            layer: None,
            scaled_art: Vec::new(),
        });
        scene
    }
    fn ping(now: Instant) -> Self {
        let mut scene = Self::new();
        scene.ping = Some(PingFx {
            id: PingId::AllIn,
            point: Point::new(960, 540),
            started: now,
        });
        scene
    }
    fn emote(now: Instant) -> Self {
        let mut scene = Self::new();
        let frames = decode_emote("facepalm").unwrap();
        let still = image::load_from_memory(&assets::get("emotes/facepalm.png").unwrap())
            .unwrap()
            .into_rgba8();
        scene.emote = Some(EmoteFx {
            slug: "facepalm".into(),
            point: Point::new(960, 540),
            size: 150,
            started: now,
            still,
            frames: Some(frames),
            animation_started: Some(now),
            animation_pending: false,
        });
        scene
    }
    fn draw(&mut self, settings: &Settings, now: Instant) {
        self.draw_in_box(
            settings,
            now,
            DrawBox {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            },
        );
    }
    fn draw_cropped(&mut self, settings: &Settings, now: Instant) {
        let pings = self.ping.as_slice();
        let desktop = DrawBox {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        let bounds = effect_box(
            self.wheel.as_ref(),
            pings,
            self.emote.as_ref(),
            settings,
            desktop,
        )
        .unwrap();
        self.draw_in_box(settings, now, bounds);
    }
    fn draw_in_box(&mut self, settings: &Settings, now: Instant, bounds: DrawBox) {
        self.pixels
            .resize(bounds.width() as usize * bounds.height() as usize * 4, 0);
        render_metrics::clear(self.pixels.len());
        self.pixels.fill(0);
        let mut canvas = Canvas {
            pixels: &mut self.pixels,
            width: bounds.width() as usize,
            height: bounds.height() as usize,
            left: bounds.left,
            top: bounds.top,
            opacity: 1.0,
        };
        if let Some(wheel) = &mut self.wheel {
            canvas.draw_wheel(wheel, &self.items, &mut self.cache);
        }
        if let Some(ping) = &self.ping {
            canvas.draw_ping(
                ping,
                settings,
                now.duration_since(ping.started).as_secs_f64(),
                &mut self.cache,
                false,
            );
        }
        if let Some(emote) = &self.emote {
            canvas.draw_emote(
                emote,
                emote.image_at(now, false),
                now.duration_since(emote.started).as_secs_f64(),
                false,
            );
        }
    }
}
#[test]
fn effect_bounds_match_full_desktop_raster_at_negative_origins_and_edges() {
    let settings = Settings {
        ping_size_px: 200,
        ..Settings::default()
    };
    let now = Instant::now();
    let desktop = DrawBox {
        left: -1920,
        top: -1080,
        right: 0,
        bottom: 0,
    };
    for edge in [false, true] {
        for mut scene in [
            Scene::wheel(2.0, 3, 0.5, now),
            Scene::ping(now),
            Scene::emote(now),
        ] {
            let center = if edge {
                Point::new(-1910, -1070)
            } else {
                Point::new(-960, -540)
            };
            if let Some(w) = scene.wheel.as_mut() {
                w.origin = center;
            }
            if let Some(p) = scene.ping.as_mut() {
                p.point = center;
            }
            if let Some(e) = scene.emote.as_mut() {
                e.point = center;
            }
            let pings: Vec<_> = scene.ping.take().into_iter().collect();
            let bounds = effect_box(
                scene.wheel.as_ref(),
                &pings,
                scene.emote.as_ref(),
                &settings,
                desktop,
            )
            .unwrap()
            .intersect(desktop)
            .unwrap();
            let mut cropped = vec![0; bounds.width() as usize * bounds.height() as usize * 4];
            let mut full = vec![0; 1920 * 1080 * 4];
            for (buffer, area) in [(&mut full, desktop), (&mut cropped, bounds)] {
                let mut canvas = Canvas {
                    pixels: buffer,
                    width: area.width() as usize,
                    height: area.height() as usize,
                    left: area.left,
                    top: area.top,
                    opacity: 1.0,
                };
                if let Some(wheel) = &mut scene.wheel {
                    canvas.draw_wheel(wheel, &scene.items, &mut scene.cache);
                }
                for ping in &pings {
                    canvas.draw_ping(ping, &settings, 0.1, &mut scene.cache, false);
                }
                if let Some(emote) = &scene.emote {
                    canvas.draw_emote(emote, emote.image_at(now, false), 0.0, false);
                }
            }
            let mut expanded = vec![0; full.len()];
            for y in 0..bounds.height() as usize {
                let offset = ((bounds.top - desktop.top) as usize + y) * 1920 * 4
                    + (bounds.left - desktop.left) as usize * 4;
                let width = bounds.width() as usize * 4;
                expanded[offset..offset + width]
                    .copy_from_slice(&cropped[y * width..(y + 1) * width]);
            }
            assert_eq!(
                full, expanded,
                "effect bounds must enclose every drawn pixel, including clipped negative monitors"
            );
        }
    }
}

#[test]
fn portrait_emotes_preserve_legacy_clipping_instead_of_truncating_artwork() {
    let now = Instant::now();
    let settings = Settings::default();
    let mut scene = Scene::new();
    scene.emote = Some(EmoteFx {
        slug: "custom_0000000000000001".into(),
        point: Point::new(960, 540),
        size: 150,
        started: now,
        still: RgbaImage::from_pixel(2, 8, image::Rgba([255, 0, 0, 255])),
        frames: None,
        animation_started: None,
        animation_pending: false,
    });
    let when = now + Duration::from_millis(300);
    scene.draw(&settings, when);
    let full = scene.pixels.clone();
    assert_ne!(
        full[(1000 * 1920 + 960) * 4 + 3],
        0,
        "portrait fixture must extend past a square Emote box"
    );
    scene.draw_cropped(&settings, when);
    assert_eq!(full, scene.pixels);
    let mut custom = settings.clone();
    custom.emote_wheel[0] = "custom_0000000000000001".into();
    let mut wheel = Scene::wheel(1.0, 0, 1.0, now).wheel.unwrap();
    wheel.kind = WheelKind::Emote;
    let desktop = DrawBox {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };
    assert_eq!(
        effect_box(Some(&wheel), &[], None, &custom, desktop),
        Some(desktop)
    );
}

#[test]
fn dirty_surface_clears_union_without_trails_and_reclaims_idle_storage() {
    unsafe {
        let instance = windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(null());
        let _ = register(instance); // Another parallel native test may already own the class.
        let mut overlay = Overlay::new(instance).unwrap();
        assert_eq!((overlay.width, overlay.height), (1, 1));
        assert_eq!(
            IsWindowVisible(overlay.hwnd),
            0,
            "offscreen test must never show an overlay"
        );
        overlay.desktop = DrawBox {
            left: -1920,
            top: -1080,
            right: 1920,
            bottom: 1080,
        };
        let bounds = DrawBox {
            left: -800,
            top: -600,
            right: -400,
            bottom: -200,
        };
        let first = overlay.prepare_frame(bounds).unwrap();
        assert_eq!((first.right, first.bottom), (400, 400));
        assert!(
            overlay.present(&first),
            "Win32 must accept an effect-sized negative-origin surface"
        );
        assert_eq!(IsWindowVisible(overlay.hwnd), 0);
        let len = overlay.width as usize * overlay.height as usize * 4;
        std::slice::from_raw_parts_mut(overlay.pixels, len).fill(255);
        overlay.drawn = Some(bounds);
        let smaller = DrawBox {
            left: -800,
            top: -600,
            right: -500,
            bottom: -300,
        };
        render_metrics::reset();
        let dirty = overlay.prepare_frame(smaller).unwrap();
        assert_eq!(
            (dirty.left, dirty.top, dirty.right, dirty.bottom),
            (0, 0, 400, 400)
        );
        assert_eq!(render_metrics::take().cleared, 400 * 400 * 4);
        assert!(std::slice::from_raw_parts(overlay.pixels, len)
            .iter()
            .all(|p| *p == 0));
        overlay.drawn = Some(smaller);
        // A second unchanged frame clears only the effect rows, not the retained capacity.
        render_metrics::reset();
        overlay.prepare_frame(smaller).unwrap();
        assert_eq!(render_metrics::take().cleared, 300 * 300 * 4);
        let rect = smaller.local(overlay.surface_box());
        assert!(
            overlay.present(&rect),
            "Win32 must accept a partial update on the retained DIB"
        );
        assert_eq!(IsWindowVisible(overlay.hwnd), 0);
        overlay.drawn = Some(smaller);
        overlay.tick(&Settings::default());
        assert_eq!((overlay.width, overlay.height), (1, 1));
        assert!(overlay.drawn.is_none());
        assert_eq!(IsWindowVisible(overlay.hwnd), 0);
    }
}

#[test]
fn baked_wheel_layer_matches_reference_through_selection_fade_clipping_and_theme_changes() {
    let now = Instant::now();
    for scale in [0.75, 1.0, 1.25, 1.333, 1.5, 2.0] {
        let mut scene = Scene::wheel(scale, 0, 1.0, now);
        let wheel = scene.wheel.as_mut().unwrap();
        // Reuse the same layer through every theme, proving invalidation too.
        for theme in [
            Theme::DefaultDark,
            Theme::DefaultLight,
            Theme::Latte,
            Theme::Frappe,
            Theme::Macchiato,
            Theme::Mocha,
        ] {
            wheel.surface = crate::wheel::Surface::with_theme(scale, theme);
            for (origin, opacity, selection) in [
                (Point::new(150, 160), 1.0, [0.0; 8]),
                (
                    Point::new(-70, -90),
                    1.0,
                    [0.4, 0.7, 0.0, 1.0, 0.0, 0.2, 0.0, 0.8],
                ),
                (Point::new(340, 260), 0.83, [0.5; 8]),
                (Point::new(1000, 1000), 1.0, [1.0; 8]),
            ] {
                wheel.origin = origin;
                wheel.selection = selection;
                let mut expected = vec![0; 320 * 280 * 4];
                let mut actual = expected.clone();
                let mut reference = Canvas {
                    pixels: &mut expected,
                    width: 320,
                    height: 280,
                    left: -30,
                    top: -20,
                    opacity,
                };
                wheel.surface.paint(&selection, |dx, dy, rgba| {
                    reference.point(origin.x + dx, origin.y + dy, rgba);
                });
                if let Some(labels) = &wheel.surface.labels {
                    let size = (crate::wheel::EXTENT * 2.0 * scale).round() as u32;
                    reference.image(
                        labels,
                        origin.x - size as i32 / 2,
                        origin.y - size as i32 / 2,
                        size,
                        1.0,
                    );
                }
                let mut cached = Canvas {
                    pixels: &mut actual,
                    width: 320,
                    height: 280,
                    left: -30,
                    top: -20,
                    opacity,
                };
                cached.draw_wheel(wheel, &[], &mut HashMap::new());
                assert_eq!(
                    actual, expected,
                    "scale={scale} theme={theme:?} origin={origin:?} opacity={opacity}"
                );
            }
            assert_eq!(wheel.layer.as_ref().unwrap().theme, theme);
        }
    }
}

#[test]
fn caption_gdi_resources_are_reused_per_scale_and_bounded() {
    unsafe {
        let instance = windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(null());
        let _ = register(instance);
        let mut overlay = Overlay::new(instance).unwrap();
        overlay.wheel_open(WheelKind::Ping, Point::new(0, 0), 1.0);
        let first = overlay
            .wheel
            .as_mut()
            .unwrap()
            .caption_mask
            .as_mut()
            .unwrap();
        let handles = (first.font, first.dc, first.bitmap, first.pixels);
        assert!(!first.font.is_null());
        first.caption = Some("retained-mask".into());
        overlay.wheel_close();
        overlay.wheel_open(WheelKind::Ping, Point::new(0, 0), 1.5);
        overlay.wheel_close();
        overlay.wheel_open(WheelKind::Emote, Point::new(-10, -10), 1.0);
        let reused = overlay
            .wheel
            .as_ref()
            .unwrap()
            .caption_mask
            .as_ref()
            .unwrap();
        assert_eq!(
            (reused.font, reused.dc, reused.bitmap, reused.pixels),
            handles
        );
        assert_eq!(reused.caption.as_deref(), Some("retained-mask"));
        for n in 0..12 {
            overlay.wheel_open(WheelKind::Ping, Point::new(0, 0), 0.25 + n as f64 * 0.05);
            overlay.wheel_close();
            assert!(overlay.caption_cache.len() <= 8);
        }
        assert_eq!(
            IsWindowVisible(overlay.hwnd),
            0,
            "resource checks must never show a native overlay"
        );
    }
}

#[test]
fn settled_wheel_key_ignores_pointer_but_invalidates_every_draw_input() {
    let now = Instant::now();
    let mut scene = Scene::wheel(1.0, 0, 1.0, now);
    let mut settings = Settings::default();
    let wheel = scene.wheel.as_mut().unwrap();
    let key = WheelDrawKey::new(wheel, &settings, false);
    assert!(!wheel.settled(now, false), "entrance must keep animating");
    let later = now + Duration::from_secs(2);
    assert!(wheel.settled(later, false));
    for n in 0..60 {
        wheel.pointer = Point::new(n, 100);
        wheel.advance(later + Duration::from_millis(n as u64 * 33), false);
        assert!(wheel.settled(later, false) && key.matches(wheel, &settings, false));
    }
    assert!(!key.matches(wheel, &settings, true));
    wheel.origin.x += 1;
    assert!(!key.matches(wheel, &settings, false));
    wheel.origin.x -= 1;
    wheel.scale = 1.5;
    assert!(!key.matches(wheel, &settings, false));
    wheel.scale = 1.0;
    settings.wheel.swap(0, 1);
    assert!(!key.matches(wheel, &settings, false));
    settings.wheel.swap(0, 1);
    wheel.hot = Some(1);
    wheel.advance(later + Duration::from_secs(1), false);
    assert!(!key.matches(wheel, &settings, false));
    for n in 1..=120 {
        wheel.advance(
            later + Duration::from_secs(1) + Duration::from_millis(n * 33),
            false,
        );
    }
    assert!(
        wheel.settled(later + Duration::from_secs(6), false),
        "a decaying old hot slot must eventually stop producing frames"
    );
    let key = WheelDrawKey::new(wheel, &settings, false);
    wheel.kind = WheelKind::Emote;
    assert!(!key.matches(wheel, &settings, false));
}

#[test]
fn settled_idle_blend_tail_is_byte_identical_without_snapping_animation() {
    let now = Instant::now();
    let settings = Settings::default();
    for scale in [1.0, 1.5, 2.0] {
        let mut scene = Scene::wheel(scale, 1, 1.0, now);
        scene.draw(&settings, now);
        let expected = render_metrics::hash(&scene.pixels);
        scene.wheel.as_mut().unwrap().selection[0] = 1e-12;
        scene.draw(&settings, now);
        assert_eq!(
            expected,
            render_metrics::hash(&scene.pixels),
            "idle tails must have identical surface/icon output before a signature may ignore them"
        );
    }
}

#[test]
fn render_characterization() {
    let settings = Settings::default();
    let now = Instant::now();
    let mut hashes = BTreeMap::new();
    for scale in [1.0, 1.5, 2.0] {
        for slot in [0, 3] {
            let mut scene = Scene::wheel(scale, slot, 1.0, now);
            scene.draw(&settings, now);
            hashes.insert(
                format!("wheel_{scale}_slot_{slot}"),
                render_metrics::hash(&scene.pixels),
            );
        }
    }
    let mut scene = Scene::wheel(1.0, 3, 0.5, now);
    scene.draw(&settings, now);
    hashes.insert(
        "wheel_mid_selection".into(),
        render_metrics::hash(&scene.pixels),
    );
    for age in [100, 700, 3200] {
        let mut scene = Scene::ping(now);
        scene.draw(&settings, now + Duration::from_millis(age));
        hashes.insert(format!("ping_{age}"), render_metrics::hash(&scene.pixels));
    }
    let mut scene = Scene::emote(now);
    scene.draw(&settings, now + Duration::from_millis(250));
    hashes.insert(
        "emote_frame_250".into(),
        render_metrics::hash(&scene.pixels),
    );
    render_metrics::characterize(
        hashes,
        include_str!("../../tools/render_hashes_windows.json"),
    );
}
#[test]
fn dynamic_gdi_caption_cache_preserves_pixels_on_this_os() {
    let now = Instant::now();
    let mut scene = Scene::wheel(1.0, 0, 1.0, now);
    scene.pixels.resize(1920 * 1080 * 4, 0);
    let wheel = scene.wheel.as_mut().unwrap();
    wheel.caption_mask = unsafe { CaptionMask::new(null_mut(), 1.0) };
    assert!(wheel.caption_mask.is_some());
    let mut canvas = Canvas {
        pixels: &mut scene.pixels,
        width: 1920,
        height: 1080,
        left: 0,
        top: 0,
        opacity: 1.0,
    };
    unsafe {
        draw_wheel_caption(&mut canvas, wheel, Some("Danger"));
    }
    assert!(canvas.pixels.iter().any(|p| *p != 0));
    let cold = render_metrics::hash(canvas.pixels);
    canvas.pixels.fill(0);
    unsafe {
        draw_wheel_caption(&mut canvas, wheel, Some("Danger"));
    }
    assert_eq!(cold, render_metrics::hash(canvas.pixels));
    assert_eq!(
        wheel.caption_mask.as_ref().unwrap().caption.as_deref(),
        Some("Danger")
    );
    canvas.pixels.fill(0);
    unsafe {
        draw_wheel_caption(&mut canvas, wheel, Some("Assist Me"));
    }
    assert_ne!(cold, render_metrics::hash(canvas.pixels));
}

#[test]
fn wheel_art_cache_preserves_frames_and_eliminates_warm_resizes() {
    let settings = Settings::default();
    let now = Instant::now();
    for scale in [1.0, 1.5, 2.0] {
        let mut scene = Scene::wheel(scale, 0, 0.5, now);
        scene.draw(&settings, now);
        let cold = render_metrics::hash(&scene.pixels);
        render_metrics::reset();
        scene.draw(&settings, now);
        assert_eq!(
            render_metrics::take().resizes,
            0,
            "warm Wheel must not resize art"
        );
        assert_eq!(render_metrics::hash(&scene.pixels), cold);
        assert!(!scene.wheel.as_ref().unwrap().scaled_art.is_empty());
        // A new Wheel owns no stale assets; closing/dropping its Wheel drops all resized art.
        assert!(Scene::wheel(scale, 0, 1.0, now)
            .wheel
            .unwrap()
            .scaled_art
            .is_empty());
    }
    let mut scene = Scene::wheel(1.0, 0, 1.0, now);
    let wheel = scene.wheel.as_mut().unwrap();
    let image = RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
    let other = RgbaImage::from_pixel(2, 2, image::Rgba([0, 255, 0, 255]));
    assert_ne!(
        wheel.icon("one", &image, 10).get_pixel(0, 0),
        wheel.icon("two", &other, 10).get_pixel(0, 0)
    );
    for size in 30..220 {
        wheel.icon("one", &image, size);
    }
    assert!(wheel.scaled_art.len() <= 160);
    assert!(
        wheel
            .scaled_art
            .iter()
            .map(|(_, i)| i.as_raw().len())
            .sum::<usize>()
            <= 8 * 1024 * 1024
    );
}

#[test]
#[ignore = "explicit release performance evidence"]
fn render_benchmark() {
    let settings = Settings::default();
    let now = Instant::now();
    let mut report = Report::default();
    for (scale, name) in [(1.0, "wheel_1"), (1.5, "wheel_1_5"), (2.0, "wheel_2")] {
        let mut scene = Scene::wheel(scale, 0, 1.0, now);
        // Warm every scenario's selected/base size before measuring steady frames.
        for slot in 0..8 {
            let wheel = scene.wheel.as_mut().unwrap();
            wheel.selection = [0.0; 8];
            wheel.selection[slot] = 1.0;
            scene.draw_cropped(&settings, now);
        }
        report.scenario(name, |i| {
            let wheel = scene.wheel.as_mut().unwrap();
            wheel.hot = Some(i % 8);
            wheel.selection = [0.0; 8];
            wheel.selection[i % 8] = 1.0;
            scene.draw_cropped(&settings, now);
            std::hint::black_box(&scene.pixels);
        });
    }
    let mut scene = Scene::ping(now);
    report.scenario("ping", |i| {
        scene.draw_cropped(&settings, now + Duration::from_millis(50 + i as u64 * 40));
        std::hint::black_box(&scene.pixels);
    });
    let mut scene = Scene::emote(now);
    report.scenario("emote", |i| {
        scene.draw_cropped(&settings, now + Duration::from_millis(i as u64 * 40));
        std::hint::black_box(&scene.pixels);
    });
    report.finish();
}
