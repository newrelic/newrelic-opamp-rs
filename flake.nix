{
  description = "OpAMP client for Rust: dev environment and checks";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    crane.url = "github:ipetkov/crane";

    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # The OpAMP spec at the commit the `proto/opamp-spec` submodule points to. The proto-bindings
    # check regenerates the Rust bindings from it; keep both in sync when updating the spec.
    opamp-spec = {
      url = "github:open-telemetry/opamp-spec/f1f80aeb72df7e97e0bdc01809436a4d36ea5c73";
      flake = false;
    };

    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      imports = [
        ./nix/rust.nix
        ./nix/devshell.nix
        ./nix/formatting.nix
        ./nix/git-hooks.nix
        ./nix/checks.nix
        ./nix/proto-bindings.nix
      ];
    };
}
