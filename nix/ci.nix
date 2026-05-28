# Classic Nix shell for CI - just the toolchains needed for backend testing
{
  pkgs,
  inputs,
  system,
}: let
  toolchain = inputs.rustnix.lib.rust.mkToolchain {
    inherit system;
  };
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
