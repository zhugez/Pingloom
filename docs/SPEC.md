# Pingloom — Approved MVP

## Target

Windows 11 x64 portable desktop app. Native Rust owns low-latency keyboard/mouse hooks, replay, gesture state, click-through ping/emote overlays, animation/audio and tray. Settings is the only WebView: a local bundled Tauri 2 window using Svelte 5 + Vite + Tailwind/daisyUI + `@lucide/svelte`. This is not a website, SvelteKit app, server, demo, or WebView-rendered overlay. Evergreen WebView2 is a Settings runtime prerequisite. No production Node process.

The approved UI migration supersedes earlier Rust-only/native-Settings restrictions. Existing native wheel/ping/emote renderers and improvements remain authoritative and must be preserved.

## Included

- Hold Alt/Ctrl and drag for eight-slice ping/emote wheels; release selects, centre/Escape/right-click cancels. Preserve ordinary input, replay and stuck-key prevention.
- Configurable triggers, toggle shortcut, size, duration, volume, mute, assets and wheel assignments; validated settings and safe atomic persistence.
- Embedded upstream artwork, animations and sounds with attribution; transparent, always-on-top, click-through overlays across monitors and negative coordinates.
- Fullscreen-display gesture pause, tray settings/pause/resume/quit, keyboard-accessible Settings with Save/Cancel and field-directed errors.
- One Settings WebView only. It communicates through narrowly scoped validated commands to the native owner; no arbitrary filesystem/path access, remote navigation, hosted endpoint, shell/plugin, or frontend authority over persistence/validation.

## Excluded

Rooms, sharing, updater, installer, startup registration, custom image import, macOS/Linux desktop support, admin-window and exclusive-fullscreen overlay support.

## Acceptance evidence

Rust tests and fmt/clippy; Windows-target compilation and linked executable; clean frontend install/check/tests/build; restrictive capability/CSP/navigation review; real Windows 11 smoke test for hooks, WebView2 lifecycle, save/reload, replay, fullscreen, DPI, animation/audio and tray. Cross-compilation is not runtime evidence. Existing embedded assets total about 58 MiB; no 2–4 MiB executable claim is valid. README must state exact evidence and outstanding checks. Current integration has not had a real Windows smoke test; Windows-only unit tests merely compile on this host. Riot assets are not covered by MIT.

## Build/runtime constraints

Vite emits local static `ui/dist` files embedded by Tauri; Node/npm are build-time only. Evergreen WebView2 must be installed on Windows 11. Keep Win32 hooks and their message pump on a dedicated owner thread; never block Tauri's event loop or run two blocking pumps on one thread. WebView2/process-failure recovery and orderly shutdown must be explicitly verified before claiming complete integration.
