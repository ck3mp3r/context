{
  lib,
  rustPlatform,
  fetchFromGitHub,
  pkg-config,
  openssl,
}: let
  pname = "trunk";
  version = "0.22.1";
in
  rustPlatform.buildRustPackage {
    inherit pname version;

    src = fetchFromGitHub {
      owner = "trunk-rs";
      repo = "trunk";
      rev = "v${version}";
      hash = "sha256-GXwFLOjmNZL2qNgFLG18pM+jXLvR4u80SUyjH9dsnJM=";
    };

    nativeBuildInputs = [pkg-config];

    buildInputs = [openssl];

    # requires network
    checkFlags = ["--skip=tools::tests::download_and_install_binaries"];

    cargoHash = "sha256-k4yQrT3gxjLPL5hI+1LtU0Nrd2a0ksGSAmI2I2AUZn0=";

    meta = {
      homepage = "https://github.com/trunk-rs/trunk";
      description = "Build, bundle & ship your Rust WASM application to the web";
      mainProgram = "trunk";
      maintainers = with lib.maintainers; [ctron];
      license = lib.licenses.asl20;
    };
  }
