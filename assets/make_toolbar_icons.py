#!/usr/bin/env python3
"""將原版 MadEdit 的 XPM 圖示轉換為 PNG，供 Slint 工具列使用。

背景：
    原版 MadEdit（C++ / wxWidgets）以 XPM（X PixMap）格式存放工具列圖示。
    本專案沿用同一套圖示，讓重製版的工具列與原版外觀一致。

用法：
    python3 assets/make_toolbar_icons.py            # 產生 assets/icons/*.png
    python3 assets/make_toolbar_icons.py --scale 1  # 產生 1 倍尺寸（16x15）

設計說明：
    * 僅使用 Python 標準函式庫（zlib + struct）撰寫 PNG，避免額外相依套件。
    * 採最近鄰放大（nearest-neighbour），保持像素圖的銳利邊緣，不做模糊化處理。
"""

from __future__ import annotations

import argparse
import re
import struct
import sys
import zlib
from pathlib import Path

# XPM 少數會用到的具名顏色
NAMED_COLORS: dict[str, tuple[int, int, int, int]] = {
    "black": (0, 0, 0, 255),
    "white": (255, 255, 255, 255),
    "red": (255, 0, 0, 255),
    "green": (0, 255, 0, 255),
    "blue": (0, 0, 255, 255),
    "cyan": (0, 255, 255, 255),
    "magenta": (255, 0, 255, 255),
    "yellow": (255, 255, 0, 255),
    "gray": (128, 128, 128, 255),
    "grey": (128, 128, 128, 255),
    "none": (0, 0, 0, 0),
    "None": (0, 0, 0, 0),
}

STRINGS_RE = re.compile(r'"((?:[^"\\]|\\.)*)"')


def parse_color(token: str) -> tuple[int, int, int, int]:
    """把 XPM 的顏色字串轉成 RGBA 四元組。"""
    token = token.strip()
    if token.lower() == "none":
        return (0, 0, 0, 0)
    if token.startswith("#"):
        digits = token[1:]
        if len(digits) == 3:
            r, g, b = (int(c * 2, 16) for c in digits)
            return (r, g, b, 255)
        if len(digits) == 6:
            r, g, b = (int(digits[i : i + 2], 16) for i in (0, 2, 4))
            return (r, g, b, 255)
    named = NAMED_COLORS.get(token) or NAMED_COLORS.get(token.lower())
    if named:
        return named
    raise ValueError(f"不支援的顏色字串：{token}")


def parse_xpm(text: str) -> tuple[int, int, list[list[tuple[int, int, int, int]]]]:
    """解析 XPM 文字，回傳 (寬, 高, 像素矩陣)。"""
    strings = STRINGS_RE.findall(text)
    if not strings:
        raise ValueError("找不到 XPM 內容")

    header = strings[0].split()
    if len(header) < 4:
        raise ValueError(f"不支援的 XPM 標頭：{strings[0]}")
    width, height, color_count, chars_per_pixel = (int(v) for v in header[:4])

    palette: dict[str, tuple[int, int, int, int]] = {}
    for entry in strings[1 : 1 + color_count]:
        # 色票格式為「鍵值 空格 c 空格 顏色」，鍵值長度即 chars_per_pixel，
        # 可能是空白字元本身，因此必須依固定長度切出，不能用 split()。
        key = entry[:chars_per_pixel]
        rest = entry[chars_per_pixel:].strip().split()
        if len(rest) < 2:
            raise ValueError(f"不支援的 XPM 色票：{entry!r}")
        palette[key] = parse_color(rest[1])

    rows = strings[1 + color_count : 1 + color_count + height]
    if len(rows) != height:
        raise ValueError("XPM 像素列數與標頭不符")

    pixels: list[list[tuple[int, int, int, int]]] = []
    for row in rows:
        line: list[tuple[int, int, int, int]] = []
        for x in range(width):
            key = row[x * chars_per_pixel : (x + 1) * chars_per_pixel]
            if key not in palette:
                raise ValueError(f"像素使用未定義的色票：{key!r}")
            line.append(palette[key])
        pixels.append(line)
    return width, height, pixels


def scale_pixels(
    pixels: list[list[tuple[int, int, int, int]]], factor: int
) -> list[list[tuple[int, int, int, int]]]:
    """最近鄰放大，保持像素圖銳利。"""
    if factor == 1:
        return pixels
    scaled: list[list[tuple[int, int, int, int]]] = []
    for row in pixels:
        wide = [px for px in row for _ in range(factor)]
        for _ in range(factor):
            scaled.append(list(wide))
    return scaled


def write_png(path: Path, pixels: list[list[tuple[int, int, int, int]]]) -> None:
    """以標準函式庫寫出 RGBA8 PNG。"""
    height = len(pixels)
    width = len(pixels[0]) if height else 0

    raw = bytearray()
    for row in pixels:
        raw.append(0)  # 每列的過濾器型別：None
        for r, g, b, a in row:
            raw.extend((r, g, b, a))

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    png = bytearray(b"\x89PNG\r\n\x1a\n")
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    png += chunk(b"IEND", b"")
    path.write_bytes(bytes(png))


def main() -> int:
    parser = argparse.ArgumentParser(description="XPM → PNG 轉換器")
    parser.add_argument("--scale", type=int, default=2, help="放大倍數（預設 2）")
    parser.add_argument(
        "--src",
        type=Path,
        default=Path(__file__).resolve().parent / "icons-src",
        help="XPM 來源資料夾",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=Path(__file__).resolve().parent / "icons",
        help="PNG 輸出資料夾",
    )
    args = parser.parse_args()

    if args.scale < 1:
        print("放大倍數必須 >= 1", file=sys.stderr)
        return 1

    args.out.mkdir(parents=True, exist_ok=True)
    sources = sorted(args.src.glob("*.xpm"))
    if not sources:
        print(f"在 {args.src} 找不到任何 XPM 檔案", file=sys.stderr)
        return 1

    for src in sources:
        width, height, pixels = parse_xpm(src.read_text(encoding="utf-8"))
        scaled = scale_pixels(pixels, args.scale)
        out_path = args.out / f"{src.stem}.png"
        write_png(out_path, scaled)
        print(f"{src.name} ({width}x{height}) → {out_path.name} ({width * args.scale}x{height * args.scale})")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
