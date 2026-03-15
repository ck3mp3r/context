class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.7.0"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.0/context-0.7.0-aarch64-darwin.tgz"
      sha256 "6a66836a68b71c877987f978948bf2dabbd8ed7b3ed31b7c951f4700b67f3cec"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.0/context-0.7.0-x86_64-darwin.tgz"
      sha256 "51e740f20ebad2dfd4a8793e21cee2974961f5a7e0c181207e6be2247a9d6f43"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.0/context-0.7.0-x86_64-linux.tgz"
      sha256 "634aef6adde51b07869efa80248602725b84fbf0bf595fada79c17a0b26ed84e"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.0/context-0.7.0-aarch64-linux.tgz"
      sha256 "bb43899be56e885f514c8b55a4f912df2b940699b5e467ed953262b769ae5f11"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
