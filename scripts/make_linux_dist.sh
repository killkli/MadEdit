#!/usr/bin/env bash
#
# 產生 Linux x86_64 發行壓縮檔（MadEdit-<版本>-linux-x86_64.tar.gz）。
#
# 用法：
#   ./scripts/make_linux_dist.sh [輸出目錄]
#
# 為什麼用 cargo-zigbuild：直接在新版發行版（如 Arch）建置的執行檔會連結到
# 當地的新版 glibc，在 Ubuntu 22.04 等較舊系統上無法執行。zigbuild 可指定
# 目標 glibc 版本（此處為 2.28），讓同一個執行檔可在多數發行版上執行。
#
# 需要：
#   cargo install cargo-zigbuild
#   zig（可透過系統套件管理員安裝，或以 `uv tool install ziglang` 安裝）
#
# 產物結構：
#   MadEdit-<版本>-linux-x86_64/
#     MadEdit                  執行檔（已 strip）
#     dictionaries/            拼字字典（en_US 已內嵌，其餘執行時載入）
#     madedit.desktop          桌面整合項目
#     madedit-icon.png         圖示
#     install.sh               安裝到 ~/.local（或以 PREFIX 指定）
#     README.txt
set -euo pipefail

# 若未自訂 CARGO_ZIGBUILD_ZIG_PATH 且 PATH 中無 zig，但有 python-zig（uv 安裝），則自動帶入
if [[ -z "${CARGO_ZIGBUILD_ZIG_PATH:-}" ]] && ! command -v zig >/dev/null 2>&1 && command -v python-zig >/dev/null 2>&1; then
    export CARGO_ZIGBUILD_ZIG_PATH="$(command -v python-zig)"
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="MadEdit"
GLIBC="2.28"
TARGET="x86_64-unknown-linux-gnu"
VERSION="$(grep -m1 '^version' "${ROOT}/Cargo.toml" | sed -E 's/.*=[[:space:]]*"([^"]+)".*/\1/')"

OUT_DIR="${1:-${ROOT}/dist}"
NAME="${APP_NAME}-${VERSION}-linux-x86_64"
STAGE="${OUT_DIR}/${NAME}"

# --- 建置 -----------------------------------------------------------------
(cd "${ROOT}" && cargo zigbuild --release --target "${TARGET}.${GLIBC}")
BIN="${ROOT}/target/${TARGET}/release/${APP_NAME}"

rm -rf "${STAGE}"
mkdir -p "${STAGE}/dictionaries"

# --- 內容 -----------------------------------------------------------------
cp "${BIN}" "${STAGE}/${APP_NAME}"
if command -v llvm-strip >/dev/null 2>&1; then
    llvm-strip "${STAGE}/${APP_NAME}" || true
elif command -v strip >/dev/null 2>&1 && [[ "$(uname)" == "Linux" ]]; then
    strip "${STAGE}/${APP_NAME}" || true
fi
cp "${ROOT}"/assets/dictionaries/* "${STAGE}/dictionaries/"
cp "${ROOT}/assets/madedit.desktop" "${STAGE}/"
cp "${ROOT}/assets/madedit-icon.png" "${STAGE}/"

# 安裝腳本：執行檔與字典放在同一目錄（spell.rs 會找 <執行檔目錄>/dictionaries），
# 再以符號連結放進 bin，並註冊桌面項目與圖示。
cat > "${STAGE}/install.sh" <<'EOF'
#!/usr/bin/env bash
# 安裝 MadEdit。預設安裝到 ~/.local；系統安裝請用 sudo PREFIX=/usr/local ./install.sh
set -euo pipefail
SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-${HOME}/.local}"
APP_DIR="${PREFIX}/lib/madedit"

mkdir -p "${APP_DIR}" "${PREFIX}/bin" \
         "${PREFIX}/share/applications" "${PREFIX}/share/icons/hicolor/48x48/apps"
rm -rf "${APP_DIR}/dictionaries"
# install 會先移除舊檔再寫入，程式執行中也能升級（cp 會因「文字檔忙碌中」失敗）
install -m 755 "${SRC}/MadEdit" "${APP_DIR}/MadEdit"
cp -r "${SRC}/dictionaries" "${APP_DIR}/"
ln -sf "${APP_DIR}/MadEdit" "${PREFIX}/bin/MadEdit"
# Exec 改寫為絕對路徑：從啟動器（如 Walker）開啟時 PATH 不一定含 ${PREFIX}/bin；
# %F 讓「以此程式開啟」可傳入檔案
sed "s|^Exec=.*|Exec=${APP_DIR}/MadEdit %F|" "${SRC}/madedit.desktop" \
    > "${PREFIX}/share/applications/madedit.desktop"
cp "${SRC}/madedit-icon.png" "${PREFIX}/share/icons/hicolor/48x48/apps/"
command -v update-desktop-database >/dev/null && update-desktop-database -q "${PREFIX}/share/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q "${PREFIX}/share/icons/hicolor" || true

echo "已安裝至 ${PREFIX}（執行檔：${PREFIX}/bin/MadEdit）"
echo "若無法在終端機直接執行 MadEdit，請確認 ${PREFIX}/bin 在 PATH 中。"
EOF
chmod +x "${STAGE}/install.sh"

cat > "${STAGE}/README.txt" <<EOF
MadEdit ${VERSION} — Linux x86_64

直接執行：
  ./MadEdit [檔案...]

安裝到 ~/.local（含應用程式選單項目與圖示）：
  ./install.sh
系統安裝：
  sudo PREFIX=/usr/local ./install.sh

需求：glibc 2.28 以上、fontconfig、X11 或 Wayland，以及支援 Vulkan 或 OpenGL 的顯示驅動。
原始碼：https://github.com/tenyi/MadEdit
EOF

# --- 壓縮 -----------------------------------------------------------------
tar -C "${OUT_DIR}" -czf "${OUT_DIR}/${NAME}.tar.gz" "${NAME}"
if command -v sha256sum >/dev/null 2>&1; then
    (cd "${OUT_DIR}" && sha256sum "${NAME}.tar.gz" > "${NAME}.tar.gz.sha256")
elif command -v shasum >/dev/null 2>&1; then
    (cd "${OUT_DIR}" && shasum -a 256 "${NAME}.tar.gz" > "${NAME}.tar.gz.sha256")
fi
echo "完成：${OUT_DIR}/${NAME}.tar.gz"
