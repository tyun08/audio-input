cask "audio-input" do
  version "0.2.0"
  sha256 "PLACEHOLDER_SHA256"

  url "https://github.com/tonyyun/audio-input/releases/download/v#{version}/Audio.Input_#{version}_aarch64.dmg"
  name "Audio Input"
  desc "AI-powered voice input for macOS — transcribe speech into any text field"
  homepage "https://github.com/tonyyun/audio-input"

  depends_on macos: ">= :ventura"

  app "Audio Input.app"

  zap trash: [
    "~/Library/Application Support/com.audioinput.app",
    "~/Library/Preferences/com.audioinput.app.plist",
    "~/Library/Logs/com.audioinput.app",
  ]
end
