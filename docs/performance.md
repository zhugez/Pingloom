# Performance evidence and regression gates

Version remains 0.1.0. Linux public distribution is paused; local Linux raster tests are development evidence only.

## Gate baseline

`tools/perf_baseline.json` starts with verified Windows/macOS packages and decode metrics from green CI run **38818302853**, build **168c1d9fae352ddd912394dda64cc476fc82286a**. Artifact hashes were checked against each `BUILD_INFO` file. Asset bytes come from the unchanged tracked assets of that build.

Sizes (`*_bytes`) and deterministic render counters (`*_count`) fail above 5% growth. Other numeric metrics, including decode/render milliseconds and RAM, warn above 10%. A baseline key missing from a run fails with a readable message. Zero baselines skip comparison with a notice. Benchmark JSON cannot override directly measured sizes. Extra JSON metric files must have distinct keys.

UI-dist bytes, render timings and Windows idle metrics were absent from the historical CI artifacts. They are collected now but remain unbaselined until the first verified CI run of this change. Do not silently infer them from local hardware or overwrite baselines during an ordinary build.

Deterministic render byte/resize counters are seeded from offscreen native tests: Windows GNU target executed on the real Windows host and Unix rasterizer on Linux, with matching counters verified on the physical arm64 Mac Mini. Counters are per-frame averages over 60 fixed frames. `written_bytes` counts pixel work/processed blit area, including transparent artwork; it is not presented-window bandwidth or peak memory. Windows now clears effect-bounded dirty rows (the characterization reference still uses a 1920x1080 canvas); Unix allocates effect-bounded frames. Counter differences between these backends are intentional.

## Characterization

`cargo test --locked --bin pingloom render_characterization` hashes actual offscreen renderer bytes with stable FNV-1a. Separate checked-in Windows/Unix golden files cover wheel 1x/1.5x/2x at hot slots 0/3, mid-selection, Ping ages 100/700/3200ms and an animated Emote frame at 250ms. Unix hashes also include frame position/dimensions. The same Unix goldens passed on Linux x64 and Mac arm64. Debug/release goldens passed on Linux and Windows.

The fixed Wheel labels (including hub caption) are included. Windows' dynamic GDI selected-action caption is not in the golden scenes: glyph rasterization depends on installed OS fonts. Mask-flush/compositing and same-OS cold/warm caption equality tests cover cache behavior; a native test also verifies font/DC/DIB reuse across Wheel opens and the eight-scale cache bound. Do not claim the entire Windows text path is portably hash-locked.

To deliberately record candidate hashes, set `PINGLOOM_RECORD_RENDER_HASHES=<file>` and run characterization. Recording **fails the test intentionally**; review the candidate visual changes and copy accepted values, then rerun without the variable. CI must not set the recording variable. Goldens never auto-update.

## Benchmark

```sh
PINGLOOM_RENDER_OUT=render.json cargo test --locked --release --bin pingloom render_benchmark -- --ignored --nocapture
python3 -m unittest discover -s tools -p 'test_perf_gate.py'
```

Each scenario warms once, then measures 60 frames: Wheel 1x/1.5x/2x with eight icons and cycling selected slot, fading Ping, animated Emote. Asset/emote decode and Wheel construction are outside frame timing. Selection is in settled states, not an interpolation stress test. Counters are thread-local and compiled only under `cfg(test)`; production has no instrumentation overhead. Timing includes counter overhead and is advisory, not a native input-latency measurement.

Measured before renderer optimization (local hardware; not CI timing baselines):

| Scenario | Windows p50 / p95 ms | Mac Mini arm64 p50 / p95 ms | Resizes/frame |
|---|---:|---:|---:|
| Wheel 1x | 8.99 / 10.27 | 2.14 / 3.13 | 8 |
| Wheel 1.5x | 17.89 / 23.76 | 3.77 / 3.87 | 8 |
| Wheel 2x | 29.64 / 39.97 | 6.17 / 6.37 | 8 |
| Ping | 3.72 / 5.34 | 0.73 / 0.76 | 2 |
| Emote | 3.41 / 3.98 | 0.76 / 0.83 | 1 |

Windows measurements used a release GNU cross-built test executable, not the release CI MSVC binary. Neither backend measurement includes `UpdateLayeredWindow`, GUI-thread dispatch, PNG/AppKit presentation, screen capture or input hooks. No synthetic input was sent to either desktop. The installed Mac application was not replaced for these tests.

