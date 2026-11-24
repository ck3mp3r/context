class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-83f33f6"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-83f33f6/context-0.8.0-83f33f6-aarch64-darwin.tgz"
      sha256 "8b0e8c565739b1dcf28a7bad763ad35d89143006cd4657d383d01e94f03485ae"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-83f33f6/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-83f33f6/context-0.8.0-83f33f6-x86_64-linux.tgz"
      sha256 "11700fb4c8d2183ab4ca8d6f3db7e6d6501388eeee949717eb2cf960fe2a4cd0"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-83f33f6/context-0.8.0-83f33f6-aarch64-linux.tgz"
      sha256 "9c3307dc65ba92d622fb178dc5ccacde708657e8b07283e0bfab979240512199"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
