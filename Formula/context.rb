class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-81a5bdc"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-81a5bdc/context-0.8.0-81a5bdc-aarch64-darwin.tgz"
      sha256 "cc6530adff996d1e98e0b454618f785117c57bb5728ecea2dd330c94b70923b6"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-81a5bdc/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-81a5bdc/context-0.8.0-81a5bdc-x86_64-linux.tgz"
      sha256 "4d016590a5b9e2c6f0f17b2a1c41ddfbbcc60d0fa6fbde95d9b7c48e52cb7630"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-81a5bdc/context-0.8.0-81a5bdc-aarch64-linux.tgz"
      sha256 "d07e3510312cf40e1ad5b0d3ef5f63a6be813243b44bc8f14a32086b22a7ddfc"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
