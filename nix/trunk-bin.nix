{
  lib,
  stdenv,
  fetchurl,
}: let
  pname = "trunk";
  version = "0.22.1";

  sources = {
    aarch64-darwin = {
      triple = "aarch64-apple-darwin";
      sha256 = "18e381936c848700fefb9ee3f64f362419fab700200eefee323ebc0d00a8e9db";
    };
    aarch64-linux = {
      triple = "aarch64-unknown-linux-musl";
      sha256 = "8c84a939aaf48bbdac2d78195c932adb319e610fb2cbf791bae07b2660cf3e46";
    };
    x86_64-linux = {
      triple = "x86_64-unknown-linux-musl";
      sha256 = "a1fe5cd6b053f5a76f66b598d296022a7f06f56def6013cc16cd6897d53f5363";
    };
  };

  source =
    sources.${stdenv.hostPlatform.system}
    or (throw "trunk-bin: unsupported system ${stdenv.hostPlatform.system}");
in
  stdenv.mkDerivation {
    inherit pname version;

    src = fetchurl {
      url = "https://github.com/trunk-rs/trunk/releases/download/v${version}/trunk-${source.triple}.tar.gz";
      inherit (source) sha256;
    };

    sourceRoot = ".";

    installPhase = ''
      install -Dm755 trunk $out/bin/trunk
    '';

    meta = {
      homepage = "https://github.com/trunk-rs/trunk";
      description = "Build, bundle & ship your Rust WASM application to the web";
      mainProgram = "trunk";
      maintainers = with lib.maintainers; [ctron];
      license = lib.licenses.asl20;
    };
  }
