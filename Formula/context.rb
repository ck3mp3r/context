class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-f74d988"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-f74d988/context-0.8.0-f74d988-aarch64-darwin.tgz"
      sha256 "af7e0d1dca74e0d8acf87bb39ebebdee7b30c3604c6e15c0a630879795ede926"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-f74d988/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-f74d988/context-0.8.0-f74d988-x86_64-linux.tgz"
      sha256 "7dde212740437d02b69c331efc4783093b678f4d3f7fcd7f4ae8684612067c68"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-f74d988/context-0.8.0-f74d988-aarch64-linux.tgz"
      sha256 "00f88d6f60ab37235d93bc74c642f65c79b94e7adb7f5a6b540af538b943905b"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
