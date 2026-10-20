{
  description = "Disposable NixOS/niri integration VM for Pingloom";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/fe13242281a67641c0102cdb1087f0ebe586c7df";

  outputs = { nixpkgs, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      guest = nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [ ./niri.nix ];
      };
      runner = import ./runner.nix { inherit pkgs guest; };
    in {
      nixosConfigurations.niri-vm = guest;
      packages.${system} = {
        default = guest.config.system.build.vm;
        inherit runner;
      };
      apps.${system}.default = {
        type = "app";
        program = "${runner}/bin/pingloom-niri-vm";
        meta.description = "Local-only NixOS/niri integration VM";
      };
      checks.${system} = {
        inherit runner;
        vm-policy =
          assert !guest.config.services.openssh.enable;
          assert !guest.config.hardware.uinput.enable;
          assert !guest.config.virtualisation.sharedDirectories.repo.writable;
          assert guest.config.users.users.tester.extraGroups == [ "wheel" ];
          pkgs.runCommand "pingloom-vm-policy" { } ''touch "$out"'';
      };
    };
}
