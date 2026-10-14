# Desktop platforms — experimental prerelease

Settings uses the same bundled Svelte/Tauri interface on every OS. Gestures, input replay,
wheel hit-testing, overlays, WebP decoding and audio remain native Rust. Linux/macOS
are real backends, not executables that print “Windows only”. CI compilation/tests do
**not** establish physical desktop acceptance.

| Platform | Native input | Native overlay | Requirements |
| --- | --- | --- | --- |
| Windows x64 | Existing Win32 low-level hooks and SendInput | Existing layered Win32 renderer | Windows 11, Evergreen WebView2 |
| Linux x64 | evdev/uinput grab; XTest replay; X11 cursor/EWMH queries | Transparent GTK/Cairo native window | Compositing X11 session, input-device permissions, tray support |
| macOS | Quartz event tap; tagged CGEvent replay | Native AppKit NSImageView window | Accessibility + Input Monitoring; app bundle installed locally |

No overlay is a WebView. No Node process or remote server runs in production.

## Windows

Run `pingloom.exe`; open Settings from the tray. The executable is unsigned.
The legacy `%APPDATA%\lolPing\settings.json` path remains unchanged.
Mica is Windows-only. Snap Layout hover on the HTML maximize button remains unsupported.

## Linux

The `.deb` and `.tar.gz` are built on Ubuntu 24.04 x64. The tarball is **not** a
self-contained AppImage; install the GTK3, WebKitGTK 4.1, AppIndicator, X11/XTest and
ALSA runtime libraries for your distribution. The Debian package declares dependencies.
GNOME needs an AppIndicator/KStatusNotifierItem extension for a usable tray menu.

Launch the installed binary, or extract the tarball and run:

```sh
./pingloom --settings
```

`--settings` also provides access without a visible tray icon. Settings remains usable
when native input is unavailable. **Wayland/XWayland gestures are unsupported**; sign
into an X11 session. A non-composited X11 desktop cannot provide transparent overlays.

Native gestures require read access to `/dev/input/event*` and read/write access to
`/dev/uinput`. Pingloom neither runs as root nor changes permissions automatically.
These permissions expose **all keyboard input** to programs running as your user;
only grant them if you understand that trade-off. Do not make devices world-writable.

An administrator can grant temporary, user-scoped ACLs (reset on device recreation/reboot):

```sh
sudo modprobe uinput
sudo setfacl -m "u:$USER:r" /dev/input/event*
sudo setfacl -m "u:$USER:rw" /dev/uinput
```

Close and reopen Pingloom after granting access. Hotplugged input devices may need new
ACLs. Persistent udev rules are deliberately not installed by the application.
Do not launch the GUI with `sudo`. Mouse 4/5 triggers are unavailable on this backend;
Settings does not offer them. Fullscreen suppression uses EWMH window-manager metadata.
The menu-neutral replay key prefers F24, otherwise an unassigned X11 key; applications
with a custom F24 shortcut should use a non-menu modifier trigger and test that shortcut.

Settings: `$XDG_CONFIG_HOME/pingloom/settings.json`, normally `~/.config/pingloom/settings.json`.

## macOS

Extract the ZIP and move `Pingloom.app` into `/Applications`. The CI bundle is ad-hoc
signed, **not Developer ID signed or notarized**; Gatekeeper may require explicit
approval in System Settings → Privacy & Security. Do not disable Gatekeeper globally.
The archive filename records the CI machine architecture (`arm64` or `x86_64`); this is
not a universal binary.

Grant Pingloom **Accessibility** and **Input Monitoring** in System Settings → Privacy &
Security, then quit and reopen it. Without permissions, gestures remain paused and
Settings displays the reason. The app never grants itself permissions.

```sh
open /Applications/Pingloom.app --args --settings
```

Command replaces the Windows/Super trigger label. Option maps to Alt; Control maps to
Ctrl. Caps Lock is not offered as a trigger on macOS. Mica is not used.
Settings: `~/Library/Application Support/pingloom/settings.json`.

## Build from source

Install Rust stable, Node 22 (build-time only), and the native build prerequisites.
macOS needs Xcode Command Line Tools. Windows needs MSVC C++ Build Tools and WebView2.
For Ubuntu 24.04 Linux:

```sh
sudo apt-get install libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \
  librsvg2-dev patchelf libx11-dev libxtst-dev libevdev-dev libasound2-dev libudev-dev \
  autoconf automake libtool pkg-config
```

Then, on the target OS:

```sh
npm --prefix ui ci
npm --prefix ui run check
npm --prefix ui test
npm --prefix ui run build
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release --bin pingloom
```

Core-only checks without desktop libraries: `cargo test --locked --no-default-features --lib`.
The no-desktop binary is not a desktop application and is not released.

## Acceptance still required

On actual hardware, verify hold/drag/release, all eight slices, center/Escape/right-click
cancellation, click pass-through, modifier/hotkey balance, foreign shortcuts, injected
input pass-through, full-screen suppression and release/quit during a pending gesture.
Test Save/Cancel/close/reopen, permissions revoked mid-session, mouse acceleration,
non-US layouts, mixed DPI, multiple monitors, reduced motion, emote timing and audio.
Restart after display-scale/topology changes; Linux monitor scales are sampled at startup.
Linux uses one bounded native surface (16 million pixels); very widely separated effects
can be discarded rather than allocating an unbounded desktop-sized buffer.

Automated tests cover core gestures, settings/lifecycle/replay, native key translation,
bounded WebP decoding, native wheel rasterization and frontend logic. They do not drive
physical hooks or certify transparency, input permissions, latency or desktop behavior.
