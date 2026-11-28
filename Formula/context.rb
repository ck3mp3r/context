class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-pre.90"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.90/context-0.8.0-pre.90-aarch64-darwin.tgz"
      sha256 "7d982dad5160ef7aa7f9de143aa16638470ada6452a92ad53e5016082fe0d152"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.90/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.90/context-0.8.0-pre.90-x86_64-linux.tgz"
      sha256 "a9d3ccb074d771d20165554495e88e1922519b97ace7b793e27e07118e445936"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.90/context-0.8.0-pre.90-aarch64-linux.tgz"
      sha256 "8891c2a979ddfa90445da27bd014e651e692f03610fc32174c07fb9d7a7dbe69"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
