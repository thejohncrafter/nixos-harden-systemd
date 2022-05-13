# Hardening Systemd Services

This project is supported by the NLnet foundation and the NGI Assure fund.

The goal is to add security configuration to as many services as possible in NixOS.

Idea :
> Let's say a service is well configured (works) if all of its tests pass,
> then we can find the best possible configuration by simply trying every possibility
> and finding the most secure combination that still works.

Strategy & Technical Overview:
 * Find all systemd services
 * Then for each service, find all the tests that activate it
    * First find all the modules by evaluating `nixpkgs/nixos/modules/module-list`
    * Then, parse all of these modules to find what systemd services it declares
    * This is done by `./run.oil discover-systemd-services ...`
    * `./run.oil test-deps ...` finds which service each test activates
    * `./run.oil collect-tests ...` will use the output of `./run.oil test-deps` and
      `./run.oil discover-systemd-services` to create a file that show what test
      correspond to each service.
 * Add hooks on these services that enable us to choose
   the hardening options of these service when we run tests
    * Once again, we use `module-list` and `./run.oil discover-systemd-services` to find
      what are the services and where they are defined
    * Add hooks on these services: we create a new argument called `systemdPassthru`
      that will be passed when we'll call tests
    * This is the job of `./run.oil hook-modules ...`
 * Test each service against each possible configuration!
    * With `./run.oil run-specific-tests`, which takes the data we gathered previously
      in input, along with the name of a service and a JSON file that specifies which hardening
      options to test the service against.

How to run (while waiting for some better docs):
 * `nix-shell`
 * `mkdir output`
 * Clone nixpkgs
    * `cd output`
    * `git clone <the version you want>`
    * We want `nixpkgs` in `./output/nixpkgs` (that's what the rest of these instructions assume)
 * `./run.oil discover-systemd-services ./output/nixpkgs ./output/systemd-services`
    * Look for service definitions in modules, save the result.
 * `./run.oil test-deps ./output/nixpkgs output/test-deps`
    * Find the systemd services each test activates
    * Takes a while because it requires evaluating each test (not *running*
      but *evaluating* to compute the resulting configuration -- no download,
      but takes a while).
 * `./run.oil collect-tests ./output/systemd-services ./output/test-deps ./output/tests`
    * Creates the file that associate each systemd service with its origin file,
      the hardening options that should be tested, and all the tests that should be run
      to check the service still works properly with the sandboxing options.
 * `./run.oil hook-modules ./output/nixpkgs ./output/tests`
    * Adds a `systemdPassthru` to the modules (and to some test-related files):
      we can now select the hardening options of every single service!
      (Well, to be precide, each service that we detected and that is well-behaved...
      from my tests, this covers 60%-70% of all modules.)
 * Test a service!
   * Write custom hardening options to `./options.json`, e.g. `{ "PrivateDevices": true }`
   * `./run.oil dry-run-specific-tests ./output/tests ./output/nixpkgs ./output/malformed-tests ./options.json hydra-server`
   * This will `hydra-server`'s tests!
   * Try some options (edit `./options.json`):
      * `{ "PrivateDevices": true }`: Still works!
      * `{ "PrivateNetwork": true }`: Breaks!

Miscellaneous:
 * We get some extra information on nixpkgs for free:
   the output of `./run.oil discover-systemd-services` shows two interesting metrics
    * For each (detectable) systemd service, the module it is defined in
    * For each (detectable) systed service, the list of the tests that activate it
       * This shows "how well" a service is tested (or "how fundamental" it is:
         for instance, `nginx` gets tested *a lot* !)
       * This also show the services that are *never* tested
       * Which hardening options are already configured for each service
 * There is a helper command `./run.oil find-malformed-tests` that lists all the tests
   that don't support our `systemdPassthru` hack. In the future, it would be great to also
   modify these tests so they work with this framework, but for now let's focus on running
   the tests that already work
 * There is another development helper, `dry-run-all-tests`, that checks wether all the tests
   actually evaluate. This helps finding some ticky parts of nixpkgs that are harder to
   hook with the `systemdPassthru` method.
 * Currently, this tool only targets boolean configuration options, not all of
   what `systemd-analyze` checks, e.g. not seccomp filters
   We target:
    - PrivateDevices
    - PrivateMounts
    - PrivateNetwork
    - PrivateTmp
    - PrivateUsers
    - ProtectControlGroups
    - ProtectKernelModules
    - ProtectKernelTunables
    - ProtectKernelLogs
    - ProtectClock
    - ProtectHostname
    - LockPersonality
    - MemoryDenyWriteExecute
    - NoNewPrivileges
    - RestrictRealtime
    - RestrictSUIDSGID

todo: use patch instead of cp to add systemdPassthru

