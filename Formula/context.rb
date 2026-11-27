class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-pre.84"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.84/context-0.8.0-pre.84-aarch64-darwin.tgz"
      sha256 "be46d9296289adf5501af31aa8865ef62a9fa804ab6a1bceb94b450fac0c7a80"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.84/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.84/context-0.8.0-pre.84-x86_64-linux.tgz"
      sha256 "0cf723ddc49eecac757dfb47ed1ade83740a47b814b7bc48a878f77c621ad30b"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.84/context-0.8.0-pre.84-aarch64-linux.tgz"
      sha256 "b231038305ab0e5860ad7cf3ac6edaa57c5722a69cd155900866939afc806a7a"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
