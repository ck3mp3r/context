class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-096fbfd"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-096fbfd/context-0.8.0-096fbfd-aarch64-darwin.tgz"
      sha256 "f9dfd4af0cf7ecbce9420ec2b4bdea38050c6e4816f316b4086173d056b8b34d"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-096fbfd/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-096fbfd/context-0.8.0-096fbfd-x86_64-linux.tgz"
      sha256 "4c83639727a9182af0e4f9036675f3809735824655152c4d7432d8158232762a"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-096fbfd/context-0.8.0-096fbfd-aarch64-linux.tgz"
      sha256 "e37357619c86bfd38b6e12c1261b6e7b45a7847993e7552bbe89fd591fce6e13"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
