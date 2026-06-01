{ pkgs }:

let
  lib = pkgs.lib;
  tauriRuntimeDeps = with pkgs; [
    alsa-lib
    at-spi2-atk
    atk
    cairo
    expat
    fontconfig
    freetype
    gdk-pixbuf
    glib
    glib-networking
    gtk3
    harfbuzz
    libayatana-appindicator
    libdrm
    libgbm
    libgpg-error
    libGL
    libinput
    librsvg
    libsoup_3
    libxkbcommon
    openssl
    pango
    fribidi
    systemd
    stdenv.cc.cc.lib
    webkitgtk_4_1
    wayland
    zlib
    libx11
    libxcb
    libxcursor
    libxext
    libxi
    libxkbfile
    libxrandr
  ];
  libraryPath = lib.makeLibraryPath tauriRuntimeDeps;
  gsettingsSchemas = lib.makeSearchPathOutput "out" "share/gsettings-schemas" [
    pkgs.gsettings-desktop-schemas
    pkgs.gtk3
  ];
  pkgConfigWrapper = pkgs.writeShellScriptBin "pkg-config" ''
    has_libs_only_l=0
    wants_appindicator=0
    wants_gio=0
    wants_schemasdir=0

    for arg in "$@"; do
      if [ "$arg" = "--libs-only-L" ]; then
        has_libs_only_l=1
      fi
      if [ "$arg" = "--variable=schemasdir" ]; then
        wants_schemasdir=1
      fi
      if [ "$arg" = "gio-2.0" ]; then
        wants_gio=1
      fi
      if [ "$arg" = "ayatana-appindicator3-0.1" ] || [ "$arg" = "appindicator3-0.1" ]; then
        wants_appindicator=1
      fi
    done

    if [ "$wants_schemasdir" = "1" ] && [ "$wants_gio" = "1" ]; then
      echo "${pkgs.glib.dev}/share/glib-2.0/schemas"
      exit 0
    fi

    if [ "$has_libs_only_l" = "1" ] && [ "$wants_appindicator" = "1" ]; then
      echo "-L${pkgs.libayatana-appindicator}/lib"
      exit 0
    fi

    exec ${pkgs.pkg-config}/bin/pkg-config "$@"
  '';
  cpWrapper = pkgs.writeShellScriptBin "cp" ''
    target_dir=""
    wants_target_value=0

    for arg in "$@"; do
      if [ "$wants_target_value" = "1" ]; then
        target_dir="$arg"
        wants_target_value=0
        continue
      fi

      case "$arg" in
        --target-directory=*)
          target_dir="''${arg#--target-directory=}"
          ;;
        --target-directory)
          wants_target_value=1
          ;;
      esac
    done

    ${pkgs.coreutils}/bin/cp "$@"
    status=$?

    if [ "$status" = "0" ] && [ -n "$target_dir" ]; then
      chmod -R u+w "$target_dir" 2> /dev/null || true
    fi

    exit "$status"
  '';
  findWrapper = pkgs.writeShellScriptBin "find" ''
    has_print0=0

    for arg in "$@"; do
      if [ "$arg" = "-print0" ]; then
        has_print0=1
      fi
    done

    if [ "$has_print0" = "1" ]; then
      ${pkgs.findutils}/bin/find "$@" | ${pkgs.gnugrep}/bin/grep -z -v -- '-gdb\.py$'
      exit "''${PIPESTATUS[0]}"
    fi

    ${pkgs.findutils}/bin/find "$@" | ${pkgs.gnugrep}/bin/grep -v -- '-gdb\.py$'
    exit "''${PIPESTATUS[0]}"
  '';
in
pkgs.mkShell {
  packages =
    with pkgs;
    [
      cpWrapper
      findWrapper
      cargo
      clippy
      file
      gcc
      gobject-introspection
      nodejs_22
      pkgConfigWrapper
      pkg-config
      rustc
      rustfmt
      wrapGAppsHook4
    ]
    ++ lib.optionals (pkgs ? linuxdeploy) [ pkgs.linuxdeploy ]
    ++ lib.optionals (pkgs ? cargo-tauri) [ pkgs.cargo-tauri ]
    ++ lib.optionals (pkgs ? appimagekit) [ pkgs.appimagekit ];

  buildInputs = tauriRuntimeDeps;

  RUSTFLAGS = "-C link-arg=-Wl,-rpath,${libraryPath}";
  LD_LIBRARY_PATH = libraryPath;
  PKG_CONFIG = "${pkgConfigWrapper}/bin/pkg-config";
  XDG_DATA_DIRS = "${gsettingsSchemas}:$XDG_DATA_DIRS";
  WEBKIT_DISABLE_COMPOSITING_MODE = "1";
  WEBKIT_DISABLE_DMABUF_RENDERER = "1";
  GDK_BACKEND = "x11";
  APPIMAGE_EXTRACT_AND_RUN = "1";

  shellHook = ''
    export CC=${pkgs.gcc}/bin/gcc
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$CC"
    export CARGO_TARGET_DIR="$PWD/target"
    echo "Keyboard Dances dev shell"
    echo "Tauri dev:      cargo tauri dev"
    echo "Cargo run:      cargo run --manifest-path src-tauri/Cargo.toml"
    echo "AppImage build: scripts/build-appimage-nixos.sh"
  '';
}
