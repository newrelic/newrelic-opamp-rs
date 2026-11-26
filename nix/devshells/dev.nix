# Development shell with the pinned toolchain and the tools the Makefiles and CI use, including
# protoc, which `make -C proto` needs to regenerate the Rust bindings of the OpAMP spec.
_: {
  perSystem =
    {
      config,
      pkgs,
      ...
    }:
    {
      devShells.dev = config.rustLib.craneLib.devShell {
        # git-hooks.nix defines the hooks and hooks.nix installs them, not this shell: `.git/hooks`
        # is shared by all worktrees, so installing here would change the commits of worktrees
        # that are not in the shell.
        packages = [
          config.pre-commit.settings.package
          config.treefmt.build.wrapper
          pkgs.actionlint
          pkgs.cargo-deny
          pkgs.cargo-llvm-cov
          pkgs.cargo-nextest
          pkgs.git
          pkgs.gnumake
          pkgs.protobuf
        ]
        ++ config.pre-commit.settings.enabledPackages;
      };
    };
}
