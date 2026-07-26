class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.7.5"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.5/context-0.7.5-aarch64-darwin.tgz"
      sha256 "bab7676a2658f15cd874ff220c1072c1dc7f3916b62280e98f53a2cea42efbef"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.5/context-0.7.5-x86_64-darwin.tgz"
      sha256 "3caedc78447bfe3e629f6ea2d7cc5c9d0321ab5dd07ba10ff217bf0bceae3f09"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.5/context-0.7.5-x86_64-linux.tgz"
      sha256 "5aaf16c08411e2ce01ef8697d0216f6cd3c3c80cc51d0982496b7aa1dcc8de25"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.5/context-0.7.5-aarch64-linux.tgz"
      sha256 "b453ba0d153287b48678f552ef3d23f8f9e781c97b03f889f5485afcc184499a"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
