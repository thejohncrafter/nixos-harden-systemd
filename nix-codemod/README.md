# nix-codemod

A specific tool to edit nixpkgs, modify systemd services, place hooks on their config and rewrite whole modules.

### List Systemd Services

### Edit Systemd Service Config

### Place Hooks in Service Config

```nix
{ pkgs, ...}:
{
  options = ...;
  config.systemd.services.myservice.serviceConfig = {
    Foo = "Bar";
  };
}
```
becomes
```nix
{ pkgs, systemdPassthru ...}:
{
  options = ...;
  config.systemd.services.myservice.serviceConfig = {
    Foo = "Bar";
    PrivateDevices = systemdPassthru.myservice.PrivateDevices;
  };
}
```

### Collect All the Test Definitions From `all-tests`

## Check Some Test is "Well-Formed"

This means that this tests will accept the `systemdPassthru` parameter

