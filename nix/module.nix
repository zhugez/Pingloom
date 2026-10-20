{ config, lib, pkgs, ... }:

let
  cfg = config.services.pingloom;
in {
  options.services.pingloom = {
    enable = lib.mkEnableOption "Pingloom native desktop ping and emote gestures user service";

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.pingloom or (pkgs.callPackage ./package.nix { });
      defaultText = lib.literalExpression "pkgs.pingloom";
      description = "The Pingloom package to use.";
    };

    niri = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = ''
          Opt-in integration to use the patched niri package (approved for VM-only testing).
          Must be explicitly enabled. This will NOT silently replace desktop niri.
        '';
      };

      package = lib.mkOption {
        type = lib.types.nullOr lib.types.package;
        default = null;
        description = ''
          The patched niri package to assign to programs.niri.package when
          services.pingloom.niri.enable is true.
        '';
      };
    };
  };

  config = lib.mkMerge [
    {
      assertions = [{
        assertion = !cfg.niri.enable || cfg.niri.package != null;
        message = "services.pingloom.niri.enable requires an explicitly selected patched niri package.";
      }];
    }
    (lib.mkIf cfg.enable {
      systemd.user.services.pingloom = {
        description = "Pingloom desktop ping and emote gestures";
        wantedBy = [ "graphical-session.target" ];
        partOf = [ "graphical-session.target" ];
        serviceConfig = {
          ExecStart = "${cfg.package}/bin/pingloom";
          Restart = "on-failure";
          RestartSec = "3s";
        };
      };
    })

    # Explicit opt-in only: do NOT silently override desktop niri
    (lib.mkIf (cfg.niri.enable && cfg.niri.package != null) {
      programs.niri.package = cfg.niri.package;
    })
  ];
}
