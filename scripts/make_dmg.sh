#!/usr/bin/env bash
#
# 產生 macOS 磁碟映像檔（MadEdit-<版本>-macOS-<架構>.dmg）。
#
# 用法：
#   ./scripts/make_dmg.sh [輸出目錄]
#
# 產物結構：
#   dist/MadEdit-<版本>-macOS-<架構>.dmg
#     - MadEdit.app
#     - Applications -> /Applications
#
set -euo pipefail

if [[ "$(uname)" != "Darwin" ]]; then
    echo "此腳本僅適用於 macOS" >&2
    exit 1
fi

for tool in hdiutil plutil codesign ditto file lipo; do
    if ! command -v "${tool}" >/dev/null 2>&1; then
        echo "缺少必要工具：${tool}，請安裝 Xcode Command Line Tools" >&2
        exit 1
    fi
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="MadEdit"
OUT_DIR="${1:-${ROOT}/dist}"
mkdir -p "${OUT_DIR}"
OUT_DIR="$(cd "${OUT_DIR}" && pwd)"

SOURCE_APP="${OUT_DIR}/${APP_NAME}.app"
if [[ ! -d "${SOURCE_APP}" ]]; then
    echo "找不到 ${SOURCE_APP}，正在建置 app bundle..."
    "${ROOT}/scripts/make_appbundle.sh" "${OUT_DIR}"
fi

# 讀取版本號
VERSION="$(plutil -extract CFBundleShortVersionString raw -o - "${SOURCE_APP}/Contents/Info.plist" 2>/dev/null || true)"
if [[ -z "${VERSION}" ]]; then
    METADATA="$(cargo metadata --locked --no-deps --format-version 1 --manifest-path "${ROOT}/Cargo.toml")"
    VERSION="$(printf '%s' "${METADATA}" | plutil -extract packages.0.version raw -o - -)"
fi

# 判斷架構
BIN="${SOURCE_APP}/Contents/MacOS/${APP_NAME}"
ARCHS="$(lipo -archs "${BIN}")"
if [[ "${ARCHS}" == *"arm64"* && "${ARCHS}" == *"x86_64"* ]]; then
    ARCH_SUFFIX="macOS-universal"
elif [[ "${ARCHS}" == "arm64" ]]; then
    ARCH_SUFFIX="macOS-arm64"
elif [[ "${ARCHS}" == "x86_64" ]]; then
    ARCH_SUFFIX="macOS-x86_64"
else
    ARCH_SUFFIX="macOS-${ARCHS// /-}"
fi

DMG_NAME="${APP_NAME}-${VERSION}-${ARCH_SUFFIX}.dmg"
# App 內附帶 dprint helper 時，檔名加上 -dprint 以區隔「帶 DPRINT」版本
if [[ -x "${SOURCE_APP}/Contents/Helpers/dprint" ]]; then
    DMG_NAME="${APP_NAME}-${VERSION}-${ARCH_SUFFIX}-dprint.dmg"
fi
DMG_PATH="${OUT_DIR}/${DMG_NAME}"

STAGE="$(mktemp -d "${TMPDIR:-/tmp}/madedit-dmg.XXXXXX")"
cleanup() {
    rm -rf "${STAGE}"
}
trap cleanup EXIT

echo "準備 DMG 內容..."
ditto "${SOURCE_APP}" "${STAGE}/${APP_NAME}.app"
ln -s /Applications "${STAGE}/Applications"

echo "正在建立 ${DMG_NAME}..."
rm -f "${DMG_PATH}"
hdiutil create -volname "${APP_NAME}" -srcfolder "${STAGE}" -ov -format UDZO "${DMG_PATH}"

echo "對 DMG 進行簽章..."
codesign --force --sign - "${DMG_PATH}"

echo "驗證 DMG 掛載..."
MOUNT_DIR="$(mktemp -d "${TMPDIR:-/tmp}/madedit-mnt.XXXXXX")"
hdiutil attach -nobrowse -readonly -mountpoint "${MOUNT_DIR}" "${DMG_PATH}" >/dev/null
if [[ ! -d "${MOUNT_DIR}/${APP_NAME}.app" || ! -L "${MOUNT_DIR}/Applications" ]]; then
    hdiutil detach "${MOUNT_DIR}" >/dev/null || true
    rm -rf "${MOUNT_DIR}"
    echo "錯誤：DMG 內容驗證失敗" >&2
    exit 1
fi
hdiutil detach "${MOUNT_DIR}" >/dev/null
rm -rf "${MOUNT_DIR}"

(cd "${OUT_DIR}" && shasum -a 256 "${DMG_NAME}" > "${DMG_NAME}.sha256")

echo "DMG 建立完成：${DMG_PATH}"
