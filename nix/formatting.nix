# `nix fmt`: one formatter entry point, using the same rustfmt as the pinned toolchain.
{ inputs, ... }:
{
  imports = [ inputs.treefmt-nix.flakeModule ];

  perSystem =
    { rustToolchain, ... }:
    {
      treefmt = {
        projectRootFile = "flake.nix";
        programs = {
          nixfmt.enable = true;
          rustfmt = {
            enable = true;
            package = rustToolchain;
          };
        };
      };
    };
}
