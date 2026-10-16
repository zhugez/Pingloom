//! Opt-in local atomic histograms. Hook probes never allocate, lock or write files.
use std::{
    io::Write,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{Duration, Instant},
};
static ENABLED: AtomicBool = AtomicBool::new(false);
const N: usize = 7;
struct Histogram {
    buckets: [AtomicU64; 32],
    max: AtomicU64,
}
impl Histogram {
    const fn new() -> Self {
        Self {
            buckets: [const { AtomicU64::new(0) }; 32],
            max: AtomicU64::new(0),
        }
    }
}
static METRICS: [Histogram; N] = [const { Histogram::new() }; N];
#[derive(Clone, Copy)]
#[allow(dead_code)] // Hook/owner metrics are platform-specific.
pub(crate) enum Metric {
    Mouse,
    Keyboard,
    Render,
    FrameTick,
    WheelOpen,
    SettingsBoot,
    Save,
}
static EVENTS: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];
#[derive(Clone, Copy)]
#[allow(dead_code)] // Overlay failures are currently Windows-specific.
pub(crate) enum Event {
    AudioInitFailure,
    OverlayFailure,
}
pub(crate) fn event(event: Event) {
    if ENABLED.load(Ordering::Relaxed) {
        EVENTS[event as usize].fetch_add(1, Ordering::Relaxed);
    }
}
fn write_events(file: &mut impl Write, counters: &[AtomicU64; 2]) -> std::io::Result<()> {
    for (name, counter) in ["audio_init_failure", "overlay_failure"]
        .iter()
        .zip(counters)
    {
        let count = counter.swap(0, Ordering::Relaxed);
        if count != 0 {
            writeln!(file, "event={name} count={count}")?;
        }
    }
    Ok(())
}

pub(crate) struct Probe {
    metric: Metric,
    start: Option<Instant>,
}
pub(crate) fn probe(metric: Metric) -> Probe {
    Probe {
        metric,
        start: ENABLED.load(Ordering::Relaxed).then(Instant::now),
    }
}
impl Drop for Probe {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            let micros = start.elapsed().as_micros().min(u64::MAX as u128) as u64;
            let h = &METRICS[self.metric as usize];
            let bucket = (63 - micros.max(1).leading_zeros() as usize).min(31);
            h.buckets[bucket].fetch_add(1, Ordering::Relaxed);
            h.max.fetch_max(micros, Ordering::Relaxed);
        }
    }
}
pub(crate) fn initialize() {
    let Some(path) = std::env::var_os("PINGLOOM_TRACE") else {
        return;
    };
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(mut file) => {
            if std::thread::Builder::new()
                .name("pingloom-local-trace".into())
                .spawn(move || loop {
                    std::thread::sleep(Duration::from_secs(10));
                    for (name, h) in [
                        "mouse_hook",
                        "keyboard_hook",
                        "render",
                        "frame_tick",
                        "wheel_open",
                        "settings_boot",
                        "save",
                    ]
                    .iter()
                    .zip(&METRICS)
                    {
                        let buckets: Vec<u64> = h
                            .buckets
                            .iter()
                            .map(|n| n.swap(0, Ordering::Relaxed))
                            .collect();
                        let count: u64 = buckets.iter().sum();
                        if count == 0 {
                            continue;
                        }
                        let quantile = |q: f64| {
                            let goal = (count as f64 * q).ceil() as u64;
                            let mut seen = 0;
                            for (i, n) in buckets.iter().enumerate() {
                                seen += n;
                                if seen >= goal {
                                    return (1u64 << (i + 1)) - 1;
                                }
                            }
                            0
                        };
                        let _ = writeln!(
                            file,
                            "{name} count={count} p50_us_upper={} p95_us_upper={} max_us={}",
                            quantile(0.5),
                            quantile(0.95),
                            h.max.swap(0, Ordering::Relaxed)
                        );
                    }
                    let _ = write_events(&mut file, &EVENTS);
                    let _ = file.flush();
                })
                .is_ok()
            {
                ENABLED.store(true, Ordering::Relaxed);
            }
        }
        Err(error) => eprintln!("Local trace unavailable: {error}"),
    }
}
#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "explicit opt-in trace writer evidence; requires PINGLOOM_TRACE and waits 11 seconds"]
    fn enabled_trace_writer_appends_metrics_and_failure_events() {
        use super::*;
        let path = std::env::var_os("PINGLOOM_TRACE").expect("set a disposable local trace path");
        let before = std::fs::metadata(&path).map_or(0, |m| m.len() as usize);
        initialize();
        assert!(ENABLED.load(Ordering::Relaxed));
        for _ in 0..3 {
            drop(probe(Metric::Render));
        }
        event(Event::AudioInitFailure);
        event(Event::OverlayFailure);
        std::thread::sleep(Duration::from_secs(11));
        let output = std::fs::read_to_string(path).unwrap();
        let output = &output[before..];
        assert!(output.contains("render count=3 "));
        assert!(output.contains("event=audio_init_failure count=1"));
        assert!(output.contains("event=overlay_failure count=1"));
    }
    #[test]
    fn failure_events_are_emitted_once_per_batch_and_reset() {
        use super::*;
        let counters = [AtomicU64::new(2), AtomicU64::new(1)];
        let mut out = Vec::new();
        write_events(&mut out, &counters).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "event=audio_init_failure count=2\nevent=overlay_failure count=1\n"
        );
        let mut out = Vec::new();
        write_events(&mut out, &counters).unwrap();
        assert!(out.is_empty());
        let before = EVENTS[0].load(Ordering::Relaxed);
        event(Event::AudioInitFailure);
        assert_eq!(before, EVENTS[0].load(Ordering::Relaxed));
    }
    #[test]
    fn disabled_probes_do_not_record() {
        let p = super::probe(super::Metric::Render);
        assert!(p.start.is_none());
    }
}
