{
  pkgs,
  inputs,
  system,
}: let
  toolchain = inputs.rustnix.lib.rust.mkToolchain {
    inherit system;
    targets = ["wasm32-unknown-unknown"];
    extras = ["rustfmt" "clippy" "rust-analyzer" "llvm-tools-preview"];
  };
in
  pkgs.mkShellNoCC {
    name = "context-dev";

    buildInputs = [
      toolchain
      pkgs.cargo-tarpaulin
      pkgs.cargo-llvm-cov
      pkgs.trunk
      pkgs.wasm-bindgen-cli
      pkgs.tailwindcss_4
      pkgs.act
      pkgs.lefthook
      pkgs.tree-sitter
    ];

    shellHook = ''
      lefthook install
    '';
  }
