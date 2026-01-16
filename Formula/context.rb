class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.4.2"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.4.2/context-0.4.2-aarch64-darwin.tgz"
      sha256 "3bbc9b02a54e0b00b735ae3f9e6e2f409bd72147194465154ee88f28bf3f74f9"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.4.2/context-0.4.2-x86_64-darwin.tgz"
      sha256 "460991f3e55522fa9da6689ce38b099c6ab711ed63a7d5824b54a604ba69de55"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.4.2/context-0.4.2-x86_64-linux.tgz"
      sha256 "5242eebfb1dc4dcebcdafacd3dce927784bde9435449a15522ebe4161d107cbe"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.4.2/context-0.4.2-aarch64-linux.tgz"
      sha256 "fadd69c2ec5090b54302ec1aad543f550fafd3dff316fc9def743be28fd06180"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
