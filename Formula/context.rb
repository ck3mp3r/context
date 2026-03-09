class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.6.1"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.6.1/context-0.6.1-aarch64-darwin.tgz"
      sha256 "099cd2fcfc8594cd41a34bccccca7b1b444448af2263325feacf13fb7afa65d8"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.6.1/context-0.6.1-x86_64-darwin.tgz"
      sha256 "021fa0939c5ff31fd3a8e97a9bc0f9d2080d0f6c532622bfdaff811285bb8bba"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.6.1/context-0.6.1-x86_64-linux.tgz"
      sha256 "1523852e30e91a0b1c684551d3ef3841bf80ac1e4ce9ebe5de469f430d5b9b7f"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.6.1/context-0.6.1-aarch64-linux.tgz"
      sha256 "4197c07b19de5617624b4f3db7c613bfdba2027070682bc732dbbbefea195231"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
