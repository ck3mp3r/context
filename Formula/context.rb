class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.7.0"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.0/context-0.7.0-aarch64-darwin.tgz"
      sha256 "77a579beeb49ac9f14ca268c64e79fb711948f627b0d79e0d5079ac99aa96665"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.0/context-0.7.0-x86_64-darwin.tgz"
      sha256 "db38ad9297aa1d09651d2f4459ec742844e70770b1025ab57ce4b3bdc1cb6b6a"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.0/context-0.7.0-x86_64-linux.tgz"
      sha256 "9d8dcb0102aed1b241a787479ca6de6abefaadc2bca3c7a3a515c4c68d602a96"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.0/context-0.7.0-aarch64-linux.tgz"
      sha256 "5171ff6e206f479ee96803be452cc27008428dbad7582bddd8e74ea839ea5bde"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
