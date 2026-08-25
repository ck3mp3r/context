class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.7.9"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.9/context-0.7.9-aarch64-darwin.tgz"
      sha256 "e5f7e5a4ac5b206ff8dfc3e4da2c7f3ea349addd7126cb54db66193f35735c35"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.9/context-0.7.9-x86_64-darwin.tgz"
      sha256 "f040034794b58b60ad75ede947d037673fa7482ec854774ffa9181af3829f9a2"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.9/context-0.7.9-x86_64-linux.tgz"
      sha256 "3048d2469e00de89f05ff7d069fcd689e00500a04c60a13a38f031cca4d46b92"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.9/context-0.7.9-aarch64-linux.tgz"
      sha256 "4b8adb57b1389fda63d5d1682e3452b6dd019e8085853f8f2d3dccc6ebe38fa6"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
