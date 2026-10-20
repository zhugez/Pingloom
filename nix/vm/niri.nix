{ config, modulesPath, pkgs, ... }:
{
  imports = [ (modulesPath + "/virtualisation/qemu-vm.nix") ];

  networking.hostName = "pingloom-niri";
  system.stateVersion = "26.05";

  virtualisation = {
    cores = 4;
    memorySize = 6144;
    diskSize = 16384;
    graphics = true;
    restrictNetwork = true;
    qemu.forceAccel = true;
    qemu.options = [ "-vga none" "-device virtio-gpu-gl-pci" ];
    sharedDirectories.repo = {
      source = ''"$PINGLOOM_SOURCE"'';
      target = "/workspace/pingloom";
      writable = false;
    };
  };

  # Only a disposable, local VM: no SSH, host input devices, or device grants.
  users.users.tester = {
    isNormalUser = true;
    uid = 1000;
    extraGroups = [ "wheel" ];
    initialPassword = "";
  };
  users.users.root.hashedPassword = "!";
  security.sudo.wheelNeedsPassword = false;

  programs.niri = {
    enable = true;
    useNautilus = false;
  };
  hardware.graphics.enable = true;
  # Virgl gives niri a GPU; only the host viewer uses software rendering.
  services.greetd = {
    enable = true;
    settings = {
      initial_session = { command = "${config.programs.niri.package}/bin/niri-session"; user = "tester"; };
      default_session = {
        command = "${pkgs.greetd}/bin/agreety --cmd ${config.programs.niri.package}/bin/niri-session";
        user = "greeter";
      };
    };
  };

  environment.systemPackages = with pkgs; [ foot grim jq wayland-utils ];
  environment.etc."niri/config.kdl".text = ''
    input {
      keyboard { xkb { layout "us"; }; };
      mouse { accel-profile "flat"; };
    }
    output "Virtual-1" { mode "1280x768"; }
    hotkey-overlay { skip-at-startup; }
    prefer-no-csd
    spawn-at-startup "foot"
    binds {
      Mod+Return { spawn "foot"; }
      Mod+Q { close-window; }
      Mod+Shift+E { quit; }
      Mod+Left { focus-column-left; }
      Mod+Right { focus-column-right; }
    }
  '';
}
