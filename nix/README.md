# Native NixOS / niri edition (experimental)

This is a source-built native Wayland edition, not an XWayland app or an arming
shortcut. It requires the pinned **Pingloom-integrated niri 26.04** package.
Stock niri and other Wayland compositors do not provide this integration.
Windows/macOS/X11 keep their existing backends. Public Linux release CI remains paused.

The shared gesture engine runs inside niri before key/button delivery. The app
receives semantic actions over a private runtime socket; niri retains suppression,
replay, fullscreen checks and authoritative pointer/scale information. Native GTK
layer-shell surfaces have no keyboard interactivity and an empty input region.
Only Settings uses a WebView. Lock, inactive seat, topology change, expired heartbeat
or transport failure cancel interception. After a safety pause, explicitly resume
with the existing toggle shortcut; a failed transport requires restarting the app.

No root GUI, input/uinput group, capabilities, evdev proxy or automatic device grant.
**The custom compositor is currently authorized for disposable-VM testing only.**
Do not replace a host desktop compositor as part of these instructions.

## Build

From a checkout containing these tracked flake files, with flakes enabled:

```sh
nix flake check --no-build
nix build .#default
nix build .#patched-niri
```

Nix pins Rust/system libraries and builds the bundled Settings UI offline from
hashed npm dependencies and Cargo.lock. Node is build-time only. `wayland` is an
opt-in Cargo feature; the Nix package enables it. Packages are exposed for
`x86_64-linux` and `aarch64-linux`; the VM is x86_64/KVM only. An evaluated package
on another architecture is not a tested binary.

## Disposable native integration VM

Linux host with accessible KVM and a local display (including WSLg):

```sh
export PINGLOOM_VM_STATE="$HOME/.local/state/pingloom-native-vm"
nix run .#vm
```

Use a fresh state directory when switching from the stock baseline VM so old
manual user-service overrides cannot select the wrong compositor. The VM uses
4 vCPU, 6 GiB RAM, a sparse 16 GiB disk, virgl, and a local GTK viewer. There is no
TCP listener or SSH service. Only the source tree is shared, read-only; physical
host input devices are never passed through. The local guest-agent socket is
root-capable **inside this disposable guest**; keep its state directory private.

The root flake's VM installs the integrated compositor and the packaged Pingloom
user service. `nix/vm/flake.nix` remains a separate **stock-niri baseline**, not the
native gesture edition.

In another host terminal:

```sh
PINGLOOM_VM_STATE="$PINGLOOM_VM_STATE" python3 nix/vm/smoke.py
python3 nix/vm/gesture.py --state "$PINGLOOM_VM_STATE"
```

The gesture check uses only QMP virtual inputs and the local guest agent. It
checks visible held-Alt/drag/release rendering, passive layer/focus behavior,
fullscreen exclusion, foreign shortcuts, pause/resume, lease expiry and crash recovery.
A native Foot probe records only the injected guest input, proving that clicks
reach a real client even while a frozen app still has an overlay. The check freezes
and kills/restarts only the guest Pingloom user service. Screenshots are saved in
the private VM state directory. Do not operate the VM viewer during this check.
The script assumes default Settings and opens/closes its own guest Foot probe.

## Module / Settings

`nixosModules.default` declares `services.pingloom.enable` and `package`. It starts
only a graphical-session **user** service. Replacing niri requires both the explicit
`services.pingloom.niri.enable` flag and a non-null `niri.package`; no silent
compositor replacement or device permissions are installed. The root VM supplies
these options; do not apply them to the host under the current authorization.

niri has no built-in tray: a tray-capable bar is needed for the tray menu. In the
guest, Settings can also be opened by stopping the running service, then launching
`pingloom --settings` as `tester` from Foot. Quit that instance before restarting
the service. Do not run Settings as root. Config remains
`$XDG_CONFIG_HOME/pingloom/settings.json` (normally `~/.config/pingloom/settings.json`).

## Validation boundaries

Unit/VM checks are not real-hardware acceptance. Mixed-output fractional DPI,
physical hotplug, session-lock/VT transitions, compositor grabs and game pointer
constraints still need acceptance on actual niri hardware. Letter-key bindings use
evdev physical positions; verify labels/shortcuts on non-US keyboard layouts. The pinned compositor
patch deliberately fails to apply to an unexpected upstream source instead of
silently accepting a changed input path. See ADR 0002 and `docs/PLATFORMS.md`.
