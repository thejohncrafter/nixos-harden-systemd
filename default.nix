{ pkgs ? import <nixpkgs> {}, ... }:
let
  nix-codemod = import ./nix-codemod { inherit pkgs; };
in {
  shell = pkgs.mkShell {
    buildInputs = with pkgs; [ oil jq nix-codemod ];
  };
}
