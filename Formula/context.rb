class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.5.4"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.4/context-0.5.4-aarch64-darwin.tgz"
      sha256 "74d83897ef3fc96087738bf9a16d04013b672d572d9a32b3d85b19b4c4c9fa44"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.4/context-0.5.4-x86_64-darwin.tgz"
      sha256 "e037968b2aa3074d0c91f746c6f3220b61b2335ee2bcdd1faf60a764d347db1a"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.4/context-0.5.4-x86_64-linux.tgz"
      sha256 "8392333323382163219208295ee09e7da0b480858b6464d0170d57f1fcbb50d2"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.4/context-0.5.4-aarch64-linux.tgz"
      sha256 "ddc338c2cb143d160b46aa8dc727fd78da2e40016b5172d1b7024c236b17dafc"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
