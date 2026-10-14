# Implementation status

Native and frontend integration shares one checkout. Native platform CI builds the three release targets; physical desktop verification remains separate.

## Architecture and current status

Tauri owns one local Settings WebView. A separate pumped Win32 owner thread retains `APPLICATION`, hooks, replay, gesture engine, native overlays, animation/audio and tray. Bounded owned requests are handled serially on that owner. Opening Settings acknowledges an owner-queue barrier without pausing gestures. Save validates, persists and applies on the owner, safely cancelling any active gesture. Close/watchdog destroy the window and await a queue barrier before admitting a new generation; they do not change input or manual pause state. The dormant native Settings source remains in-tree but is no longer the active tray route.

The Tauri bridge is registered in `src/main.rs` and implemented in `src/windows/desktop_settings.rs`; mailbox and owner lifecycle live under `src/windows/`. Five commands expose boot/catalog, Save, close, heartbeat, and custom image import. Caller label plus local origin/path are checked. Candidate payload size/tree/keys/wheel lengths/enums/assets are restricted; canonical persistence/application remains native. UI resources are bundled from `ui/dist`. Capability is Settings-window scoped and only grants event listening; navigation and CSP constrain local content and embedded image previews.

## Custom emotes and single instance

`src/custom_emotes.rs` stores local images beside the canonical settings file.
The trusted `import_emote` command accepts bounded base64 image bytes, never a path.
PNG/JPG are normalized to static PNGs; GIF/WebP retain their original validated bytes
and compression. Format is detected from content, not the filename. Rust limits input
to 8 MiB / 2048×2048. A shared decoder bounds animation to 300 frames, 30 seconds and
128 MiB decoded pixels, inspecting dimensions before native WebP decoder creation.
All frames are validated before persistence; an intact thumbnail is insufficient.
Premultiplied-alpha resampling caps native frames at 512px. There are at most 16 library
items / 32 MiB total, with path-safe `custom_<16 lowercase hex>` IDs. Imports are serialized, deduplicated and
atomically persisted without replacing existing artwork. They add catalog entries;
wheel/tap assignments remain drafts until the ordinary validated owner Save. Save
rejects custom IDs absent from the library. Core schema remains version 1.
Windows and Unix native renderers share the decoder and timing helper. Custom GIF/WebP
loop within the ordinary three-second emote lifetime; decode readiness starts the
playback clock. Windows hides the differently cropped PNG thumbnail while animated
decode is pending, avoiding a visible PNG-to-WebP scale jump. Completion resets both
playback and effect lifetime to readiness. Decode failure/worker-spawn failure releases
pending into the still fallback; static assets and reduced motion display their still
without waiting. Unix already decodes before exposing the effect. GIF zero delay defaults to 100ms and sub-10ms delays clamp to 10ms.
Reduced motion and picker thumbnails use a static first-frame PNG. Existing custom PNG
IDs/files remain compatible, and bundled WebP does not adopt custom looping. No filesystem
plugin, remote asset URL or overlay WebView is introduced. Custom audio and library
management controls remain unimplemented.

`src/single_instance.rs` uses `std::fs::File::try_lock` in the user settings directory
before starting hooks/tray. Tauri retains the File for the runtime lifetime. The lock
file is never unlinked; OS handle teardown releases it on exit/crash. Failure to lock
fails closed, before input acquisition. Process tests cover a rejected second process,
release on descriptor close and exit without Rust destructors. There is no second-launch
focus/argument-forwarding behavior. Different user/settings directories are independent.

## Build/runtime constraints

Windows 11 x64 with Evergreen WebView2 Runtime is required. Missing/unavailable WebView2 prevents Settings startup; build output logs the prerequisite on window creation failure. Node/npm are needed only to build static frontend assets. `build.rs` embeds the frontend distribution; the app has no production Node service. The embedded game assets total approximately 55 MiB, so expect a large executable. No installer/updater is supplied.

Build in this order on Windows with Rust stable and Visual Studio 2022 Build Tools/C++ workload:

```powershell
cd ui
npm ci
npm run check
npm test
npm run build
cd ..
cargo fmt --all -- --check
cargo build --locked --release --target x86_64-pc-windows-msvc --bin pingloom
```

GNU cross-build/check examples: `cargo check --locked --tests --target x86_64-pc-windows-gnu --bin pingloom`; actual link requires the matching MinGW linker. Native platform CI is defined in `.github/workflows/desktop.yml`.

## Evidence and gaps

Host frontend check/test/build, Rust formatting/tests/host Clippy, and Windows GNU check/test-check/Clippy/link build are the expected validation suite; report actual command outcomes from the integration run, not this target list. This Linux host cannot exercise WebView2/Win32 behavior. Shared session/mailbox tests run on the host; Win32-only behavior needs Windows. No Windows 11 runtime or browser visual certification is claimed.

Windows smoke checklist: Evergreen present/absent startup; tray open/focus with concurrent ping/emote gestures and barrier acknowledgement; save mid-gesture cancellation/input balance; field edits/save/restart; Cancel/Escape/OS close/watchdog and pause preservation; renderer/WebView2 failure; orderly tray shutdown; mixed DPI/multimonitor, fullscreen, hooks, overlays, animation/audio and tray. Remaining risk includes Tauri/WebView2 lifecycle and actual accessibility/visual behavior until this checklist is run on Windows.
