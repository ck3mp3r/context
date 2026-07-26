{
  description = "Context - Rust CLI and API";

  inputs = {
    base-nixpkgs.url = "github:ck3mp3r/flakes?dir=base-nixpkgs";
    nixpkgs.follows = "base-nixpkgs/unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    rustnix = {
      url = "github:ck3mp3r/flakes?dir=rustnix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.base-nixpkgs.follows = "base-nixpkgs";
      inputs.flake-parts.follows = "flake-parts";
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
          inputs.rustnix.lib.rust.overlays.fenix
        ];
        pkgs = import inputs.nixpkgs {inherit system overlays;};

        cargoToml = fromTOML (builtins.readFile ./Cargo.toml);
        cargoLock = {
          lockFile = ./Cargo.lock;
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
