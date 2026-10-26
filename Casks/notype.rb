cask "notype" do
  version "0.4.0"
  sha256 "98d9e53c75e9278a8b464c89c75d58bd69d2448987ea0d0584bd625b558ac8aa"

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
