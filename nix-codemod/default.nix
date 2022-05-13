{ pkgs ? import <nixpkgs> {} }:
pkgs.rustPlatform.buildRustPackage rec {
  name = "nix-codemod";
  src = ./.;
  cargoLock = {
    lockFile = ./Cargo.lock;
  };
}
