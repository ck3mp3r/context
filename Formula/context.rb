class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-8959cbd"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-8959cbd/context-0.8.0-8959cbd-aarch64-darwin.tgz"
      sha256 "b96baba0da6a1daa9baf46d0bd5c8bb39a55d18839b1e3b0190a590f3198b3bd"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-8959cbd/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-8959cbd/context-0.8.0-8959cbd-x86_64-linux.tgz"
      sha256 "21ca7f2e51c6e0c353a2242aa6d07e3260474976ba49c5a132282d7c6db92406"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-8959cbd/context-0.8.0-8959cbd-aarch64-linux.tgz"
      sha256 "06dedb6ab88fb526927e5c373c82d6a3a7325b91235a826f2c472ed6895963f1"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
