class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-pre.82"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.82/context-0.8.0-pre.82-aarch64-darwin.tgz"
      sha256 "73be9a2f8928007b45fa91c328c9eec5b137181857b0f53acc38836522704fca"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.82/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.82/context-0.8.0-pre.82-x86_64-linux.tgz"
      sha256 "3f8d3be9327d5a40987043cd1ad2639a47d74e5038b6034465835eab2bdebe94"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.82/context-0.8.0-pre.82-aarch64-linux.tgz"
      sha256 "6bd45508c343cd44db6990e0a760915bcace1c7dd345e8c8fafb6775e09df767"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
