{ pkgs, guest }:
pkgs.writeShellApplication {
  name = "pingloom-niri-vm";
  runtimeInputs = [ pkgs.coreutils pkgs.util-linux ];
  text = ''
    export PINGLOOM_SOURCE
    PINGLOOM_SOURCE=$(realpath "''${PINGLOOM_SOURCE:-$PWD}")
    if [[ ! -f "$PINGLOOM_SOURCE/Cargo.toml" ]]; then
      echo "Run from the Pingloom root, or set PINGLOOM_SOURCE to it." >&2
      exit 1
    fi
    state="''${PINGLOOM_VM_STATE:-''${XDG_STATE_HOME:-$HOME/.local/state}/pingloom-niri-vm}"
    umask 077
    mkdir -p "$state"
    state=$(realpath "$state")
    if [[ ! -O "$state" || $(stat -c %a "$state") != 700 ]]; then
      echo "VM state must be a user-owned directory with mode 700." >&2
      exit 1
    fi
    exec 9>"$state/lock"
    flock -n 9 || { echo "This VM state is already in use." >&2; exit 1; }
    cd "$state"
    export NIX_DISK_IMAGE="$state/guest.qcow2"
    # Ensure WSLg X11 socket is visible even immediately after a host reboot.
    if [[ ! -e /tmp/.X11-unix && -d /mnt/wslg/.X11-unix ]]; then
      ln -snf /mnt/wslg/.X11-unix /tmp/.X11-unix
    fi
    # Explicit Mesa paths also work on non-NixOS hosts without /run/opengl-driver.
    export GDK_BACKEND=x11 LIBGL_ALWAYS_SOFTWARE=1
    export __EGL_VENDOR_LIBRARY_FILENAMES=${pkgs.mesa}/share/glvnd/egl_vendor.d/50_mesa.json
    export LIBGL_DRIVERS_PATH=${pkgs.mesa}/lib/dri
    # Output-only virtual audio: no guest microphone or host input device access.
    audio=()
    if [[ -z "''${PULSE_SERVER:-}" && -S /mnt/wslg/PulseServer ]]; then
      export PULSE_SERVER=unix:/mnt/wslg/PulseServer
    elif [[ -z "''${PULSE_SERVER:-}" && -S "''${XDG_RUNTIME_DIR:-/nonexistent}/pulse/native" ]]; then
      export PULSE_SERVER="unix:$XDG_RUNTIME_DIR/pulse/native"
    fi
    if [[ -n "''${PULSE_SERVER:-}" ]]; then
      audio=(-audiodev "pa,id=pingloom-audio" -device ich9-intel-hda -device "hda-output,audiodev=pingloom-audio")
    else
      echo "No host PulseAudio socket; VM audio disabled." >&2
    fi
    echo "Opening the VM in a local GTK window (WSLg on Windows). No TCP listener."
    echo "Logs: $state/serial.log; QMP: $state/qmp.sock; guest agent: $state/agent.sock"
    exec ${guest.config.system.build.vm}/bin/run-pingloom-niri-vm \
      -display gtk,gl=on \
      -serial "file:$state/serial.log" \
      -qmp "unix:$state/qmp.sock,server=on,wait=off" \
      -device virtio-serial-pci \
      -chardev "socket,id=agent,path=$state/agent.sock,server=on,wait=off" \
      -device virtserialport,chardev=agent,name=org.qemu.guest_agent.0 \
      "''${audio[@]}" "$@"
  '';
}
