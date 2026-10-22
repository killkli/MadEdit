#!/usr/bin/env bash
#
# 產生 macOS 應用程式 bundle（MadEdit.app）。
#
# 用法：
#   cargo build --release
#   ./scripts/make_appbundle.sh [輸出目錄]
#
# 產物結構：
#   dist/MadEdit.app/Contents/MacOS/MadEdit   執行檔
#   dist/MadEdit.app/Contents/Resources/AppIcon.icns
#   dist/MadEdit.app/Contents/Info.plist
#
# 為什麼需要這個腳本：GUI 應用程式在 macOS 上必須位於 .app bundle 內，
# 才能取得正確的 NSApplication 行為（Dock 圖示、選單列名稱、原生視窗），
# rfd 的原生檔案對話框也會以 bundle 身分與系統互動。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="MadEdit"
BUNDLE_ID="io.github.madedit.MadEdit"
VERSION="$(grep -m1 '^version' "${ROOT}/Cargo.toml" | sed -E 's/.*=[[:space:]]*"([^"]+)".*/\1/')"

BIN="${ROOT}/target/release/${APP_NAME}"
OUT_DIR="${1:-${ROOT}/dist}"
BUNDLE="${OUT_DIR}/${APP_NAME}.app"

if [[ ! -f "${BIN}" ]]; then
    echo "找不到執行檔 ${BIN}" >&2
    echo "請先執行： cargo build --release" >&2
    exit 1
fi

rm -rf "${BUNDLE}"
mkdir -p "${BUNDLE}/Contents/MacOS" "${BUNDLE}/Contents/Resources"

# --- 執行檔 ---------------------------------------------------------------
cp "${BIN}" "${BUNDLE}/Contents/MacOS/${APP_NAME}"
# 拼字檢查字典（en_US 已內嵌於執行檔，其他語言自 Resources/dictionaries 載入）
mkdir -p "${BUNDLE}/Contents/Resources/dictionaries"
cp "${ROOT}"/assets/dictionaries/*.aff "${ROOT}"/assets/dictionaries/*.dic "${BUNDLE}/Contents/Resources/dictionaries/"
chmod +x "${BUNDLE}/Contents/MacOS/${APP_NAME}"

# --- 圖示：PNG -> 圖示集 -> icns -----------------------------------------
ICON_SRC="${ROOT}/assets/madedit-icon.png"
ICONSET="${OUT_DIR}/AppIcon.iconset"
if [[ -f "${ICON_SRC}" ]]; then
    rm -rf "${ICONSET}"
    mkdir -p "${ICONSET}"
    # macOS 圖標集需要每個邏輯尺寸各一張 1x 與 2x 版本
    for base in 16 32 128 256 512; do
        sips -z "${base}" "${base}" "${ICON_SRC}" \
            --out "${ICONSET}/icon_${base}x${base}.png" >/dev/null
        sips -z $((base * 2)) $((base * 2)) "${ICON_SRC}" \
            --out "${ICONSET}/icon_${base}x${base}@2x.png" >/dev/null
    done
    iconutil -c icns "${ICONSET}" -o "${BUNDLE}/Contents/Resources/AppIcon.icns"
    rm -rf "${ICONSET}"
    echo "圖示已產生：AppIcon.icns"
else
    echo "找不到 ${ICON_SRC}，略過圖示產生" >&2
fi

# --- Info.plist -----------------------------------------------------------
cat >"${BUNDLE}/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
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
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleExecutable</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleDevelopmentRegion</key>
    <string>zh_TW</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.developer-tools</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
    <key>NSHumanReadableCopyright</key>
    <string>MadEdit Rust 重製版</string>
</dict>
</plist>
PLIST

printf 'APPL????' >"${BUNDLE}/Contents/PkgInfo"

# --- 簽章 -----------------------------------------------------------------
# ad-hoc 簽章讓 bundle 在本機可正常啟動，不會被 Gatekeeper 擋下。
# 先簽內部執行檔再簽 bundle，確保巢狀簽章完整。
if command -v codesign >/dev/null 2>&1; then
    codesign --force --sign - "${BUNDLE}/Contents/MacOS/${APP_NAME}" >/dev/null 2>&1 || \
        echo "警告：執行檔簽章失敗（不影響本機執行）" >&2
    codesign --force --sign - "${BUNDLE}" >/dev/null 2>&1 || \
        echo "警告：bundle 簽章失敗（不影響本機執行）" >&2
    xattr -cr "${BUNDLE}" 2>/dev/null || true
fi

echo "已產生：${BUNDLE}"
echo "啟動方式： open \"${BUNDLE}\""
