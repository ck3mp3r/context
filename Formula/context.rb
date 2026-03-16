class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.7.1"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.1/context-0.7.1-aarch64-darwin.tgz"
      sha256 "3e308f7b2fc482974d0715ac055a43900b1a2a34176b13435f6d7d8996551222"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.1/context-0.7.1-x86_64-darwin.tgz"
      sha256 "ac4c4561429db78824aa4b73c21cb8a063523a2ec5bcfcb183754a544bf9d200"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.1/context-0.7.1-x86_64-linux.tgz"
      sha256 "3f0bad873d7e17cbb4d0a9060d51e82cea239a7d91662153370707315536f3ba"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.1/context-0.7.1-aarch64-linux.tgz"
      sha256 "229f8e84891426b57d563aada943785c3209ea79ba6f34ac543886a32aa05e74"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
