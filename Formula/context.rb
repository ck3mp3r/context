class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.4.0"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.4.0/context-0.4.0-aarch64-darwin.tgz"
      sha256 "43ef5727c2386f08526fdfc46c4dea5310431a8f1226ca39191da03b3bd9338a"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.4.0/context-0.4.0-x86_64-darwin.tgz"
      sha256 "c6f598dc454e212c61077020bd3f046a6cc37834447832845691a88317eb25a2"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.4.0/context-0.4.0-x86_64-linux.tgz"
      sha256 "4116ffcb96647d9492a224ae1b2ecd54b3a67b40567859e8a2209aa15feaf806"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.4.0/context-0.4.0-aarch64-linux.tgz"
      sha256 "64f1e4f36ba1d1616d031c659e87570342df85db6b776cdc81e99c213ac17bf0"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
