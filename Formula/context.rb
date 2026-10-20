class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0/context-0.8.0-aarch64-darwin.tgz"
      sha256 "77f9fa2ca8c8fdf87e9eabf42a51835f308fbf08864e364e8f348625c0a1d46d"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0/context-0.8.0-x86_64-darwin.tgz"
      sha256 "720ed9508d5bd9a8b6985e11ad1e1bbd336a44050ce8dee768027462286a0de2"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0/context-0.8.0-x86_64-linux.tgz"
      sha256 "c79410a7c4b0fb5112a81301eafaa7ab71bb2c561cac7faf7789d3f1cd6e0939"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0/context-0.8.0-aarch64-linux.tgz"
      sha256 "aca283c9657ce8a8068a5cd7f7a405ebbb362653a45b27b4c9915b67829a3f4c"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
