{ lib
, rustPlatform
, buildNpmPackage
, pkg-config
, wrapGAppsHook3
, gtk3
, webkitgtk_4_1
, libappindicator-gtk3
, alsa-lib
, libevdev
, libx11
, libxi
, libxtst
, gtk-layer-shell
, gst_all_1
, pingloomSrc ? ../.
}:

let
  root = pingloomSrc.origSrc or pingloomSrc;
  src = lib.cleanSourceWith {
    src = pingloomSrc;
    filter = path: type:
      let
        relative = lib.removePrefix "${toString root}/" (toString path);
        inputs = [ "Cargo.toml" "Cargo.lock" "build.rs" "tauri.conf.json" "LICENSE"
          "capabilities" "src" "assets" "icons" "ui" "docs/media/brand" "tools" ];
        needed = toString path == toString root || builtins.any
          (entry: relative == entry || lib.hasPrefix "${entry}/" relative || lib.hasPrefix "${relative}/" entry)
          inputs;
      in needed && lib.cleanSourceFilter path type
      && !(builtins.elem (baseNameOf path) [ "target" "node_modules" "dist" ]);
  };

  ui = buildNpmPackage {
    pname = "pingloom-ui";
    version = "0.1.0";
    src = src + "/ui";
    npmDepsHash = "sha256-dmNmBdpTE0Tm15OzPR7E495ePHBsWL4ZvhNwCD8ftVA=";
    dontNpmBuild = false;

    preBuild = ''
      mkdir -p ../docs/media/brand
      cp -r ${src}/docs/media/brand/* ../docs/media/brand/
    '';

    installPhase = ''
      runHook preInstall
      cp -pr --reflink=auto dist $out
      runHook postInstall
    '';
  };
in
rustPlatform.buildRustPackage {
  pname = "pingloom";
  version = "0.1.0";

  inherit src;

  cargoLock = {
    lockFile = pingloomSrc + "/Cargo.lock";
  };

  buildFeatures = [ "wayland" ];

  nativeBuildInputs = [
    pkg-config
    wrapGAppsHook3
  ];

  buildInputs = [
    gtk3
    webkitgtk_4_1
    libappindicator-gtk3
    alsa-lib
    libevdev
    libx11
    libxi
    libxtst
    gtk-layer-shell
    gst_all_1.gstreamer
    gst_all_1.gst-plugins-base
    gst_all_1.gst-plugins-good
  ];

  # Avoid whole-program LTO for this experimental package.
  env.CARGO_PROFILE_RELEASE_LTO = "off";
  env.CARGO_PROFILE_RELEASE_CODEGEN_UNITS = "16";

  preBuild = ''
    rm -rf ui/dist
    mkdir -p ui
    cp -r ${ui} ui/dist
  '';

  postInstall = ''
    install -Dm644 LICENSE $out/share/doc/pingloom/LICENSE
    if [ -f icons/icon.png ]; then
      install -Dm644 icons/icon.png $out/share/icons/hicolor/512x512/apps/pingloom.png
    fi
    mkdir -p $out/share/applications
    cat > $out/share/applications/pingloom.desktop <<EOF
[Desktop Entry]
Type=Application
Name=Pingloom
Comment=Native desktop ping and emote gestures
Exec=pingloom
Icon=pingloom
Terminal=false
Categories=Utility;
EOF
  '';

  preFixup = ''
    gappsWrapperArgs+=(
      --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath [
        libx11
        libxi
        libxtst
        libappindicator-gtk3
        gtk-layer-shell
      ]}"
    )
  '';

  meta = with lib; {
    description = "Native desktop ping and emote gestures";
    homepage = "https://github.com/zhugez/pingloom";
    license = licenses.mit;
    platforms = platforms.linux;
    mainProgram = "pingloom";
  };
}
