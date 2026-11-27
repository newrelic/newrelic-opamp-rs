# The Rust toolchain at the `rust-version` of Cargo.toml, shared by the other modules as module
# arguments: `rustToolchain`, `craneLib` and `commonArgs`.
{ inputs, ... }:
{
  perSystem =
    { system, lib, ... }:
    let
      pkgs = import inputs.nixpkgs {
        inherit system;
        overlays = [ inputs.rust-overlay.overlays.default ];
      };
      workspace = fromTOML (builtins.readFile ../Cargo.toml);
      client = fromTOML (builtins.readFile ../opamp-client/Cargo.toml);

      rustToolchain = pkgs.rust-bin.stable.${workspace.workspace.package.rust-version}.default.override {
        extensions = [
          "rust-src"
          "rust-analyzer"
          "llvm-tools"
        ];
      };
      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain (_: rustToolchain);
    in
    {
      _module.args = {
        inherit pkgs rustToolchain craneLib;
        commonArgs = {
          src = craneLib.cleanCargoSource ../.;
          strictDeps = true;
          pname = "opamp-client";
          inherit (client.package) version;
          buildInputs = lib.optional pkgs.stdenv.hostPlatform.isDarwin pkgs.libiconv;
          cargoExtraArgs = "--workspace --all-features";
          # The HTTP client loads the system CA roots when built, and the Linux sandbox has none.
          SSL_CERT_FILE = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
        };
      };
    };
}
