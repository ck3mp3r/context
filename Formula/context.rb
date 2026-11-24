class Context < Formula
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"
  version "0.8.0-a478046"
  license "GPL-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-a478046/context-0.8.0-a478046-aarch64-darwin.tgz"
      sha256 "70dd4052d7f91962c207e0ee2d2e499aaf3975a04abb3ddef8f139659ac69b93"
    else
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-a478046/context-0.7.10-x86_64-darwin.tgz"
      sha256 "91bf09cbcf4109ed632c5a70d26de63ad82903c45b582be39a43c2db5bcab399"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-a478046/context-0.8.0-a478046-x86_64-linux.tgz"
      sha256 "deb91a15955a89905c9fbc773790eef42a753df96f092ac8c1711636819ef316"
    elsif Hardware::CPU.arm?
      url "https://github.com/ck3mp3r/context/releases/download/v0.8.0-a478046/context-0.8.0-a478046-aarch64-linux.tgz"
      sha256 "a925031075c4a1a1770d7b3f4a3b265505ddada05e80f6fd0f9a5ce5bfab103a"
    end
  end

  def install
    bin.install "c5t"
  end

  test do
    system "#{bin}/c5t", "--version"
  end
end
