# Cargo steps of CI that `nix flake check` runs: rustfmt, clippy and the tests.
# treefmt and git-hooks contribute their own checks from their modules.
_: {
  perSystem =
    { config, ... }:
    let
      inherit (config) rustLib;
      inherit (rustLib) craneLib;
      workspaceArgs = rustLib.commonArgs // {
        cargoExtraArgs = "--workspace --all-features";
        # Tests rely on debug assertions, as in CI.
        CARGO_PROFILE = "dev";
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
            # These two assert on poll counts over real sleeps and fail in the macOS build sandbox, while
            # passing in the dev shell.
            cargoNextestExtraArgs = "--all-targets --no-fail-fast --retries 2 -E 'not test(=http::managed_client::tests::test_next_message_backoff) and not test(=http::managed_client::tests::test_poll_status_report)'";
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
