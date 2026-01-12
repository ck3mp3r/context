class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.4.1"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.4.1/context-0.4.1-aarch64-darwin.tgz"
      sha256 "32ea92ff757028f43a600d7af226b6ebbf9ef04d7377553b3660c932011b1a31"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.4.1/context-0.4.1-x86_64-darwin.tgz"
      sha256 "ae502d03fd6897eedf64fcd2c7d70fccea08fa1fd34adc0e2b8379c5483bb01f"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.4.1/context-0.4.1-x86_64-linux.tgz"
      sha256 "6ba8d3c022dcb44e412855e0869141842b690250b858bcf7ac8c5caa0070df5c"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.4.1/context-0.4.1-aarch64-linux.tgz"
      sha256 "72170cf75c55e5947539bac0b19eb4d1d22783b63961eb0d3485e2513586f585"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
