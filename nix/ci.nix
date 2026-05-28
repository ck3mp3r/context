# Classic Nix shell for CI - just the toolchains needed for testing
{
  pkgs,
  inputs,
  system,
}: let
  toolchain = inputs.rustnix.lib.rust.mkToolchain {
    inherit system;
    targets = ["wasm32-unknown-unknown"];
  };
in
  pkgs.mkShellNoCC {
    name = "context-ci";

    buildInputs = [
      toolchain
      pkgs.cargo-tarpaulin
      pkgs.trunk
      pkgs.wasm-bindgen-cli
      pkgs.tailwindcss_4
      pkgs.protobuf # Required for NanoGraph (Lance dependency)
    ];

    shellHook = ''
      echo "CI Testing Environment"
      echo "Rust: $(rustc --version)"
      echo ""
    '';
  }
