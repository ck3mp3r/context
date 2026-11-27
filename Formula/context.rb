class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-pre.87"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.87/context-0.8.0-pre.87-aarch64-darwin.tgz"
      sha256 "ce5a9f51bca53e6b246acea19ba3304a1c0129b67758b7d54d04cccbd6e69d87"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.87/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.87/context-0.8.0-pre.87-x86_64-linux.tgz"
      sha256 "5e79863ec4ae543c5cc58b81d8973868be1b4625669c0885ee8ff76e5472aec8"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.87/context-0.8.0-pre.87-aarch64-linux.tgz"
      sha256 "f22286886bef3704128f848ac01c44853e2d9ddcb2c773a67ce29fbb74bc0594"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
