# The steps of CI that `nix flake check` runs. treefmt and git-hooks add their own checks.
_: {
  perSystem =
    {
      pkgs,
      lib,
      craneLib,
      commonArgs,
      ...
    }:
    let
      # Tests rely on debug assertions, as in CI.
      args = commonArgs // {
        CARGO_PROFILE = "dev";
      };
      cargoArtifacts = craneLib.buildDepsOnly args;
    in
    {
      checks = {
        cargo-clippy = craneLib.cargoClippy (
          args
          // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- -D warnings -D clippy::all";
          }
        );

        cargo-nextest = craneLib.cargoNextest (
          args
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

        cargo-doc-tests = craneLib.cargoDocTest (args // { inherit cargoArtifacts; });
      };
    };
}
