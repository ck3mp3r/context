class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.5.1"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.1/context-0.5.1-aarch64-darwin.tgz"
      sha256 "25d16bcf18a8b695f6f4ec3bc8f99b959f6cbf5679b325a6e78539cc2b5fcc24"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.1/context-0.5.1-x86_64-darwin.tgz"
      sha256 "70643ae8f0b30f1f302753fa68e7807cdce7da6f32618b1623117f2c870b65d9"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.1/context-0.5.1-x86_64-linux.tgz"
      sha256 "b3e78cdf38bb39d18de5096f6d79466de093251aee262ac806d0d4b7bb4ce70c"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.1/context-0.5.1-aarch64-linux.tgz"
      sha256 "d2f75de43cc3d633f52df6b389335398d816f44ac8ec385ca04974d3e154481d"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
