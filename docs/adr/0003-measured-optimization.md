# ADR 0003: Measured optimization of render, idle cost and size

- Status: Accepted implementation direction; measurement steps in progress, CI acceptance pending
- Date: 2026-10-16

## Context
Build `6739725` (0.1.0) works on all three platforms, but it is costly in three places.

The Windows overlay redraws and clears a desktop-sized DIB on a 33 ms timer that never stops. A Wheel frame costs 14–70 ms on a Linux proxy, depending on scale. Art is resized on every frame.

The idle process still wakes about 30 times a second, and its audio stream plays silence.

Downloads are large. Emote WebPs are 54.5 MB, and the macOS universal binary embeds every asset once per architecture, so the zip is 122 MB.

The CI perf gate exists, but its baseline is empty, so nothing is enforced.

ADR 0001 keeps hooks and all gesture state on one pumped Win32 owner thread. ADR 0002 keeps the Windows owner and renderer "intact", shares embedded assets across backends, and renders Unix overlays into a single native window: AppKit `NSImageView` on macOS. The steps are in `docs/plans/optimization.md`.

## Decision
1. **Gate before optimizing.** The CI perf gate gets a per-platform baseline and tracks binary, artifact and asset bytes, plus deterministic render counters. Those fail CI above 5%. Timings and memory are recorded and warn above 10%, because runner noise makes them unfit to fail a build. A render characterization test (hashes of fixed frames) lands before any renderer change.
2. **"Intact" means same output and same threading.** In-place changes to the Windows renderer are allowed while the characterization hashes hold and drawing stays on the owner thread. Allowed changes include caching, lookup tables, dirty-rect presentation, an overlay sized to the effect box, and deleting the dormant native Settings window. A tolerance of at most 1 per channel is allowed only when a test justifies it.
3. **macOS overlay paints through CGImage.** Frames reach the existing `NSImageView` as a `CGImage`-backed `NSImage` instead of a per-frame PNG round trip. The pixel output must match the old path within 1 per channel on CI. No new crate is added.
4. **macOS release assets live in the bundle.** A default-on `embed-assets` Cargo feature keeps `include_dir!` for Windows, Linux and source builds; the portable Windows exe stays a single file. The macOS release build turns the feature off and reads `Contents/Resources/assets`, so assets ship once instead of once per architecture. This amends ADR 0002's "embedded assets" for macOS release bundles only.
5. **Hooks stay on the owner thread for now.** A render worker is reconsidered only if a real-PC trace shows a hook callback p95 above 5 ms or a max above 50 ms after the render and idle work. Any such split needs an amendment to ADR 0001.
6. **Opt-in local trace.** `PINGLOOM_TRACE=<file>` writes latency histograms and failure events to a local file. When it is unset, a probe costs one relaxed atomic load. Probes in hook procs never allocate, lock or do I/O. Nothing is uploaded, and there is no UI.

## Measurement implementation status

Steps 0.1 and 0.3 and most of 0.2 are implemented locally. Further broad changes now include bounded Windows/Unix art caches, lazy audio, idle timers/wakes, shared/pre-sized frames, thumbnail sidecars, metadata-only existence checks, off-thread catalogs, status-only heartbeats and macOS bundle resources/direct CGImage painting. They are not a completed optimization release; `docs/plans/optimization.md` contains the explicit remaining-step ledger. Actual offscreen benchmarks and per-frame counters exist for both native renderers. Eleven-state hashes passed on Windows/Linux/Mac, including fixed Wheel label captions. The dynamic GDI action caption remains outside the golden scenes because OS fonts differ; do not optimize that path as though full caption characterization had landed. Idle sampler runtime validation and CI-seeded UI/render timing metrics remain pending. Step 0.4 histogram/probe wiring exists, but one-off events and the physical latency checkpoint remain incomplete. See `docs/performance.md` for evidence and limits.

## Consequences
Every optimization PR must keep the gate and the characterization test green. A deliberate size or output change must update the baseline or hashes in the same PR.

CI is the source of truth for release artifacts. Windows changes that alter window geometry, z-order or display handling need a pass on a real multi-monitor PC. A physical Mac Mini is now available via operator-authorized SSH; record whether each change was verified there or only on CI. Earlier hardware evidence does not certify a later change.

The current gate baseline comes from green CI run 38818302853 (`168c1d9`), after V7 and its theme/Option fixes, rather than the historical audit build. Linux public distribution is paused (Coming soon); gate Windows/macOS release artifacts, retaining shared-code Linux tests as development evidence. V7 already caches Wheel geometry and labels; later steps must preserve that appearance/theme behavior and avoid duplicating its cache.

Asset re-encoding requires a local League install, and Emotes may upscale slightly at the largest size settings. The version stays 0.1.0 until the owner decides to ship.

Out of scope:

- `panic = "abort"`
- an external asset pack
- hook self-healing
- `Arc<Settings>`
- per-effect Unix windows
