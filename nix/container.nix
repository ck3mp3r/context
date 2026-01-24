{
  pkgs,
  cargoToml,
  defaultPackage,
  git,
}:
# Container image - build on Linux systems only
# Cross-compilation from Darwin is deferred to CI
pkgs.dockerTools.buildLayeredImage {
  name = "context";
  tag = cargoToml.package.version;

  # Closure contents - no base image, just what we need
  contents = [
    defaultPackage # c5t binary with embedded frontend (statically linked)
    pkgs.cacert # CA certificates for HTTPS
    git # Ultra-minimal git for sync operations
  ];

  # Setup /data directory and create non-root user
  extraCommands = ''
    mkdir -p data
    chmod 777 data  # World-writable so c5t user can write when volume is mounted

    # Create /etc for passwd/group files
    mkdir -p etc

    # Create c5t user (UID 1000) and group (GID 1000)
    echo "c5t:x:1000:1000:c5t user:/data:/bin/noshell" > etc/passwd
    echo "c5t:x:1000:" > etc/group

    # Note: Ownership is handled by Docker runtime when User is set
    # The /data directory will be writable via the mounted volume
  '';

  config = {
    Cmd = ["/bin/c5t" "api" "--home" "/data"];
    User = "c5t";
    ExposedPorts = {"3737/tcp" = {};};
    Env = [
      "PORT=3737"
      "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
      "TZ=UTC"
    ];
    WorkingDir = "/data";
  };
}
