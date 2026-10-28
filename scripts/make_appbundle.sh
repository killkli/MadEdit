#!/usr/bin/env bash
#
# 產生 macOS 應用程式 bundle（MadEdit.app）。
#
# 用法：
#   ./scripts/make_appbundle.sh [輸出目錄]
#   MADEDIT_BIN=/path/to/MadEdit ./scripts/make_appbundle.sh [輸出目錄]
# 預設建置 release；指定 MADEDIT_BIN 時直接打包該執行檔（例如 universal binary）。
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

if [[ "$(uname)" != "Darwin" ]]; then
    echo "此腳本僅適用於 macOS" >&2
    exit 1
fi
for tool in cargo plutil sips iconutil codesign ditto file lipo vtool; do
    if ! command -v "${tool}" >/dev/null 2>&1; then
        echo "缺少必要工具：${tool}，請安裝 Rust 與 Xcode Command Line Tools" >&2
        exit 1
    fi
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"
APP_NAME="MadEdit"
BUNDLE_ID="io.github.madedit.MadEdit"
METADATA="$(cargo metadata --locked --no-deps --format-version 1 --manifest-path "${ROOT}/Cargo.toml")"
VERSION="$(printf '%s' "${METADATA}" | plutil -extract packages.0.version raw -o - -)"
OUT_DIR="${1:-${ROOT}/dist}"
mkdir -p "${OUT_DIR}"
OUT_DIR="$(cd "${OUT_DIR}" && pwd)"
FINAL_BUNDLE="${OUT_DIR}/${APP_NAME}.app"

# 先在同一檔案系統完成建置、資源及簽章；失敗時舊 App 仍可使用。
STAGE="$(mktemp -d "${OUT_DIR}/.madedit-bundle.XXXXXX")"
BUNDLE="${STAGE}/${APP_NAME}.app"
cleanup() {
    if [[ -e "${STAGE}/previous.app" && ! -e "${FINAL_BUNDLE}" ]]; then
        mv "${STAGE}/previous.app" "${FINAL_BUNDLE}"
    fi
    rm -rf "${STAGE}"
}
trap cleanup EXIT

BIN="${MADEDIT_BIN:-}"
if [[ -z "${BIN}" ]]; then
    # Cargo 回報的 artifact 才是實際路徑，包含自訂 target-dir 及 build.target。
    cargo build --release --locked --manifest-path "${ROOT}/Cargo.toml" \
        --message-format=json-render-diagnostics >"${STAGE}/build.jsonl"
    while IFS= read -r line; do
        case "${line}" in
            *'"executable":"'*)
                if [[ "$(printf '%s' "${line}" | plutil -extract target.name raw -o - -)" == "${APP_NAME}" ]]; then
                    BIN="$(printf '%s' "${line}" | plutil -extract executable raw -o - -)"
                fi
                ;;
        esac
    done <"${STAGE}/build.jsonl"
fi
if [[ ! -f "${BIN}" ]]; then
    echo "找不到 MadEdit 執行檔：${BIN}" >&2
    exit 1
fi
# 避免誤把其他平台的產物打包成無法執行的 macOS App。
if ! file "${BIN}" | grep -q 'Mach-O.*executable'; then
    echo "執行檔不是 macOS Mach-O：${BIN}" >&2
    exit 1
