let
  pkgs = import <nixpkgs> {};
in
import ./nix/dev-shell.nix { inherit pkgs; }
