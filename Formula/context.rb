class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.7.10"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.10/context-0.7.10-aarch64-darwin.tgz"
      sha256 "d07cf682ba72906ffe311992954b01a6a9ec2187c7d49873e28adfd795d98b88"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.10/context-0.7.10-x86_64-darwin.tgz"
      sha256 "ff1d9b8f0f5e0def47f26ada582ea2b296746570c5d103be806123b2328592a6"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.10/context-0.7.10-x86_64-linux.tgz"
      sha256 "2e3471985ee3d4b1edbb4e2a42dc276db0fc4daaaecb0517e24beebb85c369d1"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.10/context-0.7.10-aarch64-linux.tgz"
      sha256 "676d261bb3e41105c50899f4598c7bb8aa55ce3c99373eed85b9f30940c587f8"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
