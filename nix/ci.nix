# CI shell for c5t - minimal tooling for builds
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
      # Rust toolchain (stable) with rustfmt and clippy
      toolchain
      # Test runner with JUnit output for CI
      pkgs.cargo-nextest
    ];

    shellHook = ''
      echo "CI Testing Environment"
      echo "Rust: $(rustc --version)"
      echo "cargo-nextest: $(cargo nextest --version)"
    '';
  }
