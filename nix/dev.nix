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
      (pkgs.callPackage ./wasm-bindgen-cli.nix {})
      pkgs.tailwindcss_4
      pkgs.act
      pkgs.prek
    ];

    shellHook = ''
      prek install -t pre-commit -t pre-push
    '';
  }
