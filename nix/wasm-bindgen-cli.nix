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
  version = "0.2.129";
  src = fetchCrate {
    inherit pname version;
    hash = "sha256-pcecKQd7E8Opw6bkFoE569epUi7gh5qpQF1e5PJY6V8=";
  };
  cargoDeps = rustPlatform.fetchCargoVendor {
    inherit src;
    inherit (src) pname version;
    hash = "sha256-vmUrWVU7kPJJxO5qIVeAkwQyWDELO1Z4Z5gitz2kco8=";
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
