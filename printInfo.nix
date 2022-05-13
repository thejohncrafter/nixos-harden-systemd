{ pkgs ? import <nixpkgs> {}, lib ? pkgs.lib }:
let
  mytests = import <nixpkgs/nixos/tests/all-tests.nix> { system=builtins.currentSystem; inherit pkgs; callTest = lib.id; };
  modules = import <nixpkgs/nixos/modules/module-list.nix>;
in rec {
  allTests = lib.attrNames mytests;
  testDeps = test:
    let
      mytest = mytests.${test};
      findDeps = node:
        let f = k: v: 
          let v' = node.config.systemd.services.${k};
          in (v'.visible or true) && (v'.enable.visible or true) && (v.enable or false);
        in builtins.attrNames (lib.filterAttrs f node.config.systemd.services);
      discoverTests = f:
        let go = val:
          if !lib.isAttrs val then [ ]
          else if lib.hasAttr "test" val then f val
          else lib.concatMap lib.id (lib.mapAttrsToList (lib.const go) val);
        in x: lib.unique (go x);
    in discoverTests (mytest: lib.concatMap lib.id (lib.attrValues (lib.mapAttrs (lib.const findDeps) mytest.nodes))) mytest;
  allServices = builtins.attrNames
      (lib.evalModules {
        modules = modules ++ [( { ... }: {
            _module.check = false;
            nixpkgs.system = "x86_64-linux";
            nixpkgs.config.allowBroken = true;
          } )];
        }).options.services;
  printServiceConfig = name:
    (import <nixpkgs/nixos/lib/eval-config.nix> {
      modules = [ ({ ... }: {
          #_module.check = false;
          services.${name}.enable = true;
        })];
      }).config.systemd.services.${name}.serviceConfig;
  collectJobs = x:
    let go = acc: prefix: x: lib.foldr (name: acc:
      let
        path = "${prefix}${name}.";
        val = x."${name}";
      in if lib.isDerivation val
        then [ { name = path; value = val; } ] ++ acc
        else if lib.isAttrs val
          then go acc path val
          else acc) acc (lib.attrNames x);
    in builtins.listToAttrs (go [] "." x);
  mkSystemdPassthru = collectedTests:
    let tests = builtins.fromJSON (builtins.readFile collectedTests);
    in lib.mapAttrs (_: value: builtins.listToAttrs (map (name: lib.nameValuePair name false) value.fields)) tests;
  mkOverrideOptions = defaultPassthru: service: override:
    defaultPassthru // { "${service}" = defaultPassthru."${service}" // override; };
  mkHookedTests = nixpkgs: systemdPassthru:
    let allTests = import "${toString nixpkgs}/nixos/tests/all-tests.nix" {
      inherit systemdPassthru;
      system = builtins.currentSystem;
      pkgs = import (toString nixpkgs) { system = builtins.currentSystem; };
      callTest = t: lib.hydraJob t.test;
    };
    in allTests;
  mkOverridableTests = collectedTests: nixpkgs: excluded:
    let
      tests = builtins.fromJSON (builtins.readFile collectedTests);
      defaultPassthru = mkSystemdPassthru collectedTests;
    in lib.mapAttrs (name: info: override:
      let passthru = mkOverrideOptions defaultPassthru name override;
      in lib.mapAttrsToList (name: value: lib.nameValuePair name value) (collectJobs (builtins.listToAttrs (map (
        test: lib.nameValuePair test (mkHookedTests nixpkgs passthru)."${test}"
      ) (lib.filter (name: lib.all (excl: excl != name) excluded) info.tests))))
    ) tests;
}
