class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-4610dfd"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-4610dfd/context-0.8.0-4610dfd-aarch64-darwin.tgz"
      sha256 "f98edf94db595259122936708dc0e11ee587ab5b0163f7cb8ef2d4cbbc024cb3"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-4610dfd/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-4610dfd/context-0.8.0-4610dfd-x86_64-linux.tgz"
      sha256 "0cf1b7d0a8f369b16efb78ff9dbc137c96a6bbebd49e84245bad467af4963d6b"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-4610dfd/context-0.8.0-4610dfd-aarch64-linux.tgz"
      sha256 "bff75230e83fe3427f51da365de2e8cbb1dbe7f9b228c110c5dceb13fcbc82a2"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
