//! Test-only render evidence: thread-local counters, fixed snapshots and explicit benchmarks.
use std::{cell::Cell, collections::BTreeMap};

#[derive(Clone, Copy, Default)]
pub(crate) struct Counters {
    pub resizes: u64,
    pub cleared: u64,
    pub written: u64,
}
thread_local! { static COUNTERS: Cell<Counters> = const { Cell::new(Counters { resizes: 0, cleared: 0, written: 0 }) }; }
pub(crate) fn reset() {
    COUNTERS.set(Counters::default());
}
pub(crate) fn take() -> Counters {
    COUNTERS.get()
}
pub(crate) fn resize() {
    let mut c = COUNTERS.get();
    c.resizes += 1;
    COUNTERS.set(c);
}
pub(crate) fn clear(bytes: usize) {
    let mut c = COUNTERS.get();
    c.cleared += bytes as u64;
    COUNTERS.set(c);
}
pub(crate) fn write(bytes: usize) {
    let mut c = COUNTERS.get();
    c.written += bytes as u64;
    COUNTERS.set(c);
}

pub(crate) fn hash(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    format!("{hash:016x}")
}

pub(crate) fn characterize(actual: BTreeMap<String, String>, expected: &str) {
    // Recording never passes as characterization: the caller must explicitly review
    // and copy the generated hashes, then rerun without this variable.
    if let Some(path) = std::env::var_os("PINGLOOM_RECORD_RENDER_HASHES") {
        std::fs::write(path, serde_json::to_string_pretty(&actual).unwrap() + "\n").unwrap();
        panic!("Recorded candidate render hashes; review them and rerun without PINGLOOM_RECORD_RENDER_HASHES");
    }
    let expected: BTreeMap<String, String> = serde_json::from_str(expected).unwrap();
    assert_eq!(
        actual, expected,
        "native frame changed; review visual output before updating hashes"
    );
}

#[derive(Default)]
pub(crate) struct Report(pub BTreeMap<String, f64>);
impl Report {
    pub(crate) fn scenario(&mut self, name: &str, mut draw: impl FnMut(usize)) {
        // Warm caches/allocator outside the measurement. State times stay fixed by callers.
        draw(0);
        let mut times = Vec::new();
        let mut sums = Counters::default();
        for frame in 0..60 {
            reset();
            let start = std::time::Instant::now();
            draw(frame);
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            let c = take();
            sums.resizes += c.resizes;
            sums.cleared += c.cleared;
            sums.written += c.written;
        }
        times.sort_by(f64::total_cmp);
        for (suffix, value) in [
            ("ms_p50", times[30]),
            ("ms_p95", times[56]),
            ("resize_count", sums.resizes as f64 / 60.0),
            ("cleared_bytes", sums.cleared as f64 / 60.0),
            ("written_bytes", sums.written as f64 / 60.0),
        ] {
            self.0.insert(format!("render_{name}_{suffix}"), value);
        }
    }
    pub(crate) fn finish(self) {
        let json = serde_json::to_string_pretty(&self.0).unwrap() + "\n";
        println!("{json}");
        if let Some(path) = std::env::var_os("PINGLOOM_RENDER_OUT") {
            std::fs::write(path, json).unwrap();
        }
    }
}

#[test]
fn fnv_and_counters_are_stable_and_thread_local() {
    assert_eq!(hash(b""), "cbf29ce484222325");
    assert_eq!(hash(b"hello"), "a430d84680aabd0b");
    reset();
    resize();
    clear(8);
    write(4);
    std::thread::spawn(|| {
        assert_eq!(take().resizes, 0);
        resize();
    })
    .join()
    .unwrap();
    assert_eq!((take().resizes, take().cleared, take().written), (1, 8, 4));
    reset();
}