## Windows idle sampling

`tools/windows_idle_metrics.ps1 -ProcessId <pid> -Out idle.json` reads CPU, working set and private memory over 10 seconds. It verifies process start identity and normalizes CPU milliseconds to a 10-second interval. CI runs it after startup, before smoke messages/input; the gate merges `idle.json` on Windows only. The JSON is UTF-8 without a BOM. No input, permissions, priority or target state is changed.

PowerShell syntax validation passed on the physical Windows host, but its execution policy blocked running the `.ps1`; policy was left unchanged. Runtime sampler acceptance and the first idle/UI/render timing baselines remain pending CI. CI may have no audio device, so its idle cost can understate a real desktop's audio wakeups.

## Step 1.1: exact-size Windows Wheel artwork cache

Windows Wheel icons now cache premultiplied-resampled images by asset path and exact integer width. The cache is owned by `WheelFx`, bounded to 160 entries and 8 MiB, and freed with the Wheel on close/display rebuild. It does not cache animated Emote frames or alter their playback. Unusual size churn clears the bounded cache rather than growing memory without limit.

The real Windows Canvas regression and characterization tests passed after this change: warm frames perform zero resizes at 1x/1.5x/2x, all eleven golden hashes are unchanged, asset identities remain separate and cache bounds are checked. The release benchmark also records 0 resizes/frame at each Wheel scale; cleared/written-byte counters remain identical. The benchmark explicitly warms every selected/base icon size before measuring steady frames.

One local cached timing run gave Wheel p95 9.80/13.84/50.40 ms at 1x/1.5x/2x. Compared with the earlier run, 1.5x improved but 2x regressed and unrelated Ping timing also varied substantially; do not claim a stable speedup from these runs on a live desktop. The deterministic resize reduction is proven; CI and a controlled hardware timing pass remain outstanding.

Windows resize baselines are updated to zero. The generic gate deliberately skips zero-baseline percentage comparisons; the explicit warm-cache regression test enforces zero resizes so that this improvement cannot silently regress. Existing nonzero byte counters still fail on >5% growth.

## Broad optimization implementation (not yet released)

The current checkout additionally implements:

- WebP `into_rgba_image()` (no frame-buffer memcpy), Arc-owned frames, and up to four scoped resizing workers. Tests compare every pre-sized frame to the old resampler byte-for-byte, preserve timestamps, disallow upscale and confirm frame storage sharing. Pending Emotes no longer expire before a slow decode becomes ready.
- Exact-size Unix Wheel icon caching and borrowed opacity-1 blits. Cache entries remain bounded and die with the Wheel.
- Lazy audio output with bounded queue, Wheel warm-up, five-second open-failure backoff and thirty-second no-voice idle close. Policy tests do not certify real device hotplug recovery.
- Atomic custom thumbnail sidecars; metadata-only existence checks; catalog generation off the async executor; status-only heartbeat mailbox requests.
- Windows frame timer active only while effects exist, held-key-only native resync and circle row spans. These changes require GUI smoke acceptance before release.
- Unix owner wake channel/idle waiting, idle pointer-motion fast-path and eligible-gesture-only fullscreen probing. macOS replay event-source caching preserves the live trust check.
- Direct sRGB premultiplied RGBA CGImage presentation on macOS, with provider-owned bytes. Physical Mac tests verified that old PNG and new CGImage output differ by at most one per channel, and source-buffer lifetime is safe. Installed app was not replaced for these checks.
- Default-on embedded assets for portable Windows/source builds; macOS release bundles ship Resources/assets once. On the physical arm64 Mac, the release executable was **11,537,536 bytes**, compared with the installed old arm64 executable's **68,994,240 bytes**. This is an architecture-local executable comparison, not a universal ZIP saving claim. An isolated bundle-layout resource test passed; a binary outside its bundle refused startup without resources. Full universal packaging/signature/runtime acceptance remains CI work.
- Nineteen unreferenced Ping textures/sounds removed, saving **566,125 bytes**. A canonical-model inventory test prevents future orphans. No bundled Emote was re-encoded or deleted.
- Local trace histograms and selected probes; bench-ci without fat LTO, main/tag concurrency filters, one-platform UI checks and dev debug/cache policy. Timing/RSS baselines must be re-seeded from the new CI profile; old fat-LTO timing baselines were removed rather than compared against a different profile. Profile metadata is written to perf JSON.

