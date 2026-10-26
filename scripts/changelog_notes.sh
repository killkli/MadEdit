#!/usr/bin/env bash
#
# 從 ChangeLog.md 取出指定版本的發行說明。
#
# 用法：
#   ./scripts/changelog_notes.sh 0.5.24
#   ./scripts/changelog_notes.sh v0.5.24
#   ./scripts/changelog_notes.sh Unreleased
#
# 章節標題須為「## [版本]」或「## [版本] - 日期」。輸出不含標題本身，
# 供 GitHub Release 直接當作說明本文。找不到章節或本文為空時以非零狀態結束。
set -euo pipefail

if [[ $# -ne 1 || -z "${1}" ]]; then
    echo "用法：$(basename "$0") <版本>" >&2
    exit 1
fi

VER="${1#v}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FILE="${ROOT}/ChangeLog.md"
if [[ ! -f "${FILE}" ]]; then
    echo "找不到 ${FILE}" >&2
    exit 1
fi

# 用字串比對章節標題，避免版本號裡的點被當成正規表達式。
NOTES="$(awk -v ver="${VER}" '
    BEGIN { prefix = "## [" ver "]"; found = 0; n = 0 }
    index($0, prefix) == 1 {
        found = 1
        next
    }
    found && index($0, "## [") == 1 { exit }
    found { lines[++n] = $0 }
    END {
        if (!found) exit 2
        start = 1
        while (start <= n && lines[start] ~ /^[[:space:]]*$/) start++
        end = n
        while (end >= start && lines[end] ~ /^[[:space:]]*$/) end--
        if (end < start) exit 3
        for (i = start; i <= end; i++) print lines[i]
    }
' "${FILE}")" || {
    status=$?
    if [[ "${status}" -eq 2 ]]; then
        echo "ChangeLog.md 沒有 ## [${VER}] 章節" >&2
    elif [[ "${status}" -eq 3 ]]; then
        echo "ChangeLog.md 的 ## [${VER}] 章節是空的" >&2
    else
        echo "讀取 ChangeLog.md 失敗（狀態 ${status}）" >&2
    fi
    exit 1
}

printf '%s\n' "${NOTES}"
