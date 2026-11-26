# Entry point of the flake modules: one directory per kind of flake output.
{
  imports = [
    ./checks
    ./devshells
    ./rust-lib
    ./formatting.nix
    ./git-hooks.nix
  ];
}