### Step 1.3: Windows static Wheel layer

The Windows renderer reuses V7 geometry to bake an idle premultiplied BGRA layer including the fixed label atlas. Opaque frames copy clipped rows and replace only highlighted-slot texels, compositing their label pixels in the original order. Highlights replace idle pixels instead of blending over them, preserving alpha exactly. Entrance fade keeps the original per-primitive path to retain rounding; theme changes rebuild the layer. The layer is owned by the open Wheel, about 3.4 MB at 2x, and is released on close. No new geometry cache or dependency was introduced.

All eleven Windows characterization hashes remain unchanged. A native regression compares against the original painter across six themes, scales 0.75/1/1.25/1.333/1.5/2, simultaneous partially selected slots, entrance opacity and negative/offscreen clipping. The latest physical Windows run passed **72 tests, 3 ignored**; logs are `%TEMP%/pingloom-session-resume/layer-final-tests.log`.

One local GNU-target bench-ci run measured Wheel p50/p95 **1.15/1.63**, **2.66/3.75**, **4.88/6.39 ms** at 1x/1.5x/2x. Its JSON is `%TEMP%/pingloom-session-resume/render-windows-layer-bench-ci.json`. Earlier pre-layer bench-ci p50 values were 15.27/27.11/46.65ms, but unrelated Ping/Emote times also varied, so this is not a controlled speedup ratio or CI timing baseline. This benchmark excludes native presentation and hooks; physical frame/input latency acceptance remains pending.

Windows deterministic written-byte baselines are deliberately tightened from 1,493,186 / 3,350,330 / 5,947,758 to **981,226 / 2,204,126 / 3,915,126 bytes/frame**. Warm icon resizes stay zero and cleared-byte counters are unchanged. macOS counters and all package/timing baselines are unchanged. Linux binary/library tests, both-target Clippy and four perf-gate tests also passed after this change.

### Continued Windows rendering and Settings watchdog checks

Dirty-rect/effect-sized Windows presentation, settled-Wheel signature skipping, per-scale GDI caption resources and local failure events are implemented. Native tests use hidden windows without synthetic input: negative-coordinate placement, dirty-union clearing, idle 1x1 reclamation, partial `UpdateLayeredWindowIndirect` updates and raster characterization pass. Portrait/custom art retains full-desktop clipping rather than truncating legacy width-scaled artwork. Window z-order, monitor changes and DPI still need interactive multi-monitor acceptance.

The Settings watchdog now starts only after an opening generation is reserved and retires when the lifecycle is fully closed. It remains alive through opening, closing and pending/in-flight owner barriers. Worker admission/retirement use the existing lifecycle mutex, avoiding a lost re-arm without another atomic or condition variable; thread spawn failure rolls back before any owner command is accepted. A deterministic lifecycle test covers retry, reopen on either side of retirement and duplicate-worker rejection.

On session continuation, the Windows GNU-target test executable ran on the physical Windows host: **71 passed, 3 ignored** (serial run). This includes caption resource reuse, characterization and the new watchdog regression. Windows-target and Linux Clippy passed; Linux binary tests reported **70 passed, 3 ignored**, shared library tests **29 passed**, and perf-gate stdlib tests **4 passed**. Logs on Windows: `%TEMP%/pingloom-session-resume/watchdog-tests.log`. No installed app was replaced. The lifecycle test is not a live WebView/heartbeat-expiry or wakeup measurement.

This is not the whole plan. The physical trace/hotplug checkpoints, superseded-decode cancellation and dead native Settings removal remain outstanding. Asset rebaking/absolute post-rebake budgets and the size-opt/LTO experiments also remain blocked or unrun. See the step ledger in `docs/plans/optimization.md`; these local tests do not certify CI or release acceptance.

## Next gate

Complete CI acceptance and physical multi-monitor/latency checks before release. Exact-size Windows artwork caching is implemented locally against these hashes/counters; dynamic captions use same-OS equality tests rather than portable font hashes. Further steps must preserve animation, theme behavior, input ownership and the native overlay; no follow-up optimization is accepted as shipped before CI is green. Do not publish optimization claims from unverified CI or these single local timing runs.
