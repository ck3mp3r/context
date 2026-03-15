class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.7.0"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.0/context-0.7.0-aarch64-darwin.tgz"
      sha256 "5faab46fda492f65f3a870475c05347c89c10252a45bac2376198242220b7b40"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.0/context-0.7.0-x86_64-darwin.tgz"
      sha256 "21dddcc144467e8540777dbfd0f210a5c84a9900f05b9113c5f8f23906525575"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.0/context-0.7.0-x86_64-linux.tgz"
      sha256 "b6dfb7cc1811947681b8ba84f0b9f8571f5a731700034ac1c9d6bd7df94dc6a3"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.0/context-0.7.0-aarch64-linux.tgz"
      sha256 "7466ef3280270613bf7319a48c9d8a5b4aba281252f1885cce39ede145bab8ef"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
