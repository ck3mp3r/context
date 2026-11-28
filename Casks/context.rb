cask "context" do
  version "0.7.10"

  on_arm do
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"

    url "https://github.com/ck3mp3r/context/releases/download/v0.7.10/context-0.7.10-aarch64-darwin.dmg"
  end

  name "Context"
  desc "Task management and knowledge tracking system for AI-assisted workflows"
  homepage "https://github.com/ck3mp3r/context"

  depends_on :macos

  app "Context.app"
  binary "#{appdir}/Context.app/Contents/MacOS/c5t", target: "c5t"
end
