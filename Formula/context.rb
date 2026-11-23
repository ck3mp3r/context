class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-d44892e"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-d44892e/context-0.8.0-d44892e-aarch64-darwin.tgz"
      sha256 "619d1a3cfb6994e94b9da7748c04b5e477bc8622cddef324acc146de4a69f421"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-d44892e/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-d44892e/context-0.8.0-d44892e-x86_64-linux.tgz"
      sha256 "0c5ab2d1009b2a3ff08e4ff640f781904548502c8f2c6e5704da1bc55ba8121d"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-d44892e/context-0.8.0-d44892e-aarch64-linux.tgz"
      sha256 "cfb91287ebd8665842282cbb2c9324cf2b70833eddb392b127a48a1d6448cec8"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
