{
  pkgs,
  cargoToml,
  defaultPackage,
}:
# Container image - build on Linux systems only
# Cross-compilation from Darwin is deferred to CI
pkgs.dockerTools.buildLayeredImage {
  name = "context";
  tag = cargoToml.package.version;

  # Closure contents - no base image, just what we need
  contents = [
    defaultPackage # c5t binary with embedded frontend
    pkgs.cacert # CA certificates for HTTPS/git sync
    pkgs.dash # Lightweight shell for debugging
  ];

  # Setup /data directory before packaging
  extraCommands = ''
    mkdir -p data
    chmod 777 data
  '';

  config = {
    Cmd = ["/bin/c5t" "api" "--home" "/data"];
    ExposedPorts = {"3737/tcp" = {};};
    Env = [
      "PORT=3737"
      "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
      "TZ=UTC"
    ];
    WorkingDir = "/data";
  };
}
