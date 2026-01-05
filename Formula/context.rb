class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.2.1"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.2.1/context-0.2.1-aarch64-darwin.tgz"
      sha256 "cd9fea7fa9c71b78901d061074833fadd20f38471ea623d82770627cf57f8488"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.2.1/context-0.2.1-x86_64-darwin.tgz"
      sha256 "PLACEHOLDER_HASH_WILL_BE_GENERATED_DURING_RELEASE"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.2.1/context-0.2.1-x86_64-linux.tgz"
      sha256 "f35c0bf8aee6d10f11e6985edd0ceaba0ad7e98656d043344c6e74c1caf1dfdb"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.2.1/context-0.2.1-aarch64-linux.tgz"
      sha256 "678614ce7e1bcb0cf63f00cbc37a11e9a5f7b902fcef8f64d9c9795025dc02b1"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
