#!/usr/bin/env bash
#
# 下載固定版本的 dprint 執行檔與 Wasm 插件（「帶 DPRINT」發行版用）。
#
# 版本、網址與 SHA-256 一律取自 assets/formatters/manifest.json；
# 已存在且雜湊正確的檔案不會重新下載，可在 CI 快取目錄上重複執行。
#
# 用法：
#   ./scripts/fetch_formatter.sh [輸出目錄] [--target <rust-triple>]
#
# 輸出結構（預設 target/formatters，與 src/core/dprint.rs 的查找位置一致）：
#   dprint            執行檔（可執行）
#   dprint.json       設定檔（複製自 assets/formatters/dprint.json）
#   plugins/*.wasm    固定版本的 Wasm 插件
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="${ROOT}/assets/formatters/manifest.json"

OUT_DIR=""
TARGET=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --target)
            TARGET="${2:?--target 需要一個 triple}"
            shift 2
            ;;
        *)
            if [[ -z "${OUT_DIR}" ]]; then OUT_DIR="$1"; else
                echo "多餘的參數：$1" >&2; exit 1
            fi
            shift
            ;;
    esac
done
OUT_DIR="${OUT_DIR:-${ROOT}/target/formatters}"

if [[ -z "${TARGET}" ]]; then
    case "$(uname -s)" in
        Darwin)  OS="apple-darwin" ;;
        Linux)   OS="unknown-linux-gnu" ;;
        MINGW*|MSYS*|CYGWIN*) OS="pc-windows-msvc" ;;
        *) echo "不支援的系統：$(uname -s)" >&2; exit 1 ;;
    esac
    HOST_ARCH="$(uname -m | sed -e 's/amd64/x86_64/' -e 's/arm64/aarch64/' -e 's/x64/x86_64/')"
    TARGET="${HOST_ARCH}-${OS}"
fi

# 下載與驗雜湊、解壓都用 python3 標準庫（Makefile 已依賴 python3）
python3 - "${MANIFEST}" "${TARGET}" "${OUT_DIR}" <<'PY'
import hashlib
import pathlib
import shutil
import stat
import sys
import tempfile
import urllib.error
import urllib.request
import zipfile

manifest_path, target, out_dir = pathlib.Path(sys.argv[1]), sys.argv[2], pathlib.Path(sys.argv[3])
manifest = __import__("json").loads(manifest_path.read_text(encoding="utf-8"))

def sha256(path: pathlib.Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()

def fetch(url: str, expected: str, dest: pathlib.Path) -> None:
    if dest.is_file() and sha256(dest) == expected:
        print(f"  已存在且驗證通過：{dest.name}")
        return
    print(f"  下載 {url}")
    # plugins.dprint.dev（Cloudflare）會擋掉預設的 python-urllib User-Agent
    req = urllib.request.Request(url, headers={"User-Agent": "madedit-fetch/1.0 (+https://github.com/tenyi/MadEdit)"})
    tmp_path = None
    last_error = None
    for _ in range(3):
        try:
            with urllib.request.urlopen(req, timeout=60) as resp, tempfile.NamedTemporaryFile(delete=False) as tmp:
                shutil.copyfileobj(resp, tmp)
                tmp_path = pathlib.Path(tmp.name)
            break
        except urllib.error.URLError as e:
            last_error = e
            if tmp_path is not None:
                tmp_path.unlink(missing_ok=True)
                tmp_path = None
    if tmp_path is None:
        raise SystemExit(f"下載失敗：{url}（{last_error}）")
    if sha256(tmp_path) != expected:
        tmp_path.unlink(missing_ok=True)
        raise SystemExit(f"SHA-256 不符：{url}")
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.move(str(tmp_path), dest)

out_dir.mkdir(parents=True, exist_ok=True)

# --- dprint 執行檔 ----------------------------------------------------------
entry = manifest["dprint"]["targets"].get(target)
if entry is None:
    raise SystemExit(f"manifest.json 沒有 {target} 的 dprint；請確認 --target 拼寫。")
print(f"dprint {manifest['dprint']['version']}（{target}）")
binary_name = "dprint.exe" if target.endswith("windows") else "dprint"
binary = out_dir / binary_name
# sidecar 記下壓縮檔雜湊；一致時連 dprint 本體都不用重抓
sidecar = out_dir / ".dprint-zip-sha256"
if binary.is_file() and sidecar.is_file() and sidecar.read_text().strip() == entry["sha256"]:
    print("  執行檔已存在且驗證通過：dprint")
else:
    with tempfile.TemporaryDirectory() as tmp:
        zip_path = pathlib.Path(tmp) / "dprint.zip"
        fetch(entry["url"], entry["sha256"], zip_path)
        with zipfile.ZipFile(zip_path) as z:
            members = [n for n in z.namelist() if pathlib.PurePosixPath(n).name == binary_name]
            if not members:
                raise SystemExit(f"壓縮檔內找不到 {binary_name}：{z.namelist()}")
            data = z.read(members[0])
    binary.write_bytes(data)
    binary.chmod(binary.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    sidecar.write_text(entry["sha256"] + "\n")

# --- Wasm 插件 ---------------------------------------------------------------
plugins = out_dir / "plugins"
plugins.mkdir(exist_ok=True)
for name, info in manifest["plugins"].items():
    print(f"插件 {name} {info['version']}")
    fetch(info["url"], info["sha256"], plugins / f"{name}.wasm")

# 清掉 manifest 以外的殘留插件，避免舊檔被一起打包
keep = {f"{name}.wasm" for name in manifest["plugins"]}
for stale in plugins.glob("*.wasm"):
    if stale.name not in keep:
        print(f"  移除非 manifest 插件：{stale.name}")
        stale.unlink()

# --- 設定檔（跟著 repo 走，永遠覆寫以保持同步） --------------------------------
shutil.copyfile(manifest_path.parent / "dprint.json", out_dir / "dprint.json")
print(f"完成：{out_dir}")
PY
