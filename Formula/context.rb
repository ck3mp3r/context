class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-1ff3534"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-1ff3534/context-0.8.0-1ff3534-aarch64-darwin.tgz"
      sha256 "61b6858c66710243f79dd60c5c85f8b9b093b6e183551e3d8c52e495e3e6cce0"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-1ff3534/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-1ff3534/context-0.8.0-1ff3534-x86_64-linux.tgz"
      sha256 "66d53e5091a729f767fa189cd11c86d59b71041970064f28acb0ea876cfdd198"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-1ff3534/context-0.8.0-1ff3534-aarch64-linux.tgz"
      sha256 "1a04135e30de754e309a6db838ba13e4e6aa3e8822d2004e804854f00d5f9951"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
