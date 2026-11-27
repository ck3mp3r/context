class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-pre.86"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.86/context-0.8.0-pre.86-aarch64-darwin.tgz"
      sha256 "1966a80eb262423ec8f0682877ed0389ccb23496d765c7a867149f2e1a1d3627"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.86/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.86/context-0.8.0-pre.86-x86_64-linux.tgz"
      sha256 "a2ff7e2385ca6a55369b1f3a51f848d4d051bb9fe520773a6815d0507389495a"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.86/context-0.8.0-pre.86-aarch64-linux.tgz"
      sha256 "1d6a08ff193d103412d21429fffe076835516888ba704b796918bbd148d6d24c"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
