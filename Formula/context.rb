class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.5.0"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.0/context-0.5.0-aarch64-darwin.tgz"
      sha256 "805b4c48077eada89b5082957cd61063df3bf757b2e9a294facca5f702eaba2d"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.0/context-0.5.0-x86_64-darwin.tgz"
      sha256 "8b8018314eaf3dd35b00fb66f329253f568dd6061cf158a85fd476395cbd8861"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.0/context-0.5.0-x86_64-linux.tgz"
      sha256 "19dd7c3818c7d3f4b3f5db09cbc656cae0f98a3718589b81569e67785b94e489"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.0/context-0.5.0-aarch64-linux.tgz"
      sha256 "4faa7b0dff88135ef3552c47adcb65d72342c762bcd806c3d289bd1f16470308"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
