f: {
  systemdPassthru,
  system ? builtins.currentSystem,
  pkgs ? import ../.. { inherit system; },
  ...
} @ args:

with import ../lib/testing-python.nix { inherit system pkgs; specialArgs = { inherit systemdPassthru; }; };

makeTest (if pkgs.lib.isFunction f then f (args // { inherit pkgs; inherit (pkgs) lib; }) else f)
