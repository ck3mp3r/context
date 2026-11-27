class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-pre.85"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.85/context-0.8.0-pre.85-aarch64-darwin.tgz"
      sha256 "b24f7a9a58fcadba0d836b2ca145494f23bf255021eed34a1c51d53608050b7a"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.85/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.85/context-0.8.0-pre.85-x86_64-linux.tgz"
      sha256 "1fba0ad841f6b3febe6ba9631ab20cf18a81b94fb440bc5d2ccb1f80cbcc84fe"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-pre.85/context-0.8.0-pre.85-aarch64-linux.tgz"
      sha256 "e4e115916060b6b6217f2902f7e252c8a9a41980d1ca4ccbb1023b10380e299f"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
