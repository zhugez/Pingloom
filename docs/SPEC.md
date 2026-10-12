# Approved MVP

User approved all items below. This is a rewrite, not a complete feature-parity port.

## Target

- Windows 11 x64 portable executable. Entire application is Rust, including input hooks, UI, rendering and audio integration. No Electron, WebView, JavaScript runtime or C++ helper.
- Native settings window; functional keyboard accessibility, labels and focus navigation. Pixel parity with Fluent UI is not required.
- Source reference: https://github.com/zhugez/Pingloom at f24a51e5a11c2b31dab72d89a571cc4025ad3c98; local read-only reference /tmp/pingloom-upstream.

## Included

- Hold Alt and drag for the eight-slice ping wheel; hold Ctrl and drag for the eight-slice emote wheel.
- Release to choose; release in centre, Escape or right-click cancels. Do not swallow ordinary trigger taps, typing or unrelated shortcuts. Trace upstream input arbitration, including synthetic replay and stuck-key prevention.
- Configurable trigger keys, toggle shortcut, size, duration, volume, mute and wheel slot assignment. Persist settings safely; validate loaded settings, preserve corrupt file rather than silently destroying it.
- Upstream ping textures/animations and sounds, upstream built-in emote images/animations and sounds. Correct asset attribution.
- Transparent, always-on-top, click-through overlays, supporting multiple monitors, mixed DPI and negative desktop coordinates. Draw at the gesture location and ensure normal apps regain input after completion/cancel/quit.
- Pause initiation on the display occupied by a fullscreen application. Preserve normal input in that state; don't globally disable other monitors.
- Tray actions: settings, pause/resume, quit. Configurable toggle shortcut (default Ctrl+Alt+P). No elevation requirement.
- Portable .exe plus a reproducible Windows build and CI artifact. Prefer embedded assets so executable is truly portable, or explicitly document any necessary companion asset layout.
- Gemini-generated original README banner/logo decoration only, not replacement game assets.

## Excluded

- Rooms, LAN/Nostr, crypto sharing, updater, installer, startup registration, custom image import, macOS, Linux desktop support.
- Admin-window and exclusive-fullscreen overlay support; respect Windows restrictions and document them.

## Acceptance evidence

- Runnable Rust tests for gesture transitions, cancellation, wheel selection, settings validation and coordinate handling.
- cargo fmt --check; cargo test; cargo clippy with warnings denied where feasible.
- Compile actual Windows code, not just a Linux stub. Windows CI checks/builds/tests the Windows target and produces a release executable artifact.
- Windows runtime behavior (hooks, fullscreen, mixed-DPI, animation, sound and tray) requires a real Windows smoke check. Never describe cross-compilation or CI build success as a completed desktop runtime test.
- README: architecture, build/use, limitations, attribution, deferred scope and exact validation status. MIT upstream attribution preserved; Riot assets are not covered by MIT.

## Implementation policy

Use openai-codex/gpt-6-luna at max thinking for implementation. No silent model fallback. Prefer Win32/platform APIs and a few established Rust crates to speculative abstractions. Do not silently cut a requested feature; escalate concrete blockers.
