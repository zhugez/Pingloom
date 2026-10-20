# NixOS + niri test VM

Disposable x86_64 NixOS VM, pinned to Nixpkgs in `flake.lock` (niri 26.04).
This separate flake provisions a **stock-niri baseline**, not the native gesture
edition, and does not install Pingloom. For the integrated compositor, packaged
app and runnable gesture checks, use the root flake's `nix run .#vm` and
[`nix/README.md`](../README.md). Booting this baseline alone is not gesture acceptance.

## Host prerequisites

Linux with working, user-accessible `/dev/kvm`, Nix with flakes, and Python 3.
The generated launcher includes its own Nix-packaged QEMU. On Fedora WSL2,
the host tools can be installed with (run from a graphical WSLg session):

```sh
sudo dnf install nix nix-daemon qemu-system-x86-core qemu-ui-gtk qemu-ui-opengl qemu-device-display-virtio-gpu
sudo systemctl enable --now nix-daemon.socket nix-daemon.service
```

Do not disable package-signature verification if an unrelated repository fails.
KVM must actually work inside WSL2; a device filename alone is not enough.

## Start

From the Pingloom repository root:

```sh
nix flake check --no-build path:./nix/vm
nix run path:./nix/vm
```

The first build downloads the NixOS/QEMU/desktop closure and may take a while.
The VM gets 4 virtual CPUs, 6 GiB RAM and a sparse 16 GiB disk. A local GTK window
opens through WSLg; `$DISPLAY` and its X11 socket must be available. The **host
viewer** uses X11/llvmpipe with explicit Nix Mesa paths; the **guest compositor**
runs native niri/Wayland on virtio-gpu/virgl. No guest XWayland is enabled.
This is **not** a hardware-GPU or latency benchmark.

A plain non-GL virtio GPU was tested and rejected: niri 26.04 refuses software
EGL devices in its DRM backend. The launcher therefore uses virgl, not a patched
compositor or a niri-inside-another-compositor workaround. VNC is not enabled:
QEMU rejects combining its VNC display with this GTK GL context.

The guest logs in as `tester` and opens Foot in native niri. `Super+Return` opens
another terminal; `Super+Q` closes the focused window. The default console has
no password and `tester` can sudo **inside the guest**. Do not use this config
as a real machine's NixOS configuration or put credentials in it.

**Local test VM only:** no TCP listener is opened by the launcher. QMP and the
root-capable guest-agent sockets live in a private state directory; local
processes of your UID can control the guest.

The host source tree is mounted **read-only** at `/workspace/pingloom`; no host
home, credentials or physical input/GPU devices are passed through. The VM module
also exposes the host Nix store read-only (with a guest-local writable overlay).
As with any Nix VM, never put secrets in the Nix store. Default temporary exchange
shares are created by the upstream launcher; leave `SHARED_DIR` unset. Guest
external networking is restricted; SSH is disabled and no ports are forwarded.
No input/uinput groups, device ACLs or Pingloom interception are enabled.

Optional launcher environment:

- `PINGLOOM_SOURCE`: repository to mount read-only (defaults to current directory).
- `PINGLOOM_VM_STATE`: private VM state directory (default:
  `${XDG_STATE_HOME:-$HOME/.local/state}/pingloom-niri-vm`).

Only one instance can use a state directory.
Disk, serial log and QEMU sockets stay outside the repository.

## Check and stop

Once niri has booted:

```sh
python3 nix/vm/smoke.py
```

The check uses the private guest-agent socket to verify niri/greetd, Wayland
outputs and layer-shell, the read-only source mount, and no SSH/input groups.
It does not test Pingloom gesture parity, host hardware or full lock/recovery.

In a guest terminal, `sudo poweroff` shuts down the VM cleanly. Do not kill the
host Nix daemon to stop a guest. In another host terminal, QMP is also available
at `<state>/qmp.sock` for diagnostics and shutdown. Serial boot output is at
`<state>/serial.log`. Stop the VM before deleting its state to reset the guest.

## Verified in this setup

On Fedora 44/WSL2 with KVM, the pinned guest booted NixOS
`26.11.20261015.fe13242` and niri `26.04`. `nix flake check` (including launcher
ShellCheck and policy assertions), config validation and `smoke.py` passed.
The guest exposed layer-shell v5, fractional-scale v1, foreign-toplevel v3 and
an active 1280×768 output. Virtual `Super+Return` through QMP opened a second
Foot window, and a screenshot was captured directly through Wayland/Grim.
No host physical input interception or Pingloom gesture acceptance was tested.

The native edition adds the passive overlay, compositor-owned coordinates and
suppression/replay, and a heartbeat lease. Its separate acceptance gates are in
`../README.md`. Input checks must use virtual devices **inside the guest**, never
host device grabs. The shared runner optionally exposes output-only virtual audio
when a host PulseAudio socket is available; it never exposes a microphone.
