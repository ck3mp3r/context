{
  description = "Context - Rust CLI and API";

  inputs = {
    nixpkgs.url = "github:NixOs/nixpkgs/nixpkgs-unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rustnix = {
      url = "github:ck3mp3r/flakes?dir=rustnix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
    };
  };

  outputs = inputs @ {
    self,
    flake-parts,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["aarch64-darwin" "aarch64-linux" "x86_64-darwin" "x86_64-linux"];
      perSystem = {system, ...}: let
        overlays = [
          inputs.fenix.overlays.default
          (final: prev: {
            wasm-bindgen-cli = prev.callPackage ./nix/wasm-bindgen-cli.nix {};
          })
        ];
        pkgs = import inputs.nixpkgs {inherit system overlays;};

        cargoToml = fromTOML (builtins.readFile ./Cargo.toml);
        cargoLock = {
          lockFile = ./Cargo.lock;
          outputHashes = {
            "tree-sitter-nu-0.0.1" = "sha256-G+XuQSqvJ9xRNq4fYiyHK9+AmCNofayPOC6JrFXpcjU=";
            "tree-sitter-kotlin-0.4.0" = "sha256-VGGPvg4RoRQ7WOpXxJQWatP0AAoiM4PhORZPZFED8ZY=";
            "tree-sitter-typescript-0.23.2" = "sha256-A0M6IBoY87ekSV4DfGHDU5zzFWdLjGqSyVr6VENgA+s=";
          };
        };

        # Import packaging logic
        packaging = import ./nix/packaging.nix {
          inherit
            inputs
            system
            pkgs
            cargoToml
            cargoLock
            overlays
            ;
        };
      in {
        inherit (packaging) apps packages;

        devShells = {
          default = import ./nix/dev.nix {
            inherit pkgs inputs system;
          };

          ci = import ./nix/ci.nix {
            inherit pkgs inputs system;
          };
        };

        formatter = pkgs.alejandra;
      };

      flake = {
        overlays.default = final: prev: {
          context = self.packages.default;
        };
      };
    };
}
