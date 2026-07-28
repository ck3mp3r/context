class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.7.7"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.7/context-0.7.7-aarch64-darwin.tgz"
      sha256 "3b25ba73e9f4b2bdf4a328982d38984537ccfda2c92250ce4ef7bfaa6903c34e"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.7/context-0.7.7-x86_64-darwin.tgz"
      sha256 "fd729c96c83c4dfd8af63ae6539e5fc0d899672a677d9138bf54688ad52c1a86"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.7/context-0.7.7-x86_64-linux.tgz"
      sha256 "afd029d50e317a4d81112b02eff2f51059d04d7b42aad5c40057091d58b771a4"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.7/context-0.7.7-aarch64-linux.tgz"
      sha256 "bd297899377bedcbb5ff4c15087a32fc1a6608413fff7f0651c02e54615d6839"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
