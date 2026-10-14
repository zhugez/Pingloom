# ADR 0002: Experimental native Linux and macOS backends

- Status: Accepted implementation direction; physical runtime acceptance outstanding

## Context
The user authorized a functional port to Windows, Linux and macOS, not non-Windows
placeholder binaries. The existing Windows owner and renderer must remain intact.
Settings-only WebViews, canonical Rust validation, atomic persistence and native input
replay remain constraints from ADR 0001.

## Decision
Keep the Win32 backend. Compile separate native adapters for Linux/X11 and macOS,
sharing the gesture core, embedded assets, bounded WebP decoder, audio, Settings bridge
and generation-aware lifecycle/mailbox. A serialized native engine handles input and
control requests; file persistence and GUI dispatch do not hold its input mutex.

Linux uses evdev/uinput suppression and XTest replay, with actual X11 cursor coordinates
and EWMH fullscreen metadata. macOS uses a Quartz event tap and tagged native replay.
Render Unix overlays into a single bounded native Tauri Window: GTK/Cairo on Linux,
AppKit NSImageView on macOS. This window has no WebView or IPC capability. Settings
remains the only WebView; its trusted origin is `tauri://localhost` on Unix and
`http://tauri.localhost` on Windows.

## Consequences
Linux needs explicit device access and a compositing X11 session; Wayland gestures
are unsupported. macOS needs user-granted Accessibility/Input Monitoring. No automatic
privilege escalation, ACL/udev modifications or privacy-permission grants are performed.
Queue/renderer failures pause interception. OS reduced-motion preferences suppress
native decorative motion; gesture timing remains unchanged.

Windows storage and app identity remain compatible. Unix uses new per-user config
paths. CI builds/tests all three native targets and packages genuine binaries; unsigned
Windows and ad-hoc macOS bundles are prereleases, not certified production builds.
See `docs/PLATFORMS.md` for prerequisites, architecture limits and physical acceptance.
