{pkgs}:
# Ultra-minimal git build for container sync operations only
# Strips out: perl, python, GUI tools, documentation, translations, optional protocols
#
# Based on nixpkgs git package.nix override options:
# https://github.com/NixOS/nixpkgs/blob/master/pkgs/by-name/gi/git/package.nix
pkgs.gitMinimal.override {
  # Disable all optional features to minimize closure size
  withManual = false; # No documentation (asciidoc, texinfo, xmlto, docbook, libxslt)
  pythonSupport = false; # No python3 dependency
  perlSupport = false; # No perl + 30+ perl modules
  guiSupport = false; # No tcl/tk for git-gui/gitk
  sendEmailSupport = false; # No git-send-email (requires perl SMTP libs)
  svnSupport = false; # No git-svn (requires perl + subversion)
  nlsSupport = false; # No translations/gettext

  # Keep minimal functionality for sync operations
  withpcre2 = false; # Drop pcre2 - grep patterns built-in (save ~5MB closure)

  # Additional notes:
  # gitMinimal is already optimized for minimal size
  # These features cannot be overridden further:
  # - openssl support is built-in
  # - libcurl support is built-in
  # - iconv support is built-in
  # Alpine-style build optimizations are already applied
}
