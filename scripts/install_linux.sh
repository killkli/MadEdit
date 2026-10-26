#!/usr/bin/env bash
#
# 一鍵建置 Linux 發行檔並安裝到本機（Arch / Omarchy 等）。
#
# 用法：
#   ./scripts/install_linux.sh                       # 安裝到 ~/.local
#   sudo PREFIX=/usr/local ./scripts/install_linux.sh # 系統安裝
#
# 流程：make_linux_dist.sh 產生 dist/MadEdit-<版本>-linux-<本機架構>/
#       → 執行其中的 install.sh（執行檔、字典、桌面項目與圖示），
#       安裝後即可從應用程式啟動器（Omarchy 為 Super+Space）找到 MadEdit。
#
# 需要：cargo-zigbuild 與 zig（見 make_linux_dist.sh）。
set -euo pipefail

if [[ "$(uname)" != "Linux" ]]; then
    echo "此腳本僅適用於 Linux" >&2
    exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="MadEdit"
VERSION="$(grep -m1 '^version' "${ROOT}/Cargo.toml" | sed -E 's/.*=[[:space:]]*"([^"]+)".*/\1/')"
ARCH="$(uname -m | sed -e 's/amd64/x86_64/' -e 's/arm64/aarch64/')"
STAGE="${ROOT}/dist/${APP_NAME}-${VERSION}-linux-${ARCH}"

"${ROOT}/scripts/make_linux_dist.sh" "${ROOT}/dist" "${ARCH}"

# 執行中的舊版仍可繼續使用，但要重新啟動才會換成新版
if pgrep -x "${APP_NAME}" >/dev/null 2>&1; then
    echo "提醒：${APP_NAME} 正在執行，重新啟動後才會使用新版" >&2
fi

"${STAGE}/install.sh"
