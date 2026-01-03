# Classic Nix shell for CI - just the toolchains needed for testing
{
  pkgs,
  inputs,
  system,
}: let
  fenix = inputs.fenix.packages.${system};
  # Same toolchain as devenv.nix - Rust with WASM target support
  toolchain = fenix.combine [
    fenix.stable.cargo
    fenix.stable.rustc
    fenix.targets.wasm32-unknown-unknown.stable.rust-std
  ];
in
  pkgs.mkShell {
    name = "context-ci";

    buildInputs = [
      toolchain
      pkgs.cargo-tarpaulin
      pkgs.trunk
      pkgs.wasm-bindgen-cli
      pkgs.nodejs
      pkgs.nodePackages.tailwindcss
    ];

    shellHook = ''
      echo "CI Testing Environment"
      echo "Rust: $(rustc --version)"
      echo ""
    '';
  }
