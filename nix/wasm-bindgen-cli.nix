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
  version = "0.2.115";
  src = fetchCrate {
    inherit pname version;
    sha256 = "sha256-wRynyZKYEMoIhX64n4DkGG2iepU6rE5qdBjT1LkUgtE=";
  };
  cargoDeps = rustPlatform.fetchCargoVendor {
    inherit src;
    hash = "sha256-+7hgX56dOo/GErpf/unRprv72Kkars5dOFew+NfZZMY=";
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
