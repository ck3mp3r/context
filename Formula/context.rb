class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.7.8"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.8/context-0.7.8-aarch64-darwin.tgz"
      sha256 "7c08e2aee05532a1fa22c9116ea36c78e3675c39b09c12e12d23e9b355ed8262"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.8/context-0.7.8-x86_64-darwin.tgz"
      sha256 "993c9a85782428efeeb8d85b6893247c0b808d5dbec115176c93730e1bb4c9d1"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.8/context-0.7.8-x86_64-linux.tgz"
      sha256 "924dfd741d5bde9f8852c129641fe69e5441a1a208a7397d09c5569032ea31b2"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.8/context-0.7.8-aarch64-linux.tgz"
      sha256 "4d64bc78b4c3fb7d365b99ed0d355315322a960e4dedbe856bcf9a79f56e9e0b"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
