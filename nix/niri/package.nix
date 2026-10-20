{ pkgs, src }:
let
  base = pkgs.niri;
  root = src.origSrc or src;
  core = pkgs.lib.cleanSourceWith {
    inherit src;
    filter = path: _type:
      let
        relative = pkgs.lib.removePrefix "${toString root}/" (toString path);
        inputs = [ "assets" "nix/niri/compositor.rs" "src/lib.rs" "src/model.rs"
          "src/gesture.rs" "src/geometry.rs" "src/settings.rs" "src/niri_bridge.rs" ];
      in toString path == toString root || builtins.any
        (entry: relative == entry || pkgs.lib.hasPrefix "${entry}/" relative || pkgs.lib.hasPrefix "${relative}/" entry)
        inputs;
  };
  # Registry/git dependencies are unchanged: only a local copy of the shared core is added.
  # Keep the original, hash-verified vendor tree and replace its lockfile deterministically.
  vendor = pkgs.runCommand "niri-pingloom-vendor" { } ''
    mkdir -p "$out"
    cp -rs ${base.cargoDeps}/. "$out/"
    rm "$out/Cargo.lock"
    cp ${./Cargo.lock} "$out/Cargo.lock"
  '';
in base.overrideAttrs (old: {
  cargoDeps = vendor;
  nativeBuildInputs = old.nativeBuildInputs ++ [ pkgs.python3 ];
  postPatch = old.postPatch + ''
    ${pkgs.python3}/bin/python3 ${./apply.py} "$PWD" ${core}
    # The checked-in lockfile is also used by the vendor closure.
    cp ${./Cargo.lock} Cargo.lock
  '';
  env = old.env // { NIRI_BUILD_COMMIT = "Pingloom-bridge-v1"; };
})
