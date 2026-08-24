class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.7.8-b118f3a"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.8-b118f3a/context-0.7.8-b118f3a-aarch64-darwin.tgz"
      sha256 "00677351307cd24d4311bd25e018a59cd90fa6d526c09b45da393fd42e2d700f"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.8-b118f3a/context-0.7.8-b118f3a-x86_64-darwin.tgz"
      sha256 "c33ce3b4e81e6b336d54c310fe72bda303069bb740d28ed0520317d7dde627dd"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.8-b118f3a/context-0.7.8-b118f3a-x86_64-linux.tgz"
      sha256 "b7b761bc98ec63b0bb872bdc2268eb828796b43bb3779ad0cea79a8a7cfae240"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.7.8-b118f3a/context-0.7.8-b118f3a-aarch64-linux.tgz"
      sha256 "46af2c078c4ae8590e00b85ea9c61ac7407422131682dcd6ac61e21334ad5d8a"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
