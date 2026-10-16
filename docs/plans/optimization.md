# Optimization plan

- Status: Broad implementation in progress; see step ledger below. Not all steps are complete, and no optimization release has been accepted.
- Date: 2026-10-16
- Decisions: [ADR 0003](../adr/0003-measured-optimization.md)
- Version: stays 0.1.0. Steps land on `main`; the owner decides when to ship.

## Scope

Targets, in priority order:

1. Render CPU while the overlay draws a Wheel, Ping or Emote.
2. Idle cost: timers, wakeups, resident memory.
3. Download size.
4. Input latency on the Trigger and Gesture path. Measured first, changed only where cheap.
5. Startup time and code cleanup, only where cheap.

Platforms: Windows and macOS. Linux gains only through shared code (`src/desktop/`, `src/animation.rs`, `src/raster.rs`, the shared audio module). Linux-only items are listed in the backlog and are not scheduled.

Rules that apply to every step:

- One PR per step, on `main`, with CI green including the perf gate.
- No new crates. None of the steps below needs one. Add a crate only when measurements show the need and neither std nor an existing dependency covers it.
- The portable `pingloom.exe` stays a single file.
- The overlay stays native: no WebView, no updater, no startup registration, no sharing, no telemetry upload.
- Historical timings below are Linux proxies from the original audit, not current V7 measurements. Windows still needs targeted real-PC confirmation for changed paths. A physical Mac Mini is now available by operator-authorized SSH; physical Option-drag Wheel/Ping behavior was verified after the modifier fix. Record exactly which optimization was tested; do not treat earlier smoke evidence as acceptance for later changes.
- Never inject synthetic input (`tools/windows_gesture_smoke.ps1`) on a Windows desktop while someone is using it.
- Terms follow `CONTEXT.md`: Ping, Emote, Wheel, Trigger, Gesture, Toggle shortcut, Pause.

## Current implementation notes

- Step 0.1: data-driven size/counter vs timing/RAM rules, readable missing/zero-baseline handling, extra JSON merge, package/UI/asset bytes, job summaries and stdlib regression tests are implemented locally. The baseline is verified against CI run 38818302853 for `168c1d9`. CI run/throwaway asset-growth branch acceptance remains pending; local tests verify asset growth fails without publishing any test branch.
- Step 0.2: actual offscreen render benchmarks and 11-state characterization tests exist for Windows/Unix. Hashes passed on physical Windows, Linux and arm64 Mac Mini; release benchmarks measured all five scenarios. Deterministic counters are seeded; frame timings remain pending first CI baseline. Fixed hub/compass captions are covered; the dynamic Windows GDI action caption is not yet hash-locked due to OS-font differences. See [performance evidence](../performance.md) for exact limits and measurements.
- Step 0.3: read-only 10-second Windows idle sampler and CI metric merge are implemented. Host PowerShell syntax passed, but execution policy blocked runtime sampling; no policy was changed. Runtime acceptance remains pending CI.
- Step 0.4 histograms, probes and failure events are implemented locally; the physical latency checkpoint is still pending. Step 1.1 exact-size Windows Wheel art caching is implemented locally: 0 resizes per warm frame vs 8 before, all eleven Windows golden hashes unchanged, cache bounded to 160 entries / 8 MiB and owned by the open Wheel. Native Windows regression tests and a release benchmark passed. Timings were noisy; do not claim a stable frame-time speedup before CI/controlled confirmation. Animation, native input and renderer pixel output remain unchanged.
- V7 already caches shared native Wheel surface geometry and rasterized labels per open/theme change. Step 1.3 now reuses that geometry for a Windows premultiplied idle layer including labels, replacing only highlighted texels. Entrance fade retains the original per-primitive alpha rounding; theme changes rebuild the layer. The V7 appearance remains intact.
- Keep artwork, animation, theme synchronization, native architecture and 0.1.0 intact. Establish current characterization frames before further renderer changes.
- Linux is Coming soon: public packages/release CI are paused, source backend retained. Shared-code local Linux tests may still run.

## Step ledger (current checkout)

`Implemented` below means code/local tests, **not** CI or physical-release acceptance. Do not mark this plan fully done from compilation alone.

