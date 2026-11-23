class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-d378ab4"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-d378ab4/context-0.8.0-d378ab4-aarch64-darwin.tgz"
      sha256 "ad946825eaba58a50cf11c4c4a7c1cdaae5b28bc51f71cd4c8d4157bbb01db13"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-d378ab4/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-d378ab4/context-0.8.0-d378ab4-x86_64-linux.tgz"
      sha256 "12e9b297b8429a4c5f0e4ec8654f3b4333c96c43157d3f32d9185e2dbdb1f2b0"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-d378ab4/context-0.8.0-d378ab4-aarch64-linux.tgz"
      sha256 "c677930ff9cbdcef93f713f9061772413d6baff31ee4c31a4ccb52f708317921"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
