#!/usr/bin/env python3
"""Exercise only QEMU's virtual inputs. Requires the integrated niri and Pingloom guest."""
import argparse
import base64
import json
import os
from pathlib import Path
import re
import shlex
import socket
import struct
import time
import zlib


class RPC:
    def __init__(self, path, qmp=False):
        self.socket = socket.socket(socket.AF_UNIX)
        self.socket.settimeout(30)
        self.socket.connect(str(path))
        self.stream = self.socket.makefile("rwb")
        if qmp:
            self.stream.readline()
            self.call("qmp_capabilities")

    def call(self, command, arguments=None):
        self.stream.write(json.dumps({"execute": command, "arguments": arguments or {}}).encode() + b"\n")
        self.stream.flush()
        while True:
            reply = json.loads(self.stream.readline())
            if "error" in reply:
                raise RuntimeError(reply["error"])
            if "return" in reply:
                return reply["return"]

    def shell(self, script):
        pid = self.call("guest-exec", {
            "path": "/run/current-system/sw/bin/bash", "arg": ["-lc", script], "capture-output": True,
        })["pid"]
        deadline = time.monotonic() + 40
        while time.monotonic() < deadline:
            reply = self.call("guest-exec-status", {"pid": pid})
            if reply.get("exited"):
                out = base64.b64decode(reply.get("out-data", ""))
                err = base64.b64decode(reply.get("err-data", ""))
                if reply.get("exitcode") != 0 or reply.get("out-truncated"):
                    raise RuntimeError((out + err).decode(errors="replace")[:2000])
                return out
            time.sleep(0.05)
        raise TimeoutError("Guest command timed out")


def ppm(data):
    magic, dimensions, maximum, pixels = data.split(b"\n", 3)
    width, height = map(int, dimensions.split())
    assert magic == b"P6" and maximum == b"255" and len(pixels) == width * height * 3
    return width, height, pixels


def save_png(path, data):
    width, height, pixels = ppm(data)
    def chunk(kind, content):
        return struct.pack(">I", len(content)) + kind + content + struct.pack(">I", zlib.crc32(kind + content))
    rows = b"".join(b"\0" + pixels[y * width * 3:(y + 1) * width * 3] for y in range(height))
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))


