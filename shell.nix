let
  pkgs = import <nixpkgs> {};
in
pkgs.mkShell {
  buildInputs = [
	pkgs.systemd
    pkgs.fontconfig
    pkgs.freetype
	pkgs.pkg-config
    pkgs.libinput
    # pkgs.wayland  # Commented out for X11 switch
	pkgs.libglvnd
    pkgs.libxkbcommon
	pkgs.gcc
	pkgs.binutils
	pkgs.openssl
	pkgs.alsa-lib
	# X11 libraries
	pkgs.xorg.libX11
	pkgs.xorg.libXext
  ];
  env = {
    RUSTFLAGS = "-C link-arg=-Wl,-rpath,${pkgs.lib.makeLibraryPath [
      pkgs.fontconfig
      pkgs.freetype
      # pkgs.wayland  # Commented out for X11 switch
      pkgs.libinput
	  pkgs.libglvnd
	  pkgs.libxkbcommon
	  pkgs.gcc
	  pkgs.binutils
	  # X11 libraries
	  pkgs.xorg.libX11
	  pkgs.xorg.libXext
    ]}";
  };
  shellHook = ''
      export CC=${pkgs.gcc}/bin/gcc
	  export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$CC"
   '';
}
