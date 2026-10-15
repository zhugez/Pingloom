# ADR 0002: Experimental native Linux and macOS backends

- Status: Accepted implementation direction; physical runtime acceptance outstanding

## Context
The user authorized a functional port to Windows, Linux and macOS, not non-Windows
placeholder binaries. The existing Windows owner and renderer must remain intact,
except for the user-approved visual-only amendment below.
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

## Amendment: native V7 Wheel appearance

The user explicitly approved bringing the Settings V7 Wheel design to the actual
native desktop Wheel, not just its Settings preview. In-place visual changes to
both native renderers are therefore allowed: spaced dark wedges, a separate hub,
Blush selection arcs, compass ticks and locally rasterized labels. Shared Wheel
geometry/color data is prepared once per open; selection colors remain animated
on the existing rendering thread. This changes appearance intentionally, not the
86-DIP cancellation radius, slice mapping, input handling, replay, window ownership
or effect lifetime. Native Ping/Emote artwork and effects are unchanged.

Windows keeps its native dynamic selected-action caption below the Wheel.
Acceptance requires native raster checks and the existing Windows smoke test;
compile checks alone do not certify physical desktop behavior.

## Amendment: native Wheel follows Settings appearance

The user subsequently requested that the actual Ping/Emote Wheels change color
with the selected Settings theme, superseding the initial fixed-Night behavior.
A trusted-document/session-validated `set_appearance` command accepts only the six
known palette IDs. Appearance applies immediately and persists atomically in a
separate `appearance.json`, without saving or extending the gesture settings draft.
The lifecycle lock serializes appearance writes with close/reopen; no input lock
is held for persistence. Native renderers read an atomic palette ID and refresh
cached surface/label colors when it changes, including on an already-open Wheel.
This changes appearance only, not input authority, permissions, timing or replay.
There are still no remote assets or overlay WebViews.
See `docs/PLATFORMS.md` for prerequisites, architecture limits and physical acceptance.
