# Classic Nix shell for CI - just the toolchains needed for testing
{
  pkgs,
  inputs,
  system,
}: let
  fenix = inputs.fenix.packages.${system};
in
  pkgs.mkShell {
    name = "context-ci";

    buildInputs = [
      # Rust toolchain (stable)
      fenix.stable.toolchain
    ];

    shellHook = ''
      echo "CI Testing Environment"
      echo "Rust: $(rustc --version)"
      echo ""
    '';
  }
