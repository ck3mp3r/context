class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.5.2"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.2/context-0.5.2-aarch64-darwin.tgz"
      sha256 "8cde2d4c4a2fead887bc796b6b67fbf47eb1e1f99c395891062a944f0cd27d75"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.2/context-0.5.2-x86_64-darwin.tgz"
      sha256 "c24b08ed2d935cbb7dc4c276409887b111f2124c8bd2be6812c7d2e8b0345f8d"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.2/context-0.5.2-x86_64-linux.tgz"
      sha256 "89352d0b3a3d8ff91bce2d62eaaf44af253f2186bd38a9fb752105ab360777b2"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.2/context-0.5.2-aarch64-linux.tgz"
      sha256 "202ff9fc32b9d210cb15175731afe68f9fc5a6a10d89c686a9476716da94b2ab"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
