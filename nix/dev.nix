{
  pkgs,
  inputs,
  system,
}: let
  fenix = inputs.fenix.packages.${system};
  toolchain = fenix.combine [
    fenix.stable.cargo
    fenix.stable.rustc
    fenix.stable.rustfmt
    fenix.stable.clippy
    fenix.stable.rust-analyzer
    fenix.stable.llvm-tools-preview
    fenix.targets.wasm32-unknown-unknown.stable.rust-std
  ];
in
  pkgs.mkShellNoCC {
    name = "context-dev";

    buildInputs = [
      toolchain
      pkgs.cargo-tarpaulin
      pkgs.cargo-llvm-cov
      pkgs.trunk
      pkgs.wasm-bindgen-cli
      pkgs.nodejs
      pkgs.tailwindcss_4
      pkgs.act
      pkgs.protobuf
      pkgs.lefthook
    ];

    shellHook = ''
      lefthook install
    '';
  }
