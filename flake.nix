{
  description = "Pingloom native desktop gestures, including opt-in niri integration";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/fe13242281a67641c0102cdb1087f0ebe586c7df";
  outputs = { self, nixpkgs }:
    let
      eachSystem = nixpkgs.lib.genAttrs [ "x86_64-linux" "aarch64-linux" ];
    in {
      packages = eachSystem (system:
        let pkgs = nixpkgs.legacyPackages.${system}; in {
          default = pkgs.callPackage ./nix/package.nix { pingloomSrc = self; };
          pingloom = self.packages.${system}.default;
          patched-niri = import ./nix/niri/package.nix { inherit pkgs; src = self; };
        } // pkgs.lib.optionalAttrs (system == "x86_64-linux") {
          vm-runner = import ./nix/vm/runner.nix {
            inherit pkgs;
            guest = self.nixosConfigurations.niri-vm;
          };
        });
      apps.x86_64-linux.vm = {
        type = "app";
        program = "${self.packages.x86_64-linux.vm-runner}/bin/pingloom-niri-vm";
        meta.description = "Disposable VM with the native Pingloom/niri edition";
      };
      nixosConfigurations.niri-vm = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [ ./nix/vm/niri.nix ./nix/module.nix {
          services.pipewire = { enable = true; alsa.enable = true; pulse.enable = true; };
          services.pingloom = {
            enable = true;
            package = self.packages.x86_64-linux.default;
            niri.enable = true;
            niri.package = self.packages.x86_64-linux.patched-niri;
          };
        } ];
      };
      checks.x86_64-linux.vm-policy =
        let c = self.nixosConfigurations.niri-vm.config;
        in assert !c.services.openssh.enable;
           assert !c.hardware.uinput.enable;
           assert !c.virtualisation.sharedDirectories.repo.writable;
           assert c.users.users.tester.extraGroups == [ "wheel" ];
           assert !(c.systemd.services ? pingloom);
           assert c.services.pingloom.enable;
           assert c.programs.niri.package.outPath == self.packages.x86_64-linux.patched-niri.outPath;
           nixpkgs.legacyPackages.x86_64-linux.runCommand "pingloom-native-vm-policy" { } ''touch "$out"'';
      devShells = eachSystem (system:
        let pkgs = nixpkgs.legacyPackages.${system}; in {
          default = pkgs.mkShell {
            nativeBuildInputs = [ pkgs.pkg-config pkgs.cargo pkgs.rustc pkgs.rustfmt pkgs.clippy pkgs.nodejs_22 ];
            buildInputs = (self.packages.${system}.default.buildInputs or []);
          };
        });
      nixosModules.default = import ./nix/module.nix;
      overlays.default = final: _prev: {
        pingloom = final.callPackage ./nix/package.nix { pingloomSrc = self; };
      };
    };
}
