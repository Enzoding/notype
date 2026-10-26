cask "notype" do
  version "0.4.0"
  sha256 "e1a1143ebea529f2baab2ddcde437e5ebcacf307e6cc414df15d3b1ff232b283"

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
