class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.7.6"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.6/context-0.7.6-aarch64-darwin.tgz"
      sha256 "56a1abb84a66b7fc15639e2d386699d9b39526451767d7f8c084032b988075f2"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.6/context-0.7.6-x86_64-darwin.tgz"
      sha256 "57fc6d72b060914e819bd3554b28cadeaa7ce211de09f5340808fd35875947fe"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.6/context-0.7.6-x86_64-linux.tgz"
      sha256 "7d21a98a33384a0fdeaeda31e641818a54612e4c71c2ae641843e532b6d23d84"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.6/context-0.7.6-aarch64-linux.tgz"
      sha256 "40517a13cf704428746b6eed922668191f12c65710993ce5004409b1498444c0"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
