# Cargo steps of CI that `nix flake check` runs: rustfmt, clippy and the tests.
# treefmt and git-hooks contribute their own checks from their modules.
_: {
  perSystem =
    {
      config,
      pkgs,
      lib,
      ...
    }:
    let
      inherit (config) rustLib;
      inherit (rustLib) craneLib;
      workspaceArgs = rustLib.commonArgs // {
        cargoExtraArgs = "--workspace --all-features";
        # Tests rely on debug assertions, as in CI.
        CARGO_PROFILE = "dev";
        # The HTTP client loads the system CA roots when built, and the sandbox has none on Linux.
        SSL_CERT_FILE = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
      };
      cargoArtifacts = craneLib.buildDepsOnly workspaceArgs;
    in
    {
      checks = {
        cargo-clippy = craneLib.cargoClippy (
          workspaceArgs
          // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- -D warnings -D clippy::all";
          }
        );

        cargo-nextest = craneLib.cargoNextest (
          workspaceArgs
          // {
            inherit cargoArtifacts;
            cargoNextestExtraArgs = lib.concatStringsSep " " (
              [
                "--all-targets"
                "--no-fail-fast"
                "--retries 2"
              ]
              # These two count polls over real timers, which tick late in the macOS build sandbox
              # (9 polls instead of 11 in 1.2s) while passing in the dev shell.
              ++ lib.optional pkgs.stdenv.hostPlatform.isDarwin "-E 'not test(=http::managed_client::tests::test_next_message_backoff) and not test(=http::managed_client::tests::test_poll_status_report)'"
            );
          }
        );

        cargo-doc-tests = craneLib.cargoDocTest (workspaceArgs // { inherit cargoArtifacts; });

        cargo-fmt = craneLib.cargoFmt {
          inherit (rustLib) src;
          inherit (rustLib.commonArgs) pname version;
        };
      };
    };
}
