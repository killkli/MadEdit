#!/usr/bin/env bash
# 以微型原生執行檔驗證 macOS 打包與失敗保護；不開啟 GUI、不變更安裝中的 App。
set -euo pipefail
if [[ "$(uname)" != "Darwin" ]]; then
    echo "此測試僅適用於 macOS" >&2
    exit 1
fi
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STAGE="$(mktemp -d "${TMPDIR:-/tmp}/madedit-bundle-test.XXXXXX")"
trap 'rm -rf "${STAGE}"' EXIT
printf 'int main(void) { return 0; }\n' >"${STAGE}/fixture.c"
# 同時覆蓋舊式 Intel 版本命令與新版 Apple Silicon 版本命令。
clang -target x86_64-apple-macos10.13 "${STAGE}/fixture.c" -o "${STAGE}/intel"
clang -target arm64-apple-macos11.0 "${STAGE}/fixture.c" -o "${STAGE}/arm"
lipo -create "${STAGE}/intel" "${STAGE}/arm" -output "${STAGE}/MadEdit"
OUT="${STAGE}/output with spaces"
MADEDIT_BIN="${STAGE}/MadEdit" "${ROOT}/scripts/make_appbundle.sh" "${OUT}"
APP="${OUT}/MadEdit.app"
codesign --verify --deep --strict "${APP}"
"${APP}/Contents/MacOS/MadEdit"
[[ -s "${APP}/Contents/Resources/AppIcon.icns" ]]
[[ -s "${APP}/Contents/Resources/dictionaries/fr_FR.dic" ]]
[[ "$(plutil -extract CFBundleDocumentTypes.0.LSHandlerRank raw -o - "${APP}/Contents/Info.plist")" == "Alternate" ]]
[[ "$(plutil -extract LSMinimumSystemVersion raw -o - "${APP}/Contents/Info.plist")" == "11.0" ]]
[[ "$(plutil -extract LSMinimumSystemVersionByArchitecture.x86_64 raw -o - "${APP}/Contents/Info.plist")" == "10.13" ]]
[[ "$(plutil -extract LSMinimumSystemVersionByArchitecture.arm64 raw -o - "${APP}/Contents/Info.plist")" == "11.0" ]]
BEFORE="$(shasum -a 256 "${APP}/Contents/_CodeSignature/CodeResources")"

# 簽章失敗不能回報成功，也不能改掉上一次可用的 bundle。
mkdir "${STAGE}/bin"
printf '#!/bin/sh\nexit 1\n' >"${STAGE}/bin/codesign"
chmod +x "${STAGE}/bin/codesign"
if PATH="${STAGE}/bin:${PATH}" MADEDIT_BIN="${STAGE}/MadEdit" \
    "${ROOT}/scripts/make_appbundle.sh" "${OUT}"; then
    echo "錯誤：簽章失敗卻回報成功" >&2
    exit 1
fi
[[ "$(shasum -a 256 "${APP}/Contents/_CodeSignature/CodeResources")" == "${BEFORE}" ]]
codesign --verify --deep --strict "${APP}"

# 非 Mach-O 產物與非 MadEdit 的安裝目的地都必須被拒絕。
if MADEDIT_BIN="${STAGE}/fixture.c" "${ROOT}/scripts/make_appbundle.sh" "${OUT}"; then
    echo "錯誤：接受了非 macOS 執行檔" >&2
    exit 1
fi
mkdir "${STAGE}/unrelated.app"
printf '保留\n' >"${STAGE}/unrelated.app/keep.txt"
if DEST="${STAGE}/unrelated.app" "${ROOT}/scripts/install_macos.sh"; then
    echo "錯誤：接受了非 MadEdit 的安裝目的地" >&2
    exit 1
fi
[[ -f "${STAGE}/unrelated.app/keep.txt" ]]
echo "macOS 打包、簽章與失敗保護測試通過"