fi
# --- 選配：dprint 格式化工具（MADEDIT_WITH_FORMATTER=1 打包「帶 DPRINT」版） ---
WITH_FORMATTER="${MADEDIT_WITH_FORMATTER:-0}"
FORMATTER_SRC="${MADEDIT_FORMATTERS_DIR:-${ROOT}/target/formatters}"
FORMATTER_BIN="${BUNDLE}/Contents/Helpers/dprint"
if [[ "${WITH_FORMATTER}" == "1" ]]; then
    if [[ ! -x "${FORMATTER_SRC}/dprint" || ! -f "${FORMATTER_SRC}/dprint.json" ]]; then
        echo "未找到 dprint（${FORMATTER_SRC}），下載固定版本..."
        "${ROOT}/scripts/fetch_formatter.sh" "${FORMATTER_SRC}"
    fi
    if ! file "${FORMATTER_SRC}/dprint" | grep -q 'Mach-O.*executable'; then
        echo "${FORMATTER_SRC}/dprint 不是 macOS 執行檔；請以對應架構重新下載。" >&2
        echo "例：./scripts/fetch_formatter.sh target/formatters --target aarch64-apple-darwin" >&2
        exit 1
    fi
    mkdir -p "${BUNDLE}/Contents/Helpers" "${BUNDLE}/Contents/Resources/formatters/plugins"
    cp "${FORMATTER_SRC}/dprint" "${FORMATTER_BIN}"
    chmod +x "${FORMATTER_BIN}"
    cp "${FORMATTER_SRC}/dprint.json" "${BUNDLE}/Contents/Resources/formatters/dprint.json"
    cp "${FORMATTER_SRC}"/plugins/*.wasm "${BUNDLE}/Contents/Resources/formatters/plugins/"
    echo "已附帶 dprint 與插件（來源：${FORMATTER_SRC}）"
fi

# 從每個架構的 Mach-O 載入命令讀取最低 macOS 版本；Intel 舊版可能使用
# LC_VERSION_MIN_MACOSX，Apple Silicon 與較新版產物使用 LC_BUILD_VERSION。
# universal App 同時記錄各架構版本，全域值採最高者，避免低估執行需求。
# 附帶 dprint helper 時一併納入計算；同一架構只記錄一次（Info.plist 是 dict）。
MIN_OS="0.0"
ARCH_MINIMUMS=""
MACHO_BINS=("${BIN}")
if [[ "${WITH_FORMATTER}" == "1" ]]; then
    MACHO_BINS+=("${FORMATTER_BIN}")
fi
SEEN_ARCHS=" "
for macho in "${MACHO_BINS[@]}"; do
    for arch in $(lipo -archs "${macho}"); do
        arch_min="$(vtool -arch "${arch}" -show-build "${macho}" | awk '
            $1 == "cmd" { macos = ($2 == "LC_VERSION_MIN_MACOSX"); legacy = macos }
            $1 == "platform" { macos = ($2 == "MACOS") }
            macos && ($1 == "minos" || (legacy && $1 == "version")) { print $2; found = 1; exit }
            END { if (!found) exit 1 }
        ')" || {
            echo "無法讀取 ${arch} 的 macOS 最低版本：${macho}" >&2
            exit 1
        }
        MIN_OS="$(printf '%s\n%s\n' "${MIN_OS}" "${arch_min}" | sort -t . -k1,1n -k2,2n -k3,3n | tail -n 1)"
        if [[ "${SEEN_ARCHS}" != *" ${arch} "* ]]; then
            SEEN_ARCHS+="${arch} "
            ARCH_MINIMUMS+="        <key>${arch}</key><string>${arch_min}</string>
"
        fi
    done
done
if [[ "${MIN_OS}" == "0.0" ]]; then
    echo "執行檔未包含可辨識的 macOS 架構：${BIN}" >&2
    exit 1
fi
if [[ -L "${FINAL_BUNDLE}" ]]; then
    echo "拒絕取代符號連結：${FINAL_BUNDLE}" >&2
    exit 1
fi
if [[ -e "${FINAL_BUNDLE}" ]] && \
    [[ "$(plutil -extract CFBundleIdentifier raw -o - "${FINAL_BUNDLE}/Contents/Info.plist" 2>/dev/null || true)" != "${BUNDLE_ID}" ]]; then
    echo "拒絕取代非 MadEdit 的項目：${FINAL_BUNDLE}" >&2
    exit 1
fi
mkdir -p "${BUNDLE}/Contents/MacOS" "${BUNDLE}/Contents/Resources"

# --- 執行檔 ---------------------------------------------------------------
cp "${BIN}" "${BUNDLE}/Contents/MacOS/${APP_NAME}"
# 拼字檢查字典（en_US 已內嵌於執行檔，其他語言自 Resources/dictionaries 載入）
mkdir -p "${BUNDLE}/Contents/Resources/dictionaries"
cp "${ROOT}"/assets/dictionaries/*.aff "${ROOT}"/assets/dictionaries/*.dic "${BUNDLE}/Contents/Resources/dictionaries/"
chmod +x "${BUNDLE}/Contents/MacOS/${APP_NAME}"

# --- 圖示：PNG -> 圖示集 -> icns -----------------------------------------
ICON_SRC="${ROOT}/assets/madedit-icon.png"
ICONSET="${STAGE}/AppIcon.iconset"
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
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key>
            <string>文字與二進位文件</string>
            <key>CFBundleTypeRole</key>
            <string>Editor</string>
            <key>LSHandlerRank</key>
            <string>Alternate</string>
            <key>LSItemContentTypes</key>
            <array>
                <string>public.text</string>
                <string>public.data</string>
            </array>
        </dict>
    </array>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.developer-tools</string>
    <key>LSMinimumSystemVersion</key>
    <string>${MIN_OS}</string>
    <key>LSMinimumSystemVersionByArchitecture</key>
    <dict>
${ARCH_MINIMUMS}    </dict>
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
# ad-hoc 簽章供本機執行；對外發行仍需 Developer ID 簽章與 Apple 公證。
# 簽章及驗證任一步失敗都中止，避免把壞掉的 App 當成成功產物。
plutil -lint "${BUNDLE}/Contents/Info.plist"
if [[ "${WITH_FORMATTER}" == "1" && -x "${FORMATTER_BIN}" ]]; then
    # dprint 內建 wasmtime 需要 Hardened Runtime 的可執行記憶體權限；
    # 只授予 helper，MadEdit 主程式不帶這項 entitlement。
    codesign --force --options runtime \
        --entitlements "${ROOT}/assets/formatters/dprint.entitlements" \
        --sign - "${FORMATTER_BIN}"
fi
codesign --force --sign - "${BUNDLE}/Contents/MacOS/${APP_NAME}"
codesign --force --sign - "${BUNDLE}"
codesign --verify --deep --strict "${BUNDLE}"

if [[ -e "${FINAL_BUNDLE}" ]]; then
    mv "${FINAL_BUNDLE}" "${STAGE}/previous.app"
fi
mv "${BUNDLE}" "${FINAL_BUNDLE}"
echo "已產生：${FINAL_BUNDLE}"
echo "啟動方式： open \"${FINAL_BUNDLE}\""
