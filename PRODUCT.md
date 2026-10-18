# Pingloom — Product

## Platform
Windows 11 x64 desktop and experimental macOS native builds. Linux is Coming soon: public Linux packages and release CI are paused, while the Linux/X11 source backend is retained. Native Rust owns latency-sensitive input, overlays, animation/audio and tray. Settings is one local Tauri 2 WebView (Svelte 5/Vite/Tailwind/daisyUI and `@lucide/svelte`), not a hosted app. Evergreen WebView2 is required; Node is build-time only.

## Version policy
Keep the app and consolidated prerelease at **0.1.0** until the user explicitly approves a version increase. Distribution downloads use approved release tag `v0.1.0-opt.2` while the application version remains 0.1.0. Features and fixes refresh that prerelease; do not automatically bump Cargo, Tauri or frontend versions.

## Users and purpose
People placing desktop pings and emotes independently of the game process. Settings should be understandable and accessible without adding latency or steps to gestures.

## Capabilities and constraints
- Preserve native trigger-held gestures, eight slices, cancellation, input replay and per-display fullscreen pause.
- Preserve embedded assets, animation/audio, settings schema/validation and safe persistence.
- One Settings WebView, with Save/Close, keyboard navigation, field-directed errors, previews and no authority over validation or paths. Its color theme applies immediately to Settings and both native Wheels through a bounded, session-guarded appearance command. Native appearance persists separately in appearance.json and never changes the gesture Settings draft or schema.
- Native overlay/tray behavior and existing improvements remain in Rust. Never replace the overlay with WebViews.
- No sharing, updater or startup registration. Linux/macOS builds require native input permissions; Wayland gestures are unsupported. See `docs/PLATFORMS.md`.

## Current evidence
The Tauri Settings window, registered command bridge, native owner mailbox and bundled UI are implemented. Compilation and automated tests do not prove physical runtime or visual behavior on any OS. Windows-only bridge tests compile but were not run on Windows; Evergreen lifecycle, renderer failure, accessibility and visual smoke checks remain outstanding. See `docs/IMPLEMENTATION.md` and README.
