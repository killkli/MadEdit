#!/usr/bin/env bash
#
# 一鍵建置、驗證並安裝 MadEdit.app（僅限 macOS）。
# 用法：./scripts/install_macos.sh
#       DEST="$HOME/Applications/MadEdit.app" ./scripts/install_macos.sh
# 新版複製與簽章驗證完成後才取代舊版，失敗時保留原本的 App。
set -euo pipefail

if [[ "$(uname)" != "Darwin" ]]; then
    echo "此腳本僅適用於 macOS" >&2
    exit 1
fi

for tool in plutil codesign ditto; do
    if ! command -v "${tool}" >/dev/null 2>&1; then
        echo "缺少必要工具：${tool}，請安裝 Xcode Command Line Tools" >&2
        exit 1
    fi
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="MadEdit"
BUNDLE_ID="io.github.madedit.MadEdit"
DEST="${DEST:-/Applications/${APP_NAME}.app}"
case "${DEST}" in
    *.app) ;;
    *) echo "安裝目的地必須以 .app 結尾：${DEST}" >&2; exit 1 ;;
esac
if [[ -L "${DEST}" ]]; then
    echo "拒絕取代符號連結：${DEST}" >&2
    exit 1
fi
if [[ -e "${DEST}" ]] && \
    [[ "$(plutil -extract CFBundleIdentifier raw -o - "${DEST}/Contents/Info.plist" 2>/dev/null || true)" != "${BUNDLE_ID}" ]]; then
    echo "拒絕取代非 MadEdit 的項目：${DEST}" >&2
    exit 1
fi

"${ROOT}/scripts/make_appbundle.sh" "${ROOT}/dist"

# 執行中的舊版仍可繼續使用，重新啟動後才會換成新版。
if pgrep -x "${APP_NAME}" >/dev/null 2>&1; then
    echo "提醒：${APP_NAME} 正在執行，重新啟動後才會使用新版" >&2
fi

DEST_DIR="$(dirname "${DEST}")"
mkdir -p "${DEST_DIR}"
DEST_DIR="$(cd "${DEST_DIR}" && pwd)"
DEST="${DEST_DIR}/$(basename "${DEST}")"
STAGE="$(mktemp -d "${DEST_DIR}/.madedit-install.XXXXXX")"
cleanup() {
    if [[ -e "${STAGE}/previous.app" && ! -e "${DEST}" ]]; then
        mv "${STAGE}/previous.app" "${DEST}"
    fi
    rm -rf "${STAGE}"
}
trap cleanup EXIT

# ditto 保留權限與簽章；先複製至暫存區，可在空間不足等錯誤發生時保留舊版。
ditto "${ROOT}/dist/${APP_NAME}.app" "${STAGE}/${APP_NAME}.app"
codesign --verify --deep --strict "${STAGE}/${APP_NAME}.app"
if [[ -e "${DEST}" ]]; then
    mv "${DEST}" "${STAGE}/previous.app"
fi
mv "${STAGE}/${APP_NAME}.app" "${DEST}"
echo "已安裝：${DEST}"
