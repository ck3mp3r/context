{
  lib,
  stdenv,
  fetchurl,
}: let
  pname = "wasm-bindgen-cli";
  version = "0.2.129";

  sources = {
    aarch64-darwin = {
      triple = "aarch64-apple-darwin";
      sha256 = "81d4a23d56b3c3eb8187658329116d50e0b228a93b343825fb71f70179051cd1";
    };
    aarch64-linux = {
      triple = "aarch64-unknown-linux-musl";
      sha256 = "2ed4351c35dd9440308bbb02767d47ea278efe851a52465300f3c94f5b6c2a87";
    };
    x86_64-linux = {
      triple = "x86_64-unknown-linux-musl";
      sha256 = "82d12bb940e2d4e72e0d5605387fc1b8ca179044e012b620f0ce4e7440e8320e";
    };
  };

  source =
    sources.${stdenv.hostPlatform.system}
    or (throw "wasm-bindgen-cli-bin: unsupported system ${stdenv.hostPlatform.system}");

  tarballRoot = "wasm-bindgen-${version}-${source.triple}";
in
  stdenv.mkDerivation {
    inherit pname version;

    src = fetchurl {
      url = "https://github.com/rustwasm/wasm-bindgen/releases/download/${version}/wasm-bindgen-${version}-${source.triple}.tar.gz";
      inherit (source) sha256;
    };

    sourceRoot = ".";

    installPhase = ''
      install -Dm755 ${tarballRoot}/wasm-bindgen $out/bin/wasm-bindgen
    '';

    meta = {
      homepage = "https://wasm-bindgen.github.io/wasm-bindgen/";
      license = with lib.licenses; [asl20 mit];
      description = "Facilitating high-level interactions between wasm modules and JavaScript";
      mainProgram = "wasm-bindgen";
    };
  }
