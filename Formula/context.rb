class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.4.3"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.4.3/context-0.4.3-aarch64-darwin.tgz"
      sha256 "e1df486cdba96ac7656d4b186acbdee7e9e729614f3e1cb2cf47382ceba0b13d"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.4.3/context-0.4.3-x86_64-darwin.tgz"
      sha256 "d5e82b0c51251bcfa868aff5d9dd7139d0b97e6a12f6992a748ab1c8d17d5017"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.4.3/context-0.4.3-x86_64-linux.tgz"
      sha256 "2617c0920e11c64b53dc5b497a0c718cfb9ff19b5b0db09c7a5b387d0bea270d"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.4.3/context-0.4.3-aarch64-linux.tgz"
      sha256 "6cd1b90e38ec1f491005e1a4ea4cdb2d850132617f1be5a9d021d91aeac21298"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
