class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-pre.83"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.83/context-0.8.0-pre.83-aarch64-darwin.tgz"
      sha256 "0e1759245148c9fd89eb189cdcde9e2f912dce924f02850efefe896062aa5845"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.83/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.83/context-0.8.0-pre.83-x86_64-linux.tgz"
      sha256 "7bea982ac93cdd65b71c84f596cf7dde0523ed692d8504c96b8e2c17391cd54d"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.83/context-0.8.0-pre.83-aarch64-linux.tgz"
      sha256 "0cc8f1af5575645991123bcbf74fdd3b0dd01cc3fa3092d5a858fe581a7ccef5"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
