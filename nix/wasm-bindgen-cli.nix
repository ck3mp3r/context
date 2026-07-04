{
  lib,
  rustPlatform,
  nodejs_latest,
  pkg-config,
  openssl,
  stdenv,
  curl,
}: let
  pname = "wasm-bindgen-cli";
  version = "0.2.122";
  src = rustPlatform.fetchCargoTarball {
    inherit pname version;
    sha256 = "sha256-vO4RSxi/sMWxmsEs3GuljdMfIRSu75A+Q+c5wgYToRU=";
  };
  cargoDeps = rustPlatform.fetchCargoVendor {
    inherit src;
    hash = "sha256-Inup6vvJSG5ghNyeDPyZbfZo4d0LsMG2OJfStoaeDBs=";
  };
in
  rustPlatform.buildRustPackage {
    inherit pname version src cargoDeps;

    nativeBuildInputs = [pkg-config];

    buildInputs =
      [openssl]
      ++ lib.optionals stdenv.hostPlatform.isDarwin [curl];

    nativeCheckInputs = [nodejs_latest];

    doCheck = false;

    meta = {
      homepage = "https://wasm-bindgen.github.io/wasm-bindgen/";
      license = with lib.licenses; [asl20 mit];
      description = "Facilitating high-level interactions between wasm modules and JavaScript";
      mainProgram = "wasm-bindgen";
    };
  }
