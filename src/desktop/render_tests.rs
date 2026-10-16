//! Headless evidence for the actual Unix native renderer, not a JS approximation.
use super::*;
use crate::{
    appearance::Theme,
    render_metrics::{self, Report},
};
use std::collections::BTreeMap;

fn wheel(scale: f64, slot: usize, blend: f64, now: Instant, theme: Theme) -> Renderer {
    let mut renderer = Renderer::new();
    let mut blends = [0.0; 8];
    blends[slot] = blend;
    renderer.wheel = Some(Wheel {
        kind: WheelKind::Ping,
        origin: Point::new(500, 500),
        hot: Some(slot),
        blend: blends,
        updated: now,
        scale,
        surface: crate::wheel::Surface::with_theme(scale, theme),
        scaled_art: Vec::new(),
    });
    renderer
}
fn ping(now: Instant) -> Renderer {
    let mut renderer = Renderer::new();
    renderer.pings.push(Ping {
        id: pingloom_core::PingId::AllIn,
        point: Point::new(500, 500),
        started: now,
    });
    renderer
}
fn emote(now: Instant) -> Renderer {
    let mut renderer = Renderer::new();
    renderer.emote = Some(Emote {
        point: Point::new(500, 500),
        started: now,
        frames_started: now,
        slug: "facepalm".into(),
        frames: decode_emote("facepalm").unwrap(),
        pending: None,
    });
    renderer
}
fn snapshot(renderer: &mut Renderer, settings: &Settings, time: Instant) -> String {
    let frame = renderer.frame_at(settings, time).unwrap();
    let mut bytes = Vec::new();
    bytes.extend(frame.x.to_le_bytes());
    bytes.extend(frame.y.to_le_bytes());
    bytes.extend(frame.image.width().to_le_bytes());
    bytes.extend(frame.image.height().to_le_bytes());
    bytes.extend(frame.image.as_raw());
    render_metrics::hash(&bytes)
}
#[test]
fn render_characterization() {
    let settings = Settings::default();
    let now = Instant::now();
    let mut hashes = BTreeMap::new();
    for scale in [1.0, 1.5, 2.0] {
        for slot in [0, 3] {
            let mut renderer = wheel(scale, slot, 1.0, now, Theme::DefaultDark);
            hashes.insert(
                format!("wheel_{scale}_slot_{slot}"),
                snapshot(&mut renderer, &settings, now),
            );
        }
    }
    hashes.insert(
        "wheel_mid_selection".into(),
        snapshot(
            &mut wheel(1.0, 3, 0.5, now, Theme::DefaultDark),
            &settings,
            now,
        ),
    );
    for age in [100, 700, 3200] {
        hashes.insert(
            format!("ping_{age}"),
            snapshot(&mut ping(now), &settings, now + Duration::from_millis(age)),
        );
    }
    hashes.insert(
        "emote_frame_250".into(),
        snapshot(&mut emote(now), &settings, now + Duration::from_millis(250)),
    );
    render_metrics::characterize(hashes, include_str!("../../tools/render_hashes_unix.json"));
}
#[test]
fn unix_warm_wheel_cache_has_zero_resizes_and_identical_pixels() {
    let now = Instant::now();
    let settings = Settings::default();
    for scale in [1.0, 1.5, 2.0] {
        let mut renderer = wheel(scale, 0, 0.5, now, Theme::DefaultDark);
        let cold = snapshot(&mut renderer, &settings, now);
        render_metrics::reset();
        let warm = snapshot(&mut renderer, &settings, now);
        assert_eq!(render_metrics::take().resizes, 0);
        assert_eq!(cold, warm);
        assert!(renderer.wheel.as_ref().unwrap().scaled_art.len() <= 160);
        renderer.cancel();
        assert!(renderer.wheel.is_none());
    }
}

#[test]
#[ignore = "explicit release performance evidence"]
fn render_benchmark() {
    let now = Instant::now();
    let settings = Settings::default();
    let mut report = Report::default();
    for (scale, name) in [(1.0, "wheel_1"), (1.5, "wheel_1_5"), (2.0, "wheel_2")] {
        let mut renderer = wheel(scale, 0, 1.0, now, Theme::DefaultDark);
        renderer.reduced = true;
        for slot in 0..8 {
            renderer.wheel.as_mut().unwrap().hot = Some(slot);
            renderer.frame_at(&settings, now);
        }
        report.scenario(name, |i| {
            renderer.wheel.as_mut().unwrap().hot = Some(i % 8);
            std::hint::black_box(
                renderer
                    .frame_at(&settings, now + Duration::from_millis(i as u64 * 33))
                    .unwrap(),
            );
        });
    }
    let mut renderer = ping(now);
    report.scenario("ping", |i| {
        std::hint::black_box(
            renderer
                .frame_at(&settings, now + Duration::from_millis(50 + i as u64 * 40))
                .unwrap(),
        );
    });
    let mut renderer = emote(now);
    report.scenario("emote", |i| {
        std::hint::black_box(
            renderer
                .frame_at(&settings, now + Duration::from_millis(i as u64 * 40))
                .unwrap(),
        );
    });
    report.finish();
}