def check(state, unit):
    qmp = RPC(state / "qmp.sock", qmp=True)
    guest = RPC(state / "agent.sock")
    user = "runuser -u tester -- env XDG_RUNTIME_DIR=/run/user/1000 "
    environment = """set -euo pipefail
for s in /run/user/1000/niri.*.sock; do test ! -S "$s" || export NIRI_SOCKET="$s"; done
for s in /run/user/1000/wayland-*; do test ! -S "$s" || export WAYLAND_DISPLAY="${s##*/}"; done
"""
    native = user + 'NIRI_SOCKET="$NIRI_SOCKET" WAYLAND_DISPLAY="$WAYLAND_DISPLAY" '
    def ipc(request):
        return json.loads(guest.shell(environment + native + "niri msg --json " + request))
    def layers():
        return [s for s in ipc("layers") if s["namespace"] == "pingloom-overlay"]
    def shot():
        encoded = guest.shell(environment + native + "grim -t ppm - | base64 -w0")
        return base64.b64decode(encoded)
    def events(*items):
        qmp.call("input-send-event", {"events": items})
    def key(code, down):
        events({"type": "key", "data": {"down": down, "key": {"type": "qcode", "data": code}}})
    def button(down):
        events({"type": "btn", "data": {"down": down, "button": "left"}})
    def position(x):
        events({"type": "abs", "data": {"axis": "x", "value": x}},
               {"type": "abs", "data": {"axis": "y", "value": 16000}})
    def toggle():
        for code in ("ctrl", "alt", "p"):
            key(code, True)
            time.sleep(0.05)
        for code in ("p", "alt", "ctrl"):
            key(code, False)
            time.sleep(0.05)
        time.sleep(0.2)
    def drag():
        position(12500)
        time.sleep(0.15)
        key("alt", True)
        time.sleep(0.15)  # USB pointer and virtual keyboard are separate devices.
        button(True)
        time.sleep(0.15)
        position(20500)
    def release():
        button(False)
        time.sleep(0.15)
        key("alt", False)
    def wait_hidden():
        deadline = time.monotonic() + 8
        while layers():
            assert time.monotonic() < deadline, "Overlay did not unmap after gesture/effect"
            time.sleep(0.2)

    assert re.fullmatch(r"[A-Za-z0-9_.@-]+", unit), "Invalid guest unit name"
    unit = shlex.quote(unit)
    def admitted():
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            pid = int(guest.shell(user + "systemctl --user show " + unit + " -p MainPID --value"))
            if pid > 0 and b"Native niri gesture bridge ready" in guest.shell(f"journalctl _PID={pid} --no-pager"):
                return pid
            time.sleep(0.2)
        raise TimeoutError("App did not acknowledge compositor admission")
    pid = admitted()
    # Start from a known virtual key state; never touch host input devices.
    for code in ("alt", "alt_r", "ctrl", "ctrl_r", "shift", "shift_r", "meta_l", "meta_r"):
        key(code, True)
        key(code, False)
    release()
    wait_hidden()
    old_focus = ipc("focused-window")
    probe = "/tmp/pingloom-virtual-input-probe"
    guest.shell(user + "mkdir -p -m700 " + probe + "; " + user + "rm -f " + probe + "/input")
    command = "stty -echo -icanon -isig; printf '\\033[?1000h\\033[?1006hPingloom virtual input probe\\n'; cat >" + probe + "/input"
    guest.shell(environment + native + "niri msg action spawn -- foot --app-id=pingloom-input-probe sh -c " + shlex.quote(command))
    deadline = time.monotonic() + 10
    while True:
        focus = ipc("focused-window")
        if focus and focus["app_id"] == "pingloom-input-probe" and focus != old_focus:
            break
        assert time.monotonic() < deadline, "Virtual input probe did not map"
        time.sleep(0.1)
    guest.shell(environment + native + "niri msg action set-column-width '100%'")
    time.sleep(0.5)
    def mouse_events():
        data = base64.b64decode(guest.shell("base64 -w0 " + probe + "/input"))
        return data.count(b"\x1b[<")
    before = shot()
    save_png(state / "gesture-before.png", before)
    try:
        drag()
        deadline = time.monotonic() + 15
        while True:
            # Keep the physical hold live while a debug renderer prepares its first frame.
            position(20501)
            held = shot()
            changed = sum(a != b for a, b in zip(ppm(before)[2], ppm(held)[2])) // 3
            if layers() and changed > 5000:
                break
            assert time.monotonic() < deadline, "No visible native wheel during held-trigger drag"
            time.sleep(0.2)
        surfaces = layers()
        assert all(s["layer"] == "Overlay" and s["keyboard_interactivity"] == "None" for s in surfaces)
        assert ipc("focused-window")["id"] == focus["id"], "Overlay stole keyboard focus"
        save_png(state / "gesture-held.png", held)
    finally:
        release()
    assert layers(), "Selection effect did not retain a native overlay"
    received = mouse_events()
    button(True)
    time.sleep(0.15)
    button(False)
    time.sleep(0.15)
    assert mouse_events() >= received + 2, "Overlay input region blocked native mouse delivery"
    wait_hidden()
    assert ipc("focused-window")["id"] == focus["id"]
    print("PASS: held Alt/drag/release paints the native wheel/effect; focus and mouse pass-through preserved")

    guest.shell(environment + native + "niri msg action fullscreen-window")
    try:
        time.sleep(0.3)
        drag()
        time.sleep(0.5)
        assert not layers(), "Fullscreen exclusion did not pass input through"
    finally:
        release()
        guest.shell(environment + native + "niri msg action fullscreen-window")
    print("PASS: compositor fullscreen state excludes the gesture")

    key("ctrl", True)
    time.sleep(0.15)
    key("c", True)
    key("c", False)
    time.sleep(0.15)
    try:
        button(True)
        position(12500)
        time.sleep(0.4)
        assert not layers(), "Foreign Ctrl+C shortcut incorrectly armed a wheel"
    finally:
        button(False)
        time.sleep(0.15)
        key("ctrl", False)
    assert b"\x03" in base64.b64decode(guest.shell("base64 -w0 " + probe + "/input")), "Ctrl+C did not reach the native client"
    print("PASS: a foreign shortcut reaches the native client without arming a wheel")

    toggle()
    try:
        drag()
        time.sleep(0.5)
        assert not layers(), "Paused input still opened a wheel"
    finally:
        release()
        toggle()
    print("PASS: pause passes ordinary virtual input through; shortcut resumes")

    # Stop the whole app, including its heartbeat thread. Fresh Alt-clicks must
    # reach the native client after the lease, even while the stale overlay remains.
    drag()
    try:
        deadline = time.monotonic() + 15
        while not layers():
            assert time.monotonic() < deadline, "Wheel did not reopen before lease test"
            position(20501)
            time.sleep(0.2)
        guest.shell(f"kill -STOP {pid}")
        time.sleep(2)
        button(False)  # Drain the suppressed release from the cancelled gesture.
        time.sleep(0.15)
        received = mouse_events()
        button(True)
        time.sleep(0.15)
        button(False)
        time.sleep(0.15)
        assert mouse_events() >= received + 2, "Expired app lease still intercepted native input"
    finally:
        release()
        guest.shell(f"kill -CONT {pid}")
        wait_hidden()
        guest.shell(user + "systemctl --user restart " + unit)
        pid = admitted()
    print("PASS: a frozen app loses its input lease; native clicks pass through before it resumes")

    # Kill only the guest user service while holding; the compositor must survive.
    drag()
    try:
        deadline = time.monotonic() + 15
        while not layers():
            assert time.monotonic() < deadline, "Wheel did not reopen before crash test"
            position(20501)
            time.sleep(0.2)
        guest.shell(user + "systemctl --user kill --signal=KILL " + unit)
        time.sleep(1.8)
        assert not layers(), "Dead app retained overlay surfaces"
        assert ipc("focused-window")["id"] == focus["id"], "Compositor/focus lost after app crash"
    finally:
        release()
        guest.shell(user + "systemctl --user restart " + unit)
        admitted()
    guest.shell(environment + native + "niri msg action close-window --id " + str(focus["id"]))
    print("PASS: guest app crash leaves compositor/focus alive; service restarted")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--state", type=Path, default=Path(os.environ.get("XDG_STATE_HOME", Path.home() / ".local/state")) / "pingloom-niri-vm")
    parser.add_argument("--unit", default="pingloom")
    args = parser.parse_args()
    check(args.state.resolve(), args.unit)
