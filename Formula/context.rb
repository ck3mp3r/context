class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-pre.88"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.88/context-0.8.0-pre.88-aarch64-darwin.tgz"
      sha256 "4ec9e579bad7a27e0a25289254321b8f0d92d1a681dac51d59e6cc1d5a8d8379"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.88/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.88/context-0.8.0-pre.88-x86_64-linux.tgz"
      sha256 "e82a111f8748e36bbb8610d767b74a9e032451c47427e067657931daf2828af9"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.88/context-0.8.0-pre.88-aarch64-linux.tgz"
      sha256 "b4ab455acbcf4eac08573d8d4549cce3bf8af4daa486b606b4fbbb71e2fa4495"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
