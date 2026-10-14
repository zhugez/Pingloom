# Pingloom — Product

## Platform
Windows 11 x64 desktop, with experimental Linux/X11 and macOS native backends. Native Rust owns latency-sensitive input, overlays, animation/audio and tray. Settings is one local Tauri 2 WebView (Svelte 5/Vite/Tailwind/daisyUI and `@lucide/svelte`), not a hosted app. Evergreen WebView2 is required; Node is build-time only.

## Users and purpose
People placing desktop pings and emotes independently of the game process. Settings should be understandable and accessible without adding latency or steps to gestures.

## Capabilities and constraints
- Preserve native trigger-held gestures, eight slices, cancellation, input replay and per-display fullscreen pause.
- Preserve embedded assets, animation/audio, settings schema/validation and safe persistence.
- One Settings WebView, with Save/Cancel, keyboard navigation, field-directed errors, previews and no authority over validation or paths.
- Native overlay/tray behavior and existing improvements remain in Rust. Never replace the overlay with WebViews.
- No sharing, updater or startup registration. Linux/macOS builds require native input permissions; Wayland gestures are unsupported. See `docs/PLATFORMS.md`.

## Current evidence
The Tauri Settings window, registered command bridge, native owner mailbox and bundled UI are implemented. Compilation and automated tests do not prove physical runtime or visual behavior on any OS. Windows-only bridge tests compile but were not run on Windows; Evergreen lifecycle, renderer failure, accessibility and visual smoke checks remain outstanding. See `docs/IMPLEMENTATION.md` and README.
