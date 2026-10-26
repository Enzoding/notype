#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${ROOT_DIR}"

APP_NAME="Notype"
BIN_NAME="notype"
BUNDLE_ID="com.notype.app"
VERSION="${VERSION:-$(grep -m1 '^version = ' "${ROOT_DIR}/Cargo.toml" | cut -d '"' -f 2)}"
RELEASE_DIR="${ROOT_DIR}/release"
APP_BUNDLE="${RELEASE_DIR}/${APP_NAME}.app"
DMG_NAME="${APP_NAME}-${VERSION}.dmg"
DMG_PATH="${RELEASE_DIR}/${DMG_NAME}"

echo "==> Building release binary..."
cargo build --release --locked

echo "==> Creating release directory..."
mkdir -p "${RELEASE_DIR}"
rm -rf "${APP_BUNDLE}"
mkdir -p "${APP_BUNDLE}/Contents/MacOS"
mkdir -p "${APP_BUNDLE}/Contents/Resources"

echo "==> Copying binary..."
cp "${ROOT_DIR}/target/release/${BIN_NAME}" "${APP_BUNDLE}/Contents/MacOS/${BIN_NAME}"
chmod +x "${APP_BUNDLE}/Contents/MacOS/${BIN_NAME}"

echo "==> Copying AppIcon..."
if [ -f "${ROOT_DIR}/assets/AppIcon.icns" ]; then
    cp "${ROOT_DIR}/assets/AppIcon.icns" "${APP_BUNDLE}/Contents/Resources/AppIcon.icns"
fi

echo "==> Generating Info.plist..."
cat <<EOF > "${APP_BUNDLE}/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleExecutable</key>
    <string>${BIN_NAME}</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleLocalizations</key>
    <array>
        <string>en</string>
        <string>zh-Hans</string>
    </array>
    <key>LSMinimumSystemVersion</key>
    <string>13.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
    <key>NSMicrophoneUsageDescription</key>
    <string>Notype needs microphone access to record audio for speech-to-text dictation.</string>
    <key>NSSpeechRecognitionUsageDescription</key>
    <string>Notype needs speech recognition permissions to process your voice dictation.</string>
    <key>NSAppleEventsUsageDescription</key>
    <string>Notype needs accessibility and control permissions to paste polished text into your active application.</string>
    <key>NSSystemAdministrationUsageDescription</key>
    <string>Notype needs accessibility permissions to monitor the global dictation hotkey and insert text.</string>
</dict>
</plist>
EOF

echo "==> Generating localized InfoPlist.strings..."
mkdir -p "${APP_BUNDLE}/Contents/Resources/en.lproj"
mkdir -p "${APP_BUNDLE}/Contents/Resources/zh-Hans.lproj"
mkdir -p "${APP_BUNDLE}/Contents/Resources/zh_CN.lproj"

cat <<EOF > "${APP_BUNDLE}/Contents/Resources/en.lproj/InfoPlist.strings"
"CFBundleDisplayName" = "Notype";
"CFBundleName" = "Notype";
"NSMicrophoneUsageDescription" = "Notype needs microphone access to record audio for speech-to-text dictation.";
"NSSpeechRecognitionUsageDescription" = "Notype needs speech recognition permissions to process your voice dictation.";
"NSAppleEventsUsageDescription" = "Notype needs accessibility and control permissions to paste polished text into your active application.";
"NSSystemAdministrationUsageDescription" = "Notype needs accessibility permissions to monitor the global dictation hotkey and insert text.";
EOF

cat <<EOF > "${APP_BUNDLE}/Contents/Resources/zh-Hans.lproj/InfoPlist.strings"
"CFBundleDisplayName" = "Notype";
"CFBundleName" = "Notype";
"NSMicrophoneUsageDescription" = "Notype 需要麦克风权限以进行高保真语音识别与转写。";
"NSSpeechRecognitionUsageDescription" = "Notype 需要语音识别权限以处理您的口述录音。";
"NSAppleEventsUsageDescription" = "Notype 需要辅助功能与控制权限以将润色文字直接插入到当前应用中。";
"NSSystemAdministrationUsageDescription" = "Notype 需要辅助功能权限以监听全局口述快捷键及自动输入文字。";
EOF

cp "${APP_BUNDLE}/Contents/Resources/zh-Hans.lproj/InfoPlist.strings" "${APP_BUNDLE}/Contents/Resources/zh_CN.lproj/InfoPlist.strings"

plutil -lint "${APP_BUNDLE}/Contents/Info.plist"
plutil -lint "${APP_BUNDLE}/Contents/Resources/en.lproj/InfoPlist.strings"
plutil -lint "${APP_BUNDLE}/Contents/Resources/zh-Hans.lproj/InfoPlist.strings"
plutil -lint "${APP_BUNDLE}/Contents/Resources/zh_CN.lproj/InfoPlist.strings"

# Code Signing Identity
SIGN_IDENTITY="${SIGN_IDENTITY:-$(security find-identity -v -p codesigning | grep "Developer ID Application:" | head -n 1 | sed -E 's/.*"([^"]+)".*/\1/' || true)}"

if [ -n "${SIGN_IDENTITY}" ]; then
    echo "==> Using Apple Developer ID certificate: ${SIGN_IDENTITY}"
    SIGN_ARGS=(--options runtime --timestamp --entitlements "${ROOT_DIR}/entitlements.plist")
else
    echo "==> No Developer ID certificate found; using ad-hoc signature (-)..."
    SIGN_IDENTITY="-"
    SIGN_ARGS=()
fi

echo "==> Code signing the inner binary and .app bundle..."
codesign --force "${SIGN_ARGS[@]}" --sign "${SIGN_IDENTITY}" "${APP_BUNDLE}/Contents/MacOS/${BIN_NAME}"
codesign --force "${SIGN_ARGS[@]}" --sign "${SIGN_IDENTITY}" "${APP_BUNDLE}"

echo "==> Creating macOS DMG installer package..."
DMG_ROOT="${RELEASE_DIR}/dmg_staging"
rm -rf "${DMG_ROOT}"
mkdir -p "${DMG_ROOT}"
cp -R "${APP_BUNDLE}" "${DMG_ROOT}/"
ln -s /Applications "${DMG_ROOT}/Applications"

rm -f "${DMG_PATH}"
hdiutil create \
    -volname "${APP_NAME}" \
    -srcfolder "${DMG_ROOT}" \
    -ov \
    -format UDZO \
    "${DMG_PATH}"

rm -rf "${DMG_ROOT}"

# Also make a friendly generic symlink release/Notype.dmg
ln -sf "${DMG_NAME}" "${RELEASE_DIR}/${APP_NAME}.dmg"

if [ "${SIGN_IDENTITY}" != "-" ]; then
    echo "==> Code signing the DMG installer package..."
    codesign --force --sign "${SIGN_IDENTITY}" --timestamp "${DMG_PATH}"
fi

echo "==> Packaging complete!"
echo "    App bundle: ${APP_BUNDLE}"
echo "    DMG installer: ${DMG_PATH}"
