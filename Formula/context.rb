class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.6.0"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.6.0/context-0.6.0-aarch64-darwin.tgz"
      sha256 "8c940b7ad9613b5508b994ab3d523b78533b305fe7dfe0a439476ac361aac1a3"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.6.0/context-0.6.0-x86_64-darwin.tgz"
      sha256 "33c952a6ed0a9ed44855ec5a2014e66acaef1477346ea8ceebdb2de13f2fc5b4"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.6.0/context-0.6.0-x86_64-linux.tgz"
      sha256 "64c9b591862f67a5bcdd39d0577f4eda76ead13331738c6affee496e56ea6239"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.6.0/context-0.6.0-aarch64-linux.tgz"
      sha256 "4b74b02192b12c33055cb4fd1fcde424f53bb3d03105f6490be3155626c9daa0"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
