# Values of the `rustLib` options (see options.nix): the Rust toolchain, crane and the sources and
# arguments of the builds.
{ inputs, ... }:
{
  perSystem =
    { pkgs, lib, ... }:
    let
      workspaceToml = fromTOML (builtins.readFile ../../Cargo.toml);
      clientToml = fromTOML (builtins.readFile ../../opamp-client/Cargo.toml);
      rustVersion = workspaceToml.workspace.package.rust-version;

      toolchain = pkgs.rust-bin.stable.${rustVersion}.default.override {
        extensions = [
          "rust-src"
          "rust-analyzer"
          "llvm-tools"
        ];
      };

      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain (_: toolchain);

      src = craneLib.cleanCargoSource ../../.;

      commonArgs = {
        inherit src;
        strictDeps = true;
        pname = "opamp-client";
        inherit (clientToml.package) version;
        buildInputs = lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.libiconv ];
      };
    in
    {
      config.rustLib = {
        inherit
          rustVersion
          toolchain
          craneLib
          src
          commonArgs
          ;
      };
    };
}
