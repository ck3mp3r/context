class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-340d170"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-340d170/context-0.8.0-340d170-aarch64-darwin.tgz"
      sha256 "607d121dde0f4cc19b17e8c0b35af4dc86107c588a402bb7f46cc0bb1b04e55b"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-340d170/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-340d170/context-0.8.0-340d170-x86_64-linux.tgz"
      sha256 "5ed0c419c8dce528c1d0997bfefabcbd494d80d87e7b12424f47dbd8c818097a"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-340d170/context-0.8.0-340d170-aarch64-linux.tgz"
      sha256 "66542ba94630f18ada92b60fc9ff51e7b2f0e99b300fc2a7cf1cf4111bcda817"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
