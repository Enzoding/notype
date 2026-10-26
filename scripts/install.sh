#!/usr/bin/env bash
#
# Notype 一键安装与升级脚本 (macOS)
# 用法:
#   curl -fsSL https://raw.githubusercontent.com/Enzoding/notype/main/scripts/install.sh | bash
#
set -euo pipefail

# 1. 系统环境校验
if [ "$(uname)" != "Darwin" ]; then
    echo "❌ 错误: Notype 是一款专为 macOS 设计的桌面应用，暂不支持其他操作系统。" >&2
    exit 1
fi

echo "=========================================="
echo "      🚀 欢迎安装 Notype (macOS)         "
echo "=========================================="

REPO="Enzoding/notype"
DMG_TMP="/tmp/Notype-installer-$$.dmg"
MOUNT_DIR="/tmp/notype_mount_$$"

cleanup() {
    if [ -d "${MOUNT_DIR}" ]; then
        hdiutil detach "${MOUNT_DIR}" -force >/dev/null 2>&1 || true
        rm -rf "${MOUNT_DIR}"
    fi
    rm -f "${DMG_TMP}"
}
trap cleanup EXIT

# 2. 检查是否有 GitHub Token（用于私有仓库或提升 API 限额）
AUTH_HEADER=()
if [ -n "${GITHUB_TOKEN:-}" ]; then
    AUTH_HEADER=(-H "Authorization: Bearer ${GITHUB_TOKEN}")
elif [ -n "${GH_TOKEN:-}" ]; then
    AUTH_HEADER=(-H "Authorization: Bearer ${GH_TOKEN}")
fi

echo "==> 正在获取最新版本信息..."

# 优先尝试从 Release API 解析最新版本，如果未公开或 API 受限则回退至通用下载链接
DOWNLOAD_URL=""
if RELEASE_JSON=$(curl -fsSL "${AUTH_HEADER[@]}" "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null); then
    DOWNLOAD_URL=$(echo "${RELEASE_JSON}" | grep -o 'https://[^"]*Notype[^"]*\.dmg' | head -n 1 || true)
fi

if [ -z "${DOWNLOAD_URL}" ]; then
    DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/Notype.dmg"
fi

echo "==> 正在从 GitHub 下载最新版本..."
echo "    来源: ${DOWNLOAD_URL}"

if ! curl -fL "${AUTH_HEADER[@]}" --progress-bar -o "${DMG_TMP}" "${DOWNLOAD_URL}"; then
    echo ""
    echo "❌ 下载失败！"
    echo "💡 提示: 如果该仓库仍为私有状态，请在终端执行："
    echo "   GITHUB_TOKEN=你的Token bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/${REPO}/main/scripts/install.sh)\""
    echo "   或者将仓库在 GitHub 设为 Public 即可让所有用户免 Token 安装。"
    exit 1
fi

echo ""
echo "==> 正在挂载安装镜像并部署应用..."
mkdir -p "${MOUNT_DIR}"
hdiutil attach -nobrowse -readonly -mountpoint "${MOUNT_DIR}" "${DMG_TMP}" >/dev/null

if [ ! -d "${MOUNT_DIR}/Notype.app" ]; then
    echo "❌ 镜像中未找到 Notype.app，安装失败。" >&2
    exit 1
fi

# 关闭当前运行中的旧进程（若有）
if pgrep -x "notype" >/dev/null 2>&1; then
    echo "==> 检测到正在运行的 Notype，准备重启更新..."
    pkill -x "notype" >/dev/null 2>&1 || true
    sleep 0.5
fi

# 部署至 /Applications
TARGET_APP="/Applications/Notype.app"
rm -rf "${TARGET_APP}"
cp -R "${MOUNT_DIR}/Notype.app" "/Applications/"

echo "==> 正在自动解除 macOS Gatekeeper 安全隔离限制 (xattr -cr)..."
xattr -cr "${TARGET_APP}" 2>/dev/null || true

echo "==> 正在清理临时安装文件..."
cleanup
trap - EXIT

echo ""
echo "=========================================="
echo "  ✅ Notype 安装成功！"
echo "  📍 位置: /Applications/Notype.app"
echo "  🛡️ 安全隔离已自动解除，无需手动放行"
echo "=========================================="
echo ""

echo "==> 正在启动 Notype..."
open "${TARGET_APP}"
