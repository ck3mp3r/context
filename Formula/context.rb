class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.5.3"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.3/context-0.5.3-aarch64-darwin.tgz"
      sha256 "54c45534c61550ec228dd0f0039cfe7c17bd5e19d79c621b4741bf1970cd3615"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.3/context-0.5.3-x86_64-darwin.tgz"
      sha256 "b96f8e1dd5a0343d04c544f9bd62b74cdc9eb5a6a8a727ab3c43c5bd0ff33449"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.3/context-0.5.3-x86_64-linux.tgz"
      sha256 "7eff1405e9630d91def78f362a82b5bda6ec3fbad28c5e900dd657ecde1a37b7"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.5.3/context-0.5.3-aarch64-linux.tgz"
      sha256 "8b7f966e46d2884c1d6e9fcc59cf0dca0af43a385ac3153e7255ea9d81ba1150"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
