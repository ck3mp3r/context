{
  lib,
  rustPlatform,
  fetchCrate,
  nodejs_latest,
  pkg-config,
  openssl,
  stdenv,
  curl,
}: let
  pname = "wasm-bindgen-cli";
  version = "0.2.127";
  src = fetchCrate {
    inherit pname version;
    hash = "sha256-di+qBAdd7pENLiIB9CoZoab+W5xeDoByMREcCGTSzWo=";
  };
  cargoDeps = rustPlatform.fetchCargoVendor {
    inherit src;
    inherit (src) pname version;
    hash = "sha256-FTv2GZIAQs0ePdIZXIXil7JbZ6kIT05VG6vqC1qNFxQ=";
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
