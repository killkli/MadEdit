#!/usr/bin/env python3
"""MadEdit 應用程式圖示產生器。

以 `assets/madedit-icon.svg` 為唯一來源，產生：
  - assets/madedit-icon.png : Slint 視窗圖示（@image-url 內嵌使用）
  - assets/madedit-icon.ico : Windows 執行檔圖示資源（Explorer / 工作列）

ICO 以 PNG 壓縮格式封裝各尺寸，比傳統 BMP 封裝小非常多（約 25KB vs 300KB），
Windows Vista 以上原生支援此格式。

用法：python3 assets/make_icons.py
"""

from __future__ import annotations

import struct
import subprocess
import sys
from pathlib import Path

ASSETS_DIR = Path(__file__).resolve().parent
SVG_SOURCE = ASSETS_DIR / "madedit-icon.svg"
PNG_OUTPUT = ASSETS_DIR / "madedit-icon.png"
ICO_OUTPUT = ASSETS_DIR / "madedit-icon.ico"

# PNG 主檔尺寸（供 Slint 視窗圖示使用，兼作 ICO 最大尺寸來源）
MASTER_SIZE = 512

# ICO 內嵌的各個尺寸
ICO_SIZES = (256, 128, 64, 48, 32, 16)


def render_png(size: int) -> bytes:
    """呼叫 ImageMagick 將 SVG 轉成指定尺寸的 PNG 位元組。"""
    result = subprocess.run(
        [
            "magick",
            "-background",
            "none",
            str(SVG_SOURCE),
            "-resize",
            f"{size}x{size}",
            "PNG32:-",
        ],
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        sys.stderr.write(result.stderr.decode("utf-8", "replace"))
        raise SystemExit(f"無法將 {SVG_SOURCE} 轉換為 PNG")
    return result.stdout


def build_ico(images: list[bytes]) -> bytes:
    """將多個 PNG 封裝成單一 ICO 檔（PNG 壓縮格式）。"""
    # ICONDIR：保留欄位、型別(1=圖示)、圖示數量
    header = struct.pack("<HHH", 0, 1, len(images))

    # ICONDIRENTRY：每筆 16 bytes，記錄該尺寸圖示的位移與長度
    entries = b""
    offset = len(header) + 16 * len(images)
    for size, data in zip(ICO_SIZES, images):
        entries += struct.pack(
            "<BBBBHHII",
            0 if size >= 256 else size,  # 寬度（0 代表 256）
            0 if size >= 256 else size,  # 高度（0 代表 256）
            0,  # 色彩數量（0 代表 truecolor）
            0,  # 保留
            1,  # 色彩平面數
            32,  # 每像素位元數
            len(data),
            offset,
        )
        offset += len(data)

    return header + entries + b"".join(images)


def main() -> None:
    if not SVG_SOURCE.exists():
        raise SystemExit(f"找不到圖示來源檔：{SVG_SOURCE}")

    # 產生 Slint 視窗使用的主 PNG
    PNG_OUTPUT.write_bytes(render_png(MASTER_SIZE))
    print(f"已產生 {PNG_OUTPUT.name}（{MASTER_SIZE}x{MASTER_SIZE}）")

    # 產生多尺寸 ICO
    ico = build_ico([render_png(size) for size in ICO_SIZES])
    ICO_OUTPUT.write_bytes(ico)
    print(f"已產生 {ICO_OUTPUT.name}（{len(ico) / 1024:.1f} KB，尺寸：{ICO_SIZES}）")


if __name__ == "__main__":
    main()
