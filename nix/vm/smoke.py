#!/usr/bin/env python3
"""Check the running disposable VM through its local QEMU guest-agent socket."""
import base64
import json
import os
from pathlib import Path
import socket
import time


def check(state):
    deadline = time.monotonic() + 180
    with socket.socket(socket.AF_UNIX) as sock:
        sock.settimeout(10)
        while True:
            try:
                sock.connect(str(state / "agent.sock"))
                break
            except (FileNotFoundError, ConnectionRefusedError):
                if time.monotonic() >= deadline:
                    raise TimeoutError("VM guest-agent socket did not appear") from None
                time.sleep(0.2)
        stream = sock.makefile("rwb")

        def rpc(command, arguments):
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("Guest smoke check timed out")
            sock.settimeout(remaining)
            stream.write(json.dumps({"execute": command, "arguments": arguments}).encode() + b"\n")
            stream.flush()
            response = json.loads(stream.readline())
            if "error" in response:
                raise RuntimeError(response["error"])
            return response["return"]

        script = """
set -euo pipefail
timeout 90 bash -c 'until compgen -G "/run/user/1000/niri.*.sock" >/dev/null; do sleep 1; done'
systemctl is-active greetd
pgrep -u tester -x niri
nixos-version
niri --version
if systemctl is-active --quiet sshd; then echo 'Unexpected SSH service'; exit 1; fi
if id -nG tester | grep -Eq '(^| )(input|uinput)( |$)'; then echo 'Unexpected input grant'; exit 1; fi
findmnt -n -o OPTIONS /workspace/pingloom | grep -Eq '(^|,)ro(,|$)'
socket=$(find /run/user/1000 -maxdepth 1 -name 'niri.*.sock' -print -quit)
test -n "$socket"
wayland=$(find /run/user/1000 -maxdepth 1 -type s -name 'wayland-*' -printf '%f\\n' | head -n 1)
test -n "$wayland"
runuser -u tester -- env XDG_RUNTIME_DIR=/run/user/1000 NIRI_SOCKET="$socket" niri msg --json outputs | jq -e 'select(length > 0) | map_values({name, logical, current_mode})'
runuser -u tester -- env XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY="$wayland" wayland-info | grep -E "interface: '(wl_output|zwlr_layer_shell_v1|wp_fractional_scale_manager_v1|zwlr_foreign_toplevel_manager_v1)'"
"""
        pid = rpc("guest-exec", {
            "path": "/run/current-system/sw/bin/bash",
            "arg": ["-lc", script],
            "capture-output": True,
        })["pid"]
        while time.monotonic() < deadline:
            result = rpc("guest-exec-status", {"pid": pid})
            if result.get("exited"):
                output = base64.b64decode(result.get("out-data", "")).decode(errors="replace")
                errors = base64.b64decode(result.get("err-data", "")).decode(errors="replace")
                if result.get("exitcode") != 0:
                    raise RuntimeError(output + errors)
                print(output)
                assert "zwlr_layer_shell_v1" in output, "Layer-shell unavailable"
                assert "wl_output" in output, "No Wayland output"
                print("PASS: niri running, layer-shell/output present, source read-only, no SSH/input grants")
                return
            time.sleep(0.2)
        raise TimeoutError("Guest smoke check timed out")


if __name__ == "__main__":
    default = Path(os.environ.get("XDG_STATE_HOME", Path.home() / ".local/state")) / "pingloom-niri-vm"
    check(Path(os.environ.get("PINGLOOM_VM_STATE", default)))