| Steps | State / evidence |
|---|---|
| 0.1 | Implemented; 4 stdlib gate tests, verified package baselines. New bench-ci timing/RSS/UI baselines await first green CI. |
| 0.2 | Implemented offscreen benchmark and fixed-frame hashes on Windows/Linux/Mac; dynamic GDI caption has same-OS cold/warm equality test, not portable font hashes. CI pending. |
| 0.3 | Sampler wired into Windows CI; PowerShell parsed, local runtime blocked by policy (unchanged). |
| 0.4 | Local atomic histograms/probes and audio/overlay failure events implemented; hook/render/open/boot/save wiring, no network. Native trace-writer test passed; physical latency checkpoint still pending. |
| 1.1 | Exact-size bounded cache on Windows and Unix; warm 0-resize regressions and unchanged hashes. |
| 1.2 | Windows circle row spans implemented; actual Windows raster hashes passed unchanged in the latest batch. Old filled_circle was already removed by V7. |
| 1.3 | Implemented: Windows premultiplied idle layer includes fixed labels; changed slot texels replace idle pixels with exactly composited highlights/labels. Reuses V7 geometry, caches per open/theme, frees on close, preserves entrance fade. Native Windows hashes and six-theme/fractional-scale/clipping/fade parity tests pass. Local bench-ci Wheel p95 1.63/3.75/6.39ms at 1x/1.5x/2x; CI and physical frame/hook trace pending. |
| 1.4–1.6 | Implemented: dirty-rect presentation, effect-sized Win32 DIB/display debounce and settled-Wheel input-signature frame skip. Native Windows hidden-window tests cover partial presentation, negative coordinates, clipping, clearing/reclamation and unchanged raster hashes. Portrait/custom art retains full-desktop clipping to preserve legacy output. Physical multi-monitor/z-order/DPI acceptance pending. |
| 1.7 | Dynamic caption redraw cached by name; font/DC/DIB/mask reused per exact scale (8 cached scales maximum), invalidated on desktop geometry change. Duplicate DPI lookup removed. Native handle-reuse/cache-bound and same-OS cold/warm caption equality tests pass; no portable OS-font hash claim. |
| 1.8 | Pending physical hook/frame trace after Windows rendering work; no worker/thread split authorized or added. |
| 2.1 | Windows frame timer now armed while effects are active; render paths arm/stop centrally, first tick posted once. Compile/raster tests only; GUI smoke pending CI. |
| 2.2 | Unix owner wake channel + 33/250ms active/idle wait implemented; macOS long run-loop wait with synchronized shutdown stop. Mac compile/tests passed; wakeup/physical gesture timing acceptance pending. |
| 2.3 | Bounded audio queue, lazy/warm output, idle close and retry backoff implemented. Policy tests pass; actual unplug/replug and first-play latency pending hardware. |
| 2.4–2.5 | Held-key-only Windows native resync, fewer Settings clones, Unix idle motion fast-path and busy flag implemented. Native tests/Clippy pass; 1kHz/missed-wake stress pending. |
| 2.6 | Implemented: watchdog starts on Settings open, retires only after lifecycle/barrier work closes, and reuses the lifecycle lock for race-free worker admission/retirement. Spawn failure rolls back opening before enqueue. Lifecycle regression passes on Windows/Linux; manual open/close/heartbeat-expiry and wakeup measurement remain pending. |
| 3.1 | Removed 19 canonical-metadata orphans (566,125 bytes); asset inventory regression passes. |
| 3.2 | Blocked: no original local League extraction source established. No lossy rebake, FPS reduction or animation-quality downgrade performed. |
| 3.3 | Asset facade + default embed-assets feature and macOS Resources configuration implemented. Physical Mac arm64 external-resource build, bundle-layout asset test, and missing-resource fail-fast passed; universal signed CI ZIP acceptance pending. |
| 3.4 | Asset growth gate exists; 42MB/1.25MB absolute limits blocked by 3.2 (current valid assets exceed those targets). Do not set impossible budgets or silently delete artwork. |
| 3.5 | opt-level experiment/cargo-bloat comparison not run; release opt-level and panic policy unchanged. |
| 4.1 | Zero-copy WebP conversion, Arc frame ownership and <=4 resize workers implemented. Byte parity/no-upscale/memory reduction tests pass. Decode+resize latency budget still needs CI. |
| 4.2 | Atomic thumbnail sidecars at import/lazy legacy reads and mtime refresh implemented; excluded from IDs/library bytes. No delete API exists to attach sidecar cleanup to. |
| 4.3 | Unix clones Arc handles; opacity-1 blits borrow image bytes instead of cloning a frame. Raster hashes unchanged locally. |
| 4.4 | Optional cancellation of superseded decode workers not implemented; there was no existing cancellation signal in this checkout. |
| 5.1 | Direct owned CGDataProvider/CGImage → NSImage path implemented; physical Mac PNG-vs-CGImage pixel test (<=1/channel), lifetime tests and Clippy passed. GUI presentation timing/visual acceptance pending. |
| 5.2 | Fullscreen probe gated by actual engine eligibility; ordinary-click/disabled/game_check regression test passes. |
| 5.3 | Shared geometry/icon caches, identical-frame presentation skip and reduced-motion query throttling implemented; physical Retina acceptance pending. Unix had no dynamic action-caption mask to cache. |
| 5.4 | macOS CGEventSource cached per input thread; trust remains checked per replay for revocation safety. Linux-only replay rewrite not scheduled while distribution is paused. |
| 6.1–6.3 | Metadata-only exists, catalog spawn_blocking and status-only owner request implemented. Existing FIFO/completion tests retained; full desktop lifecycle acceptance pending CI. |
| 7.1–7.2 | Main/tag filters, markdown ignores, concurrency and bench-ci profile implemented. docs/** is deliberately not ignored because docs/media/brand is imported into the UI. |
| 7.3 | PR-specific macOS LTO/size-gate experiment not implemented; signed-release profile unchanged. |
| 7.4 / 7.6 | Dev line tables/dependency debug=0 and main-only cache saves implemented; UI checks/fmt run once on Windows (Linux release CI stays disabled). |
| 7.5 | Not applicable: Linux public release jobs were removed at the owner's request. |
| 8.1 | Pending: dead native Settings module not deleted before Windows Phase 1 acceptance. |

## Historical baseline (not the current perf gate)

Build `6739725`, CI run 38756974425:

| Platform | `binary_bytes` | Decode p50 / p95 | Peak RSS | Artifacts |
|---|---|---|---|---|
| Windows | 67,940,352 | 105.0 / 120.2 ms | 87.7 MiB | installer 57.3 MB, portable exe 67.9 MB |
| Linux | 70,288,768 | – / 69.6 ms | 90.6 MiB | deb 58.2 MB, tarball 58.0 MB |
| macOS (universal) | 138,485,648 | 83 / 103.8 ms | 151.5 MiB | zip 122.2 MB |

- Embedded assets total 57,611,930 B. Emote WebPs are 54,507,938 B of that: 48 files, 320x320, 74–90 frames, 3 s each. ALPH chunks are 59.7% of the WebP bytes and VP8 the rest.
- The Windows exe is mostly data: `.rdata` 59.7 MB against `.text` 7.8 MB.
- The macOS universal binary embeds the assets once per slice: x86_64 69.9 MB plus arm64 69.2 MB.
- CI wall time: Windows 534 s, Linux 298 s, macOS 699 s, of which the universal bundle takes 515 s.
- `tools/perf_baseline.json` is `{}`, so nothing is gated today.

Hot spots (Linux proxies):

- **Wheel frame:** 14.0 ms at scale 1 (20.4 ms with 8 icons), 44.6 ms at scale 1.5, 70.4 ms at scale 2. At scale 1.5 and above it overruns the 33 ms frame period.
- **Art resize:** 0.23–5.5 ms per call, every frame, uncached.
- **Full-desktop DIB clear:** 0.46 ms at 8 MB (1080p) up to 4.83 ms at 66 MB. The `UpdateLayeredWindow` cost has not been measured.
- **Idle Windows owner thread:** about 30 frame-timer wakeups/s plus 2/s from the watchdog. The audio output stream also plays silence forever, estimated at about 100 wakeups/s on WASAPI.
- **Decoded animated Emote:** 28.9 MiB resident.
- **macOS:** a PNG encode and decode round trip costs about 4 ms per frame at scale 1.

## Phases

| Phase | Theme | Steps | Needs first |
|---|---|---|---|
| 0 | Measurement | 0.1–0.4 | – |
| 1 | Windows render hot path | 1.1–1.8 | 0.2 |
| 2 | Idle cost | 2.1–2.6 | 0.3 for the evidence |
| 3 | Download size | 3.1–3.5 | 0.1 |
| 4 | Emote decode and memory | 4.1–4.4 | 0.1 |
| 5 | macOS and shared Unix | 5.1–5.4 | 0.2 |
| 6 | Settings I/O | 6.1–6.3 | – |
| 7 | CI | 7.1–7.6 | – |
| 8 | Cleanup | 8.1 | Phase 1 merged |

Order: 0, 1, 2, 3, 4, 5. Phases 6 and 7 can go anywhere in between. Phase 8 goes last. Within a phase, the steps run in the order listed.

## Expected outcome

| Metric | Now | After | Confidence |
|---|---|---|---|
| Wheel frame, scale 1 / 2 (proxy) | 14.0 / 70.4 ms | 3–5x faster | medium, audit estimate |
| Art resizes per steady frame | every icon, every frame | 0 after warm-up | high |
| Bytes cleared per frame (1080p) | 8,294,400 | effect box only | high |
| Resident overlay DIB | 8–63 MiB | box-sized, a few MiB | medium |
| Windows idle wakeups | about 30/s + 2/s + audio | 2/s | high (count), Windows pass for power |
| Animated Emote resident | 28.9 MiB | about 9.9 MiB at default size | high |
| Windows portable exe | 67.9 MB | about 50–54 MB | medium |
| macOS zip | 122.2 MB | about 66 MB after 3.3, about 50 MB with 3.2 | medium |
| macOS paint per frame | about 4 ms | under 1 ms | medium, no Mac |
| CI, docs-only push | 361 / 246 / 423 s | skipped | high |

## Phase 0: Measurement

Each later step proves its win against these instruments, so they land first.

### 0.1 Harden the perf gate, track artifacts, seed the baseline

- Audit: MEAS-1, overlay-16, GATE-PACKAGE-UI-SETTINGS. Effort S.
- Files: `tools/perf_gate.py`, `tools/perf_baseline.json`, `.github/workflows/desktop.yml` (gate step).
- Approach:
  - Turn the metric list into data: `(key, limit %, fatal)`.
  - A key that is in the baseline but missing from the current run prints `::error::` with the key name; today that path raises `KeyError`. A key missing from both is skipped. A zero baseline is skipped with a notice instead of dividing by zero.
  - Accept extra JSON files after `<out.json>` and merge them. This keeps the current usage working and lets 0.2 and 0.3 add metrics.
  - Record artifact bytes: installer, portable exe, deb, tarball, macOS zip (from `release/`), and the `ui/dist` total. These are fatal above 5%.
  - Seed `perf_baseline.json`, keyed by platform, from verified Windows/macOS artifacts of green run 38818302853 (`168c1d9`). Linux distribution is paused (Coming soon), so no Linux release baseline is required. Seed `ui_dist_bytes` from the first green run of this step; it was not recorded by the previous CI and must not be guessed from a local build.
- Accept:
  - The job summary shows a comparison table on all three OSes.
  - A throwaway branch that adds a 5 MB file to `assets/` fails the gate on every OS. Do not merge it.
  - A bench JSON with a missing key produces a readable error, not a traceback.
- Risk: runner noise. Timings stay warn-only at 10%; sizes and counters are fatal.
- ADR: [ADR 0003](../adr/0003-measured-optimization.md), decision 1.

### 0.2 Render benchmark and characterization test

- Audit: overlay-16, MEAS-1. Effort M.
- Files: `src/windows/overlay.rs` (tests), `src/desktop/render.rs` (tests), `.github/workflows/desktop.yml` (nextest filter), `tools/perf_gate.py` (keys).
- Approach:
  - Add an ignored `render_benchmark` per backend, next to `decode_benchmark` (`src/animation.rs`).
    - Windows: it drives the overlay's own `Canvas` draw code over a `Vec<u8>` the size of a 1920x1080 desktop, with no window.
    - Unix: it drives `Renderer::frame_at`.
  - Scenarios: Wheel at scale 1.0, 1.5 and 2.0 with 8 icons and the hot slot cycling, 60 frames each; a Ping fading; an Emote animating.
  - Report `frame_ms` p50/p95 per scenario.
  - Also report deterministic counters per frame, via `cfg(test)` counters: art resizes and bytes cleared or written. Write the results to `PINGLOOM_RENDER_OUT`.
  - Gating: counters are fatal above 5%; milliseconds only warn.
  - Add a non-ignored characterization test that hashes the frame bytes for fixed states: each Wheel scale, hot slot 0 and 3, mid-fade, caption, a Ping, and an Emote frame. Use a 10-line inline FNV-1a, because `DefaultHasher` output is not stable across toolchains.
- Accept:
  - Both tests are green on `main` before any Phase 1 change.
  - The bench keys appear in the `perf-*.json` artifacts.
- Risk: low; test-only code.

### 0.3 Windows idle footprint in the smoke test

- Audit: overlay-16, IDLE-1. Effort S.
- Files: `.github/workflows/desktop.yml` (Windows runtime smoke step), `tools/perf_gate.py`.
- Approach:
  - After the existing start-up wait, sample `Get-Process` twice, 10 s apart, with no input: `TotalProcessorTime`, `WorkingSet64`, `PrivateMemorySize64`.
  - Write `idle.json` (`idle_cpu_ms_per_10s`, `idle_ws_mib`, `idle_private_mib`) and pass it to the gate as an extra JSON. All three are warn-only.
- Accept: the values appear in `perf-windows.json` and in the job summary.
- Caveats:
  - The CI runner may have no audio device. The audio thread then exits early (see 2.3), so CI understates the audio cost.
  - `peak_rss_mib` is the test process peak and will not fall in proportion to 1.5 or 4.1.
  - A real-PC pass confirms these numbers.

### 0.4 Opt-in local trace (`PINGLOOM_TRACE`)

- Audit: FIELD-TRACE-LOGGING, INPUT-TELEMETRY-GATE. Effort M. Optional, but step 1.8 and the real-PC passes depend on it.
- Files: new `src/trace.rs`, `src/main.rs`, `src/windows/mod.rs` (hook procs, frame timer), `src/windows/overlay.rs` (`wheel_open`), `src/desktop/mod.rs`, `src/windows/audio.rs` (init failure).
- Approach:
  - Read `PINGLOOM_TRACE=<file>` once at startup into a static.
  - When it is unset, every probe costs one relaxed atomic load.
  - When it is set, each probe adds its `Instant` delta to a fixed log2 microsecond histogram held in atomics:
    - mouse hook callback
    - keyboard hook callback
    - frame render
    - frame-timer ticks
    - Wheel open
    - Settings boot
    - Save
  - A writer thread, started only when the variable is set, appends one line every 10 s with count, p50, p95 and max. It also writes one-off events: audio init failure, overlay failure.
  - Local file only: no UI, no network. Inside hook procs: no allocation, no lock, no I/O.
- Accept:
  - Unset: no file I/O, and the render and decode benches are unchanged within noise.
  - Set, on a Windows PC: the file gains lines during play.
- Risk: the probes sit in the low-level hook procs. Keep each to an `Instant` pair and atomic adds.
- ADR: an additive change to the Windows owner; ADR 0003 covers it.

## Phase 1: Windows render hot path

Every step in this phase must:

- keep the 0.2 characterization hashes unchanged, or justify each difference with a tolerance test of at most 1 per channel;
- pass the Windows gesture smoke test (Wheel above 50,000 changed px, Ping above 2,000, over an always-on-top window);
- keep the owner-thread contract of ADR 0001.

ADR 0002 says the Windows renderer "must remain intact". ADR 0003 reads that as same output and same threading, which allows these in-place changes.

### 1.1 Cache resized art

- Audit: overlay-03 (high). Effort S.
- Files: `src/windows/overlay.rs`.
- Approach:
  - `Canvas::image` calls `raster::resize_art` for every icon on every frame (`src/raster.rs:5`; the filter lives in `resize_art_to` at `src/raster.rs:16`).
  - Add an overlay-owned cache: a `Vec` of `((asset key, w, h), Arc<RgbaImage>)` with a linear scan, capped at about 128 entries and cleared on `wheel_close` and on display change.
  - Icon size is `46 * scale * (1 + 0.18 * selection)`, so the size changes during the selection animation. The sizes are integers, though: about 9 sizes per icon at scale 1 and 17 at scale 2. The exact-size cache therefore warms within one animation and stays pixel-identical, with no snapping to endpoint sizes.
  - Keep the premultiplied filter. `sampling_preserves_native_pixels_and_ignores_hidden_rgb` must stay green.
  - Animated Emote frames are handled by 4.1, not by this cache.
- Accept:
  - After warm-up, the resize counter is 0 per steady Wheel frame.
  - Characterization hashes are unchanged.
  - The `frame_ms` of the 8-icon Wheel at scale 1 drops by the resize share: 0.23–0.3 ms per 128→46 icon in the audit.
- Risk: low. Memory is bounded: at scale 2, 136 entries of about 47 KB, about 6 MB, freed on close.

### 1.2 Row spans for circles

- Audit: overlay-01. Effort S.
- Files: `src/windows/overlay.rs`.
- Approach: `circle` (`overlay.rs:710`) and `filled_circle` (`:726`) scan the full bounding square and call `hypot` for every pixel. Instead, compute each row's x-span from the inner and outer radius. Pixels inside the span get the same `ring_coverage`, so the output stays identical.
- Accept: hashes unchanged; `frame_ms` drops by the square-scan share (2.7–7.2 ms in the proxy).
- Risk: low; off-by-one at the span ends shows up in the hashes.

### 1.3 Static Wheel layer and slot lookup table

- Audit: overlay-01, WIN-RASTER-BAKE (high). Effort M.
- Files: `src/windows/overlay.rs`.
- Approach:
  - On `wheel_open`, build two things once per scale:
    - a table of slot index and coverage for each pixel in the Wheel box, which removes `hypot`, `atan2` and the f64 blend per pixel per frame (128,818 px at scale 1, 289,840 at 1.5, 515,271 at 2);
    - a premultiplied BGRA static layer holding the parts that do not change while the Wheel is open.
  - Each frame copies the static layer's rows, then blends only the hot-slot highlight, icons and caption.
  - Free both on `wheel_close`.
  - Do not share constants or code with `src/desktop/render.rs:232-256`; that renderer differs.
- Accept:
  - Hashes unchanged.
  - Wheel `frame_ms` drops at least 3x at every scale; expect 3–5x, not 10x. Scale 2 lands under the 33 ms frame period on the CI runner.
- Measure: `render_benchmark` at scale 1, 1.5 and 2. On the real PC, the trace frame-time p95.
- Risk: medium; this is the largest renderer diff. The static layer at scale 2 is about 2 MB while the Wheel is open.

### 1.4 Dirty-rect present

- Audit: WIN-FRAME-DIRTY (high), overlay-02. Effort S–M.
- Files: `src/windows/overlay.rs`.
- Approach:
  - Remember the previous frame's drawn box.
  - Each frame, clear only the rows of the union of the previous and current boxes, not the whole desktop DIB.
  - Present with `UpdateLayeredWindowIndirect`, passing `prcDirty` set to that union and a null `hdcDst`.
  - Compute effect boxes from the same geometry the draw code uses, caption included.
- Accept:
  - The bytes-cleared counter equals the union area times 4, not 8,294,400 at 1080p.
  - A frame-sequence test ends with an all-zero buffer after the last effect, so no trails remain.
  - The smoke test passes.
- Measure: this step yields the first real `UpdateLayeredWindow` cost, from the trace on the real PC, single monitor and two monitors.
- Risk: low to medium. A missed box leaves stale pixels; the zero-buffer test catches that.

### 1.5 Size the overlay to the effect box; display-change fixes

- Audit: MEM-1 (high), overlay-02, WIN-DISPLAY-REBUILD. Effort M.
- Files: `src/windows/overlay.rs`, `src/windows/mod.rs` (`handle_display_change`, `:1062`).
- Approach:
  - `Canvas` already carries `left`, `top`, `width` and `height` and maps coordinates in `point()` (`overlay.rs:680-697`).
  - Create the DIB and window at 1x1.
  - Grow the DIB to the union box of active effects, clamped to the virtual desktop. Recreate it only when the box grows, or when it shrinks below 1/4 of the current area. Position it via `pptDst` and `psize`.
  - This removes 8–63 MiB of residency that 1.4 alone does not.
  - Display change, today:
    - `handle_display_change` builds the new `Overlay` before dropping the old one, so peak memory doubles.
    - It rebuilds on every message in a `WM_DISPLAYCHANGE` burst.
  - Display change, after:
    - Drop the old overlay first.
    - Debounce with a one-shot timer of about 250 ms.
    - Skip the rebuild when the virtual-screen rect is unchanged.
- Accept:
  - Windows idle private bytes from 0.3 drop by about the old DIB size.
  - The gesture smoke test passes, including the always-on-top case: the z-order raise from `6739725` must still apply whenever the window is shown.
  - Wheel and Ping draw correctly on a monitor left of or above the primary (negative coordinates).
- Measure: 0.3 on CI. On the user's two-monitor Windows 11 PC: Task Manager memory and the trace.
- Risk: medium. The window now moves and resizes per Gesture, which brings z-order, DPI virtualization and multi-monitor edge cases. This step needs a real two-monitor pass.
- ADR: ADR 0003.

### 1.6 Skip identical Wheel frames

- Audit: overlay-14. Effort S.
- Files: `src/windows/overlay.rs`, `src/windows/mod.rs`.
- Approach:
  - When only the Wheel is visible, build a signature from the draw inputs: kind, hot slot, settled blend, fade done, scale, origin, `reduced_motion`.
  - If it equals the last presented signature, skip the draw and the present.
  - `wheel.pointer` is never read while drawing, so pointer moves within one slot produce no frame.
  - Keep the tick in `handle_action`. WheelOpen, Move and Cancel arrive through `update_wheel_state`, not `handle_action`.
- Accept: a counter test shows 0 presented frames once the Wheel has settled on a slot. Hashes unchanged.
- Risk: low. Derive the signature from the struct the draw code reads, so no field is missed.

### 1.7 Wheel-open GDI work and caption cache

- Audit: WIN-WHEEL-OPEN-GDI, overlay caption mask. Effort S.
- Files: `src/windows/overlay.rs`, `src/windows/mod.rs`, `src/gesture.rs`.
- Approach:
  - `Action::WheelOpen` carries the DPI scale already computed at PointerDown (`mod.rs:1210`), so `update_wheel_state` (`mod.rs:582`) drops its second `dpi_scale_at`.
  - Cache the `HFONT` and `CaptionMask` per scale in `Overlay`, created on first need and invalidated on display change. `wheel_open` (`overlay.rs:316`) runs on the input path via `apply_decision`, so its cost lands on the hook.
  - The caption today runs `DrawTextW` four times, plus a full mask scan, on every frame. Redraw it only when the selected name or the scale changes.
- Accept:
  - GDI object creations per Wheel open: 0 after the first open (counter).
  - Hashes unchanged.
- Risk: low. Invalidate the caches when the scale or display changes.

### 1.8 Hook-latency checkpoint (decision, no code)

- Audit: overlay-15, WIN-HOOK-THREAD-SPLIT. Deferred.
- After Phases 1 and 2, run the 0.4 trace on the real Windows PC. Windows removes a slow low-level hook after about 300 ms (`LowLevelHooksTimeout`).
- Propose a render worker only if:
  - mouse or keyboard hook callback p95 is above 5 ms, or
  - the max is above 50 ms during ordinary play.
- A render worker means amending ADR 0001; moving the hooks off the owner thread is not proposed. Otherwise close the item.

## Phase 2: Idle cost

### 2.1 Run the frame timer only while something is visible

- Audit: IDLE-1, WIN-IDLE-TIMER, overlay-06, overlay-08. Effort S.
- Files: `src/windows/mod.rs`, `src/windows/overlay.rs`.
- Approach:
  - Today `TIMER_FRAME` (33 ms) is set at `mod.rs:228` and never killed.
  - `Overlay::tick` returns whether anything is still active.
  - The owner keeps a `frame_timer_on` flag, because `SetTimer` with an existing id restarts the period rather than being a no-op.
  - After each owner message that can show something, make one check and arm the timer if needed, then render the first frame inline. The messages:
    - WheelOpen in `update_wheel_state`
    - Select or Click in `handle_action`
    - `WM_FRAMES`
    - Settings apply (`mod.rs:813`, `:1038`)
  - When `tick` reports idle: `KillTimer`, then `SW_HIDE` once. Track visibility so `SW_HIDE` is not repeated every tick (`overlay.rs:451`).
  - Replace `self.settings().clone()` at `mod.rs:854` with the disjoint borrow `self.overlay.tick(self.engine.settings())`.
  - In `update_wheel_state`, copy `tick_sound`, `muted` and `volume` instead of cloning `Settings`.
- Accept:
  - Idle `WM_TIMER` rate is 2/s (watchdog only), shown by the trace counter.
  - The smoke check "overlay hidden while idle" passes, and so do the gesture checks.
  - An Emote animation plays to its last frame.
  - Reduced motion is unchanged.
- Measure: the 0.3 idle CPU. On the real PC, `powercfg /energy` or WPA wakeups as confirmation.
- Risk: a show path that skips the check freezes the frame until the next input. A single check point after dispatch, rather than per call site, avoids that.

### 2.2 Idle Unix owner loop and macOS run loop

- Audit: overlay-07, IDLE-5. Effort M.
- Files: `src/desktop/mod.rs`, `src/desktop/input/macos.rs`.
- Approach:
  - `pingloom-native-owner` sleeps 33 ms forever. Switch it to `recv_timeout`: 33 ms while the renderer is active, about 250 ms when idle.
  - Waking it is the tricky part. The non-Windows mailbox wake is a `fn(isize) -> bool` (`|_| true`) and cannot capture a channel, so use a `static OnceLock<Sender<()>>`.
  - Send a wake when input opens a Wheel or spawns an effect, and when a Settings request arrives.
  - macOS: the event-tap thread polls `CFRunLoop::run_in_mode(.., 100 ms, ..)` in a loop (`macos.rs:308-313`). Run it with a long timeout, and have shutdown stop the stored run loop instead of polling `running`.
- Accept:
  - Idle owner wakeups at or below 4/s, down from about 30/s (`pidstat -w` on Linux).
  - A unit test shows that a Shared input which opens a Wheel sends a wake.
  - The first Wheel frame arrives within one 33 ms period.
- Risk: medium; a missed wake delays the Wheel. There is no Mac for Activity Monitor "Idle Wake Ups"; release note.
- ADR: none. The single-window Unix overlay from ADR 0002 is unchanged.

### 2.3 Audio: open lazily, close when idle, recover from device loss

- Audit: IDLE-3, AUDIO-DEVICE-LIFETIME. Effort S–M.
- Files: `src/windows/audio.rs` (shared by all platforms).
- Approach:
  - The thread loops on `recv_timeout`.
  - Do not open an `OutputStream` until the first play, or until a warm-up message sent on Wheel open so the first tick sound is not late.
  - On failure, retry on a later play with a 5 s backoff. Today `OutputStream::try_default()` failure makes the thread return for good (`audio.rs:18-20`).
  - After about 30 s with no play and no live sink, drop the sinks first, then the stream.
  - Prune finished voices on each timeout.
- Accept:
  - Idle with no recent sound: no open stream after 30 s. On Windows no Pingloom session appears in the Volume Mixer; on Linux `pactl list sink-inputs` is empty.
  - Start with no output device, then connect one: sounds play.
  - Unplug the default device during play: sound recovers.
- Measure: the 0.3 idle CPU. Trace play-to-ready latency on the first play.
- Risk: first-play latency, probably tens of ms and unmeasured. The Wheel-open warm-up hides it for tick sounds and the following Ping or Emote sound.

### 2.4 Re-query only held keys on each click

- Audit: IDLE-2, WIN-RESYNC-PER-CLICK. Effort S.
- Files: `src/windows/mod.rs` (`resync_native_keys`).
- Approach:
  - On PointerDown, call `GetAsyncKeyState` only for vks marked in `native_keys_down`. Keep the full sweep on the 500 ms watchdog.
  - Use a disjoint borrow instead of the `Settings` clone at `mod.rs:569`.
- Accept: a unit test on the key selection; the gesture smoke test passes.
- Risk: low; the watchdog sweep still catches anything missed.

### 2.5 Unix: skip idle pointer moves

- Audit: MAC-IDLE-POINTER-LOCK, LINUX-POINTER-QUERY. Effort S.
- Files: `src/desktop/mod.rs`, `src/desktop/input/macos.rs`, `src/desktop/input/linux.rs`.
- Approach:
  - Add an `AtomicBool gesture_busy` to `Shared`. Set it under the state lock whenever the engine enters Pending or Wheel, and clear it otherwise.
  - In the macOS tap callback, return early for an idle `MouseMoved` before `location()` and before taking the lock.
  - Linux uses the same flag to skip `display.pointer()` on idle moves; button queries and release catch-up stay.
  - Keep the `last_pointer_ms` semantics the watchdog relies on.
- Accept:
  - A gesture unit test shows that the first drag move still opens the Wheel.
  - Under 1 kHz mouse motion, lock acquisitions per second at idle are 0 (debug counter).
- Risk: a flag published out of step with the engine state could drop the first drag move. That is why it is set and cleared under the lock.

### 2.6 Settings watchdog only while Settings is open (optional)

- Audit: IDLE-4. Effort S. Do this last.
- Files: `src/windows/desktop_settings.rs` (`spawn_watchdog`, `:683`; `schedule_open`, `:779`), `src/desktop/mod.rs:257`, `src/main.rs`.
- Approach:
  - Start `pingloom-settings-watchdog` from `schedule_open`, guarded by an `AtomicBool`. The loop exits when there is no active generation and no pending resume.
  - Avoid a lost re-arm: clear the flag, then re-check the state, then exit. No `Condvar`.
- Accept: the existing lifecycle tests pass. Manual open, close and heartbeat-expiry still work. Idle wakeups drop by 1/s.
- Risk: low.

## Phase 3: Download size

### 3.1 Remove unreferenced assets

- Audit: F4. Effort S.
- Files: `assets/textures/` (9 files, 137,717 B), `assets/sounds/` (10 files, 428,408 B: Attack, Click, Danger, Defend, GetBack, OnMyWay_Close, OnMyWay_Far, OnMyWay_caster, OnMyWay_recipient, SRP_13), `src/model.rs` (test).
- Approach:
  - First add a test that every file in `assets/textures` and `assets/sounds` is referenced from `src/model.rs`.
  - The test lists the 19 orphans; `git rm` them.
  - No extractor change is needed.
- Accept: `binary_bytes` falls by about 566 KB on every OS; all tests pass.
- Risk: low; the test guards against future orphans.

### 3.2 Re-bake Emotes (tier D)

- Audit: F1 (high), ASSET-1. Effort M, plus a League session.
- Files: `tools/extract-assets/emote_bake.py`, `assets/emotes/*.webp` (48), `tools/extract-assets/README.md:20`.
- Approach:
  - In `emote_bake.py:124-141`:
    - `CANVAS` 320 → 256
    - `WEBP_QUALITY` 80 → 70
    - `WEBP_ALPHA_QUALITY` 90 → 80 (no lower: glows band)
    - `EDGE_FEATHER_PX` 40 → 32
  - Keep `FPS` at 30.
  - Re-run `extract_assets.py --league <install> --emotes`. It needs Python 3.14, Pillow, NumPy and a local League install.
  - In the same session, optionally re-encode Ogg at q3 instead of q5, saving about 0.39 MB.
- Optional tier E: `FPS` 24 saves a further 21–25 MB in total.
  - Update the `at_ms == 33` assertions at `src/animation.rs:250` and `src/windows/overlay.rs:1166`.
  - Eyeball facepalm, woah_dizzy and now_see_here.
- Accept:
  - Emote WebPs total about 37–41 MB, down from 54.5 MB, and each Emote is at most 1.25 MB.
  - Side-by-side visual check at the default size 150 (drawn at 187 px) and at 300, on dark and light backgrounds: no banding, no clipped glow.
  - Decode p95 is no worse.
- Measure: gate `binary_bytes` falls by about 14–18 MB per slice; decode bench.
- Risk:
  - Quality. Emote size 80–300 draws at `size * 1.25`, so sizes above 204 now upscale a 256 source. Today upscaling starts at 256. Check at 300 and at high DPI.
  - About 55 MB of binary churn in git.

### 3.3 macOS: ship assets once as bundle Resources

- Audit: F2 (high). Effort M.
- Files:
  - `Cargo.toml`
  - `src/windows/assets.rs`
  - the `ASSETS` call sites: `src/animation.rs`, `src/custom_emotes.rs`, `src/desktop/render.rs`, `src/windows/desktop_settings.rs`, `src/windows/audio.rs`, `src/windows/overlay.rs`, and `src/windows/settings_window.rs` (gone if 8.1 lands first)
  - new `tauri.macos.conf.json`
  - `.github/workflows/desktop.yml`
  - `README.md:179`
  - `docs/adr/0002-experimental-native-unix-backends.md` (pointer to ADR 0003)
- Approach:
  - Add a default-on feature `embed-assets = ["dep:include_dir"]` and move `dep:include_dir` out of `desktop`.
  - Add an accessor `assets::get(path) -> Option<Cow<'static, [u8]>>` plus a directory listing for tests, and route the 9 non-test call sites (7 files) through it.
  - With the feature on: `include_dir!`, as today.
  - With it off: read from `current_exe()/../Resources/assets`.
  - Startup fails fast with an error when that directory is missing, so the existing macOS smoke test catches a broken bundle.
  - Add `tauri.macos.conf.json` with `bundle.resources` mapping `assets/` to `assets/`.
  - The macOS release build passes `--no-default-features --features desktop,custom-protocol`. Windows and Linux keep embedding, so the portable exe stays one file.
  - Add a CI step that fails if `Pingloom.app/Contents/Resources/assets/emotes/facepalm.webp` is missing, and run `codesign --verify --deep --strict` after the ad-hoc sign so the Resources are sealed.
  - Update `README.md:179`, the "Embedded assets ... ~55 MiB" note. From-source builds (`README.md:134`) keep the default feature.
- Accept: the bundle check and the smoke test pass for both slices.
- Measure:
  - The macOS binary drops from about 139 MB to about 25 MB.
  - The zip drops from about 122 MB to about 66 MB (−47%).
- Fallback with zero code: ship two per-arch zips of about 61 MB each.
- Risk:
  - Running the binary outside the bundle with the feature off finds no assets; the fail-fast catches that.
  - No physical Mac; release note.
- ADR: amends ADR 0002 ("embedded assets") for macOS only. See ADR 0003.

### 3.4 Asset budget

- Audit: F3. Effort S.
- Files: `tools/perf_gate.py`, `tools/perf_baseline.json`.
- Approach:
  - After 3.1–3.3, refresh the baseline on purpose.
  - Add `assets_bytes` (the sum of `assets/`), which matters because the macOS `binary_bytes` no longer covers assets after 3.3.
  - Add absolute budgets independent of the baseline, both fatal: assets at most about 42 MB in total, and each Emote at most 1.25 MB. Today the largest is poro_ride at 1,531,840 B.
- Accept: a throwaway branch with a 5 MB asset fails, and so does one with a 1.3 MB Emote.

### 3.5 Size-optimized code (experiment, throwaway branch)

- Audit: F5. Effort S.
- Approach:
  - Run `cargo bloat --release --crates` first.
  - Try `[profile.release] opt-level = "s"` with `[profile.release.package.libwebp-sys] opt-level = 3`.
  - Keep it only if it saves at least 1 MB and decode p95, render `frame_ms` and RSS each stay within 10%.
  - `.text` is only 7.8 MB, so the odds are low.
- `panic = "abort"` is not tried. Today a panic on the owner thread unwinds, its tray icon is dropped, and the owner-watch thread (`src/main.rs:60-91`) exits Tauri. With abort the process dies with a ghost tray icon. It needs the owner's approval first.

## Phase 4: Emote decode and memory

### 4.1 Zero-copy WebP frames, pre-sized in the worker

- Audit: DECODE-FRAME-COPY, MEM-2 (medium). Effort M.
- Files: `Cargo.toml` (`webp-animation` features `["static", "image"]`; `image` is already in the tree, so no new crate), `src/animation.rs`, `src/windows/overlay.rs`, `src/desktop/render.rs`.
- Approach:
  - Read the dimensions and timestamp first, then call `frame.into_rgba_image()`, which moves the buffer. This replaces `RgbaImage::from_raw(.., frame.data().to_vec())` at `animation.rs:119` and saves 29–35 MiB of memcpy per Emote decode.
  - Pass the decode worker the draw size: `emote_size_px * 1.25` times the DPI scale at the spawn point.
  - When that is smaller than the source, resize each frame once in the worker with the existing `resize_art_to`. Never upscale.
  - A serial resize would add 111–122 ms. Split the frames across `std::thread::scope` workers (`available_parallelism`, capped at 4) so the added latency stays under about 30 ms.
  - The renderer then hits `resize_art`'s borrowed path, with no per-frame resize.
  - A size change mid-animation still draws correctly through the existing per-frame resize.
- Accept:
  - Resident memory per animated Emote falls from 28.9 MiB to about 9.9 MiB at the default size.
  - Decode plus resize p95 is at most today's decode p95 plus 30 ms.
  - A test shows a pre-sized frame equals `resize_art(frame, size)` byte for byte.
- Risk: thread fan-out on 2-core machines; cap it with `available_parallelism`.

### 4.2 Custom Emote thumbnails generated once

- Audit: WIN-EMOTE-SPAWN-IO (medium), SET-CUSTOM-CATALOG-UNCACHED. Effort M.
- Files: `src/custom_emotes.rs`, `src/windows/overlay.rs` (`spawn_emote`, `:404`).
- Approach:
  - Today every read of a GIF or WebP custom Emote decodes it, fits it to 512 px and PNG-encodes it (`custom_emotes.rs:53-70`). That happens on each spawn, on the Windows owner thread (about 95 ms for a 2048 px source), and for every custom id on every Settings open.
  - Write a `<id>.thumb.png` sidecar at import; `import_into` already has the decoded frames.
  - Legacy libraries get the sidecar written on first read. Regenerate it when the source mtime is newer.
  - Exclude sidecars from `ids()` and from the 32 MiB library cap, and delete a sidecar together with its Emote.
  - Built-in stills go through the existing `Arc<RgbaImage>` asset cache.
- Accept:
  - The second and later spawns of a custom GIF or WebP Emote do not decode on the owner thread (trace `spawn_emote` ≤ 1 ms).
  - Tests: the sidecar is not listed, is removed with its Emote, and leaves the cap unchanged.
- Risk: low. Keep the ID path-safety rules unchanged.

### 4.3 Unix frames behind `Arc`

- Audit: overlay-05. Effort S.
- Files: `src/desktop/render.rs`, `src/animation.rs`.
- Approach: hold the decoded frames behind an `Arc` so the renderer stops cloning about 400 KB per frame.
- Accept: the desktop `render_benchmark` shows no per-frame copy (bytes-written counter). Hashes unchanged.

### 4.4 Cancel superseded decodes (optional)

- Audit: MEM-3. Effort S.
- Files: `src/animation.rs` (`decode_media` frame loops; WebP at `:115-131`).
- Approach: check the existing cancel signal inside the frame loop so a replaced Emote stops decoding. Do not add a decoded-frame cache.

## Phase 5: macOS and shared Unix

### 5.1 Overlay paint through CGImage instead of PNG

- Audit: overlay-04 (medium). Effort M.
- Files: `src/desktop/paint.rs` (macOS path).
- Approach:
  - Replace the per-frame PNG encode and decode (about 1.33 ms plus 2.85 ms at scale 1) with this path:
    - a `CGDataProvider` over the frame bytes
    - a `CGImage` (8 bpc, 32 bpp, premultiplied-last, sRGB)
    - `NSImage initWithCGImage:size:`
    - the existing `NSImageView`
  - Premultiply once per frame.
  - Skip the update when the frame is unchanged.
  - This keeps ADR 0002's AppKit `NSImageView`; `CALayer` contents is not chosen.
  - Uses the existing `core-graphics` and `objc2` dependencies.
- Test: a pixel-compare unit test on the macOS CI runner.
  - Render fixed frames through the old (PNG) and the new (CGImage) path, draw each `NSImage` into a same-size bitmap context, and compare the bytes, allowing at most 1 per channel for premultiply rounding.
  - It needs no Accessibility permission.
- Accept: the pixel test is green on macOS CI; macOS paint time is under 1 ms per frame in a headless bench.
- Note: `scale_at` returns 1.0 on macOS (`src/desktop/mod.rs:132-135`), so the overlay renders at 1x on Retina. This step does not change that. Size the CGImage in points so the result matches today's.
- Risk: color-space differences show only on a real display. Release notes must say it is untested on a physical Mac.
- ADR: ADR 0003.

### 5.2 Gate the fullscreen check

- Audit: MACOS-FULLSCREEN-UNGATED (medium), LINUX-FULLSCREEN-RTT (gating part). Effort S.
- Files: `src/desktop/mod.rs`, `src/desktop/input/macos.rs`, `src/desktop/input/linux.rs`.
- Approach:
  - Call `fullscreen_at` only when the press could start a Gesture and `game_check` is on. The predicate is `starts_gesture && Idle && enabled && !foreign_key && ping != emote`, as Windows already does at `mod.rs:1203`.
  - Evaluate the predicate under the lock, release the lock, probe, then decide again under the lock with the result.
- Accept: a unit test with a counting probe shows non-Trigger clicks never probe; the gesture tests are unchanged.
- Risk: low.

### 5.3 Shared Unix render caches

- Audit: overlay-10 to overlay-13, RENDER-1 (low). Effort S–M.
- Files: `src/desktop/render.rs`.
- Approach:
  - Cache the painter geometry per scale.
  - Query reduced motion once per second.
  - Cache the caption alpha mask keyed by kind, hot slot, scale and settings generation.
  - Precompute item and name keys.
- Accept: desktop `render_benchmark` p95 drops; hashes unchanged.

### 5.4 Cheaper replay setup

- Audit: UNIX-REPLAY-IN-CALLBACK, safe subset only. Effort S.
- Files: `src/desktop/input/macos.rs`, `src/desktop/input/linux.rs`.
- Approach:
  - macOS: cache the `CGEventSource` and the trusted check. Refresh the trusted check when the tap is disabled or times out.
  - Linux: cache the XTest connection.
  - Keep the `MENU_CODE` reset, and keep the lock.
- Accept: the existing replay tests pass.

## Phase 6: Settings I/O

### 6.1 `exists()` without reading the file

- Audit: SET-SAVE-EXISTS-FULL-READ (medium). Effort S.
- Files: `src/custom_emotes.rs:80-82`.
- Approach:
  - Implement `exists()` with `symlink_metadata`: `is_file()` and `0 < len <= MAX_INPUT_BYTES`, checked for the three allowed extensions.
  - Today it reads up to 8 MiB per id on every Save.
- Accept: a parity test against `media().is_some()` for valid, empty, oversized, symlink and missing files.

### 6.2 Catalog off the async runtime

- Audit: SET-CUSTOM-CATALOG-UNCACHED. Effort S.
- Files: `src/windows/desktop_settings.rs:237`.
- Approach: wrap `catalog()` in `spawn_blocking`. With 4.2 in place, it only reads files.
- Accept: the Settings boot tests pass. The trace shows Settings boot time with 16 custom Emotes.

### 6.3 Status-only heartbeat (low value, last)

- Audit: SET-HEARTBEAT-CLONE. Effort S.
- Files: `src/windows/runtime_mailbox.rs`, `src/windows/desktop_settings.rs:326-362`, `src/windows/mod.rs`.
- Approach: add a mailbox request that returns only the run status, so the 5 s heartbeat stops cloning `Settings` on the owner thread.
- Risk: it touches the owner mailbox. ADR 0001's completion and FIFO semantics stay as they are, and it needs a Windows pass.

## Phase 7: CI

Wall time and the developer loop matter here, not the product. These steps can land any time.

### 7.1 Concurrency and trigger filters

- Audit: CI-CONCURRENCY-PATHS. Effort S.
- Files: `.github/workflows/desktop.yml:3-6`.
- Approach:
  - Add `concurrency: { group: ${{ github.workflow }}-${{ github.ref }}, cancel-in-progress: ${{ github.ref != 'refs/heads/main' }} }`.
  - Limit `push` to `main` and tags, and keep `pull_request`.
  - Add `paths-ignore` for `**.md`, `docs/**` and `CONTEXT.md`.
- Accept: a docs-only push runs nothing (saves 361/246/423 s), and a double push to a PR cancels the first run.
- Risk: if this workflow becomes a required check, docs-only PRs would hang on it. Use a path-filter job then.

### 7.2 Bench profile without fat LTO

- Audit: CI-BENCH-DUP-LTO. Effort S.
- Files: `Cargo.toml`, `.github/workflows/desktop.yml`, `tools/perf_baseline.json`.
- Approach:
  - Add `[profile.bench-ci] inherits = "release"`, `lto = "off"`, `codegen-units = 16`.
  - Run the bench with `cargo nextest run --cargo-profile bench-ci`.
  - Record the profile in the perf JSON and re-seed the decode, render and RSS baselines. They are warn-only, so nothing breaks.
- Accept: the bench step saves about 25–40 s per OS.

### 7.3 Faster macOS builds off `main`

- Audit: CI-MACOS-SERIAL-UNIVERSAL. Effort S.
- Files: `.github/workflows/desktop.yml`, `tools/perf_gate.py`.
- Approach:
  - On the macOS job, set `CARGO_PROFILE_RELEASE_LTO=off` unless the ref is `main` or a tag.
  - Gate `binary_bytes` and the artifact sizes only on fat-LTO builds; otherwise just record them. Without that, every PR fails the size gate against a fat baseline.
  - Parallel per-arch jobs plus a lipo job is the second option, if this is not enough. Step 3.3 also shortens the link.
- Accept: macOS PR wall time drops, measured over 3 runs; `main` still produces a fat build and passes the size gate.

### 7.4 Dev debuginfo and cache policy

- Audit: CI-CACHE-DEBUGINFO. Effort S.
- Files: `Cargo.toml`, `.github/workflows/desktop.yml:47`.
- Approach:
  - Add `[profile.dev] debug = "line-tables-only"` and `[profile.dev.package."*"] debug = 0`.
  - Set `save-if: ${{ github.ref == 'refs/heads/main' }}` on `Swatinem/rust-cache`.
- Accept: `gh cache list` stays well under 10 GiB (about 9.1 GiB now), and the native test step time drops.

### 7.5 Trim Linux apt install

- Audit: CI-LINUX-APT. Effort S.
- Approach: add `--no-install-recommends`. Try dropping `autoconf automake libtool librsvg2-dev`, and keep any that the build turns out to need.
- Accept: the Linux dependency step takes well under 72 s, averaged over 3 runs.

### 7.6 UI checks on one OS

- Audit: CI-REDUNDANT-UI-STEPS. Effort S.
- Approach: run `cargo fmt --check`, `npm run check` and `npm test` only on Linux. Keep `npm ci` and `npm run build` on every OS, because `tauri build` needs `ui/dist`.
- Accept: the Windows UI step takes about 10–15 s less (estimate).

## Phase 8: Cleanup

### 8.1 Delete the dead native Settings window

- Audit: DEAD-NATIVE-SETTINGS-WINDOW. Effort M.
- Files: `src/windows/settings_window.rs` (2,651 lines), `src/windows/mod.rs`. The module is declared at `:6-7`; the `IsDialogMessageW` branch, the five `callback_busy()` checks and the hwnd gate are in the hook and timer paths (`:1199`).
- Approach: delete the file, the `SettingsWindow` field, those checks and the dialog branch. `desktop_settings.rs` remains the only Settings path.
- Accept:
  - `binary_bytes` is unchanged (the code is already linker-stripped).
  - Compile and clippy time drop.
  - The Windows smoke test passes: hooks, tray, Settings.
- Risk: it touches the hook procs. Land it separately from Phase 1, after Phase 1 merges, and give it a real Windows pass.
- ADR: deleting dead code from the Windows owner. ADR 0003 records that this is within "intact".

## Real-machine passes

Confirmation only; the CI gate is the source of truth.

- **Windows** (the user's two-monitor Windows 11 PC), after Phase 1 and again after Phase 2:
  - Run the old and new builds with `PINGLOOM_TRACE` set and compare frame and hook-callback p95 and max.
  - Check idle CPU and working set in Task Manager.
  - Check that the Volume Mixer session disappears after 30 s of silence.
  - Run the gesture smoke script only when nobody is using the desktop.
- **macOS:** not available. Release notes for 2.2, 3.3 and 5.1 to 5.4 say they were tested on CI only.

## Deferred and rejected

| Item | Status | Reason |
|---|---|---|
| Hook thread split / render worker (overlay-15, WIN-HOOK-THREAD-SPLIT) | Deferred | Needs trace data (1.8) and an ADR 0001 amendment |
| Save fsync off the owner thread (WIN-SAVE-FSYNC) | Deferred | Save is rare; ADR 0001 requires ordered, definitive completion |
| Hook self-heal (WIN-HOOK-SELFHEAL) | Rejected | Re-install thrash under UIPI, lock screen and RDP |
| External Emote pack (F6) | Rejected | Breaks the single-file portable exe; contradicts ADR 0002 |
| `panic = "abort"` | Rejected without owner approval | Behavior change: unwinding cleanup in `src/main.rs:60-91` |
| `Arc<Settings>` (SETTINGS-ARC) | Rejected | A clone costs 110–190 ns; disjoint borrows (2.1, 2.4) suffice |
| Cached modifier bits (ENGINE-MODIFIER-SCAN) | Dropped | About 0.3 µs per key event; drive-by only if `gesture.rs` is touched anyway |
| START-1, START-2, SET-1 | Refuted | Measured cost negligible or on a rare failure path |

## Linux-only backlog (not scheduled)

- Premultiply the GTK frame once per frame, skipping alpha 0 (overlay-09). The premultiply helper from 5.1 can be shared.
- Intern the EWMH atoms once and pipeline `get_geometry` and `translate_coordinates` (LINUX-FULLSCREEN-RTT).

## Found along the way (not optimization)

- The Unix renderer clears all Pings and the Emote when the desktop exceeds 16M px (`src/desktop/render.rs:211-214`), for example on two 4K monitors. That is a correctness gap on Linux and macOS. Fixing it needs its own decision, because per-effect windows would change ADR 0002's single-window overlay.
