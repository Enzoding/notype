#!/usr/bin/env bash
#
# Apple Notarization (官方公证) 脚本
#
# 前置准备 (只需配置一次):
#   在 https://appleid.apple.com 生成一个 App 专用密码，然后在终端执行：
#   xcrun notarytool store-credentials "notary-profile" \
#       --apple-id "你的AppleID邮箱" \
#       --team-id "QTASW57L4C" \
#       --password "xxxx-xxxx-xxxx-xxxx"
#
# 运行方式:
#   bash scripts/notarize.sh
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

KEYCHAIN_PROFILE="${KEYCHAIN_PROFILE:-notary-profile}"
VERSION="$(grep -m1 '^version = ' "${ROOT_DIR}/Cargo.toml" | cut -d '"' -f 2)"
DMG_PATH="${ROOT_DIR}/release/Notype-${VERSION}.dmg"

if [ ! -f "${DMG_PATH}" ]; then
    echo "❌ 找不到 ${DMG_PATH}，请先执行 bash scripts/package.sh 打包。" >&2
    exit 1
fi

echo "=========================================="
echo "     🍎 Apple 官方公证 (Notarization)      "
echo "=========================================="
echo "==> 正在向 Apple 公证服务器提交扫描..."
echo "    文件: ${DMG_PATH}"
echo "    Profile: ${KEYCHAIN_PROFILE}"

xcrun notarytool submit "${DMG_PATH}" \
    --keychain-profile "${KEYCHAIN_PROFILE}" \
    --wait

echo "==> 公证扫描通过！正在为 DMG 装订票据 (Staple)..."
xcrun stapler staple "${DMG_PATH}"

echo "==> 验证 Gatekeeper 评估状态..."
spctl -a -t open --context context:primary-signature -v "${DMG_PATH}" || true

echo ""
echo "=========================================="
echo "  🎉 公证完成！所有 Mac 用户均可直接秒开"
echo "=========================================="
