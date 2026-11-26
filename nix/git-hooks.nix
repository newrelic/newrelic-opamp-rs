# Git hooks: formatting, lint and cargo checks on commit. They are defined here and exposed as a
# flake check; the `hooks` dev shell installs them (see devshells/hooks.nix).
{ inputs, ... }:
{
  imports = [ inputs.git-hooks.flakeModule ];

  perSystem =
    { config, pkgs, ... }:
    let
      rustToolchain = config.rustLib.toolchain;
    in
    {
      pre-commit.settings = {
        package = pkgs.prek;
        hooks = {
          treefmt = {
            enable = true;
            package = config.treefmt.build.wrapper;
          };

          codespell = {
            enable = true;
            name = "codespell";
            entry = pkgs.lib.getExe pkgs.codespell;
            language = "system";
            pass_filenames = false;
            always_run = true;
          };

          actionlint.enable = true;
          deadnix.enable = true;
          statix.enable = true;
          check-merge-conflicts.enable = true;
          check-added-large-files.enable = true;
          detect-private-keys.enable = true;

          cargo-check = {
            enable = true;
            package = rustToolchain;
          };

          clippy = {
            enable = true;
            packageOverrides = {
              cargo = rustToolchain;
              clippy = rustToolchain;
            };
            settings = {
              denyWarnings = true;
              extraArgs = "--workspace --tests";
            };
          };
        };

        # git-hooks.nix keeps its whole config block under a `settings` option, hence the repeat.
        settings.rust.check.cargoDeps = pkgs.rustPlatform.importCargoLock {
          lockFile = ../Cargo.lock;
        };
      };
    };
}
