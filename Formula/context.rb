class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.7.10"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.10/context-0.7.10-aarch64-darwin.tgz"
      sha256 "25cf6a76b9c488a1b29f0115489c4e06bf127da5cfc597a848535086f33e043d"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.10/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.10/context-0.7.10-x86_64-linux.tgz"
      sha256 "7d485f71f297a6e0d3aeb67087870ba4aa85df1ef1d9fe140105657cd186b3ff"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.10/context-0.7.10-aarch64-linux.tgz"
      sha256 "7fb0539ebd84b2d0065a370388b40f709c5ffc66ab8892e2c1437d7329b095d9"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
