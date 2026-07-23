# Classic Nix shell for CI - just the toolchains needed for backend testing
{
  pkgs,
  inputs,
  system,
}: let
  toolchain = inputs.rustnix.packages.${system}.toolchain;
in
  pkgs.mkShellNoCC {
    name = "context-ci";

    buildInputs = [
      toolchain
      pkgs.cargo-tarpaulin
    ];

    shellHook = ''
      echo "CI Testing Environment"
      echo "Rust: $(rustc --version)"
      echo ""
    '';
  }
