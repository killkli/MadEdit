#!/usr/bin/env bash
#
# 一鍵建置 MadEdit.app 並安裝到 /Applications（僅限 macOS）。
#
# 用法：
#   ./scripts/install_macos.sh
#
# 流程：cargo build --release → make_appbundle.sh 產生 dist/MadEdit.app
#       → 刪除 /Applications 內的舊版 → 複製新版過去。
set -euo pipefail

if [[ "$(uname)" != "Darwin" ]]; then
    echo "此腳本僅適用於 macOS" >&2
    exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="MadEdit"
DEST="${DEST:-/Applications/${APP_NAME}.app}"

cargo build --release --manifest-path "${ROOT}/Cargo.toml"
"${ROOT}/scripts/make_appbundle.sh" "${ROOT}/dist"

# 執行中的舊版仍可繼續使用，但要重新啟動才會換成新版
if pgrep -x "${APP_NAME}" >/dev/null 2>&1; then
    echo "提醒：${APP_NAME} 正在執行，重新啟動後才會使用新版" >&2
fi

if [[ -e "${DEST}" ]]; then
    rm -rf "${DEST}"
    echo "已刪除舊版：${DEST}"
fi
# ditto 會保留 bundle 的權限、擴充屬性與簽章
ditto "${ROOT}/dist/${APP_NAME}.app" "${DEST}"

echo "已安裝：${DEST}"
