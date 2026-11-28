class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-pre.89"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.89/context-0.8.0-pre.89-aarch64-darwin.tgz"
      sha256 "e6e13b664479b1b09299a2ef6fbb9898f5c66523462e4d19c4ab5927a4488760"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.89/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.89/context-0.8.0-pre.89-x86_64-linux.tgz"
      sha256 "5fdaa8195b3cfbbcdf5e0a9f7f41fa89bf1f8464a144186d62dc6c6c8f580ee4"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.89/context-0.8.0-pre.89-aarch64-linux.tgz"
      sha256 "473510974bd74eedf4627d7f55d403fb3bd76eba88862a8ec4af4e0a1611a97a"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
