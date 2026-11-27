# Git hooks, installed on entering the dev shell and also run by `nix flake check`.
{ inputs, ... }:
{
  imports = [ inputs.git-hooks.flakeModule ];

  perSystem =
    {
      config,
      pkgs,
      rustToolchain,
      ...
    }:
    {
      pre-commit.settings = {
        hooks = {
          treefmt = {
            enable = true;
            package = config.treefmt.build.wrapper;
          };
          actionlint.enable = true;
          codespell = {
            enable = true;
            name = "codespell";
            entry = pkgs.lib.getExe pkgs.codespell;
            language = "system";
            pass_filenames = false;
            always_run = true;
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

        # The clippy hook runs offline in `nix flake check`.
        settings.rust.check.cargoDeps = pkgs.rustPlatform.importCargoLock { lockFile = ../Cargo.lock; };
      };
    };
}
