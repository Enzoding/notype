cask "notype" do
  version "0.4.1"
  sha256 "b817e78e42f5b15e94dadb09d151d8ffa43d17be131b2e12c4f88aabfd3e1526"

  url "https://github.com/Enzoding/notype/releases/download/v#{version}/Notype-#{version}.dmg"
  name "Notype"
  desc "Local macOS dictation with AI polishing and transcription"
  homepage "https://github.com/Enzoding/notype"

  auto_updates true
  depends_on macos: ">= :ventura"

  app "Notype.app"

  postflight do
    system_command "xattr",
                   args: ["-cr", "#{appdir}/Notype.app"]
  end

  zap trash: [
    "~/Library/Application Support/Notype",
    "~/Library/Preferences/com.notype.app.plist",
  ]
end
