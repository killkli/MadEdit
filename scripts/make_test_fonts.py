#!/usr/bin/env python3
"""Regenerate pinned visual-test fonts (optional developer dependency: fonttools)."""

import argparse
import hashlib
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parent.parent


def verify(path, expected):
    if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
        raise ValueError(f"unexpected source font: {path}")


def subset_font(source, destination, name, unicodes):
    font = TTFont(source, recalcTimestamp=False)
    options = subset.Options()
    options.recalc_timestamp = False
    worker = subset.Subsetter(options=options)
    worker.populate(unicodes=unicodes)
    worker.subset(font)
    # Modified OFL fonts use new names; original copyright/license records stay intact.
    for record in font["name"].names:
        if record.nameID in (1, 4, 6, 16):
            value = name.replace(" ", "") if record.nameID == 6 else name
            record.string = value.encode(record.getEncoding())
    if "CFF " in font:
        font["CFF "].cff.fontNames = [name.replace(" ", "")]
        for top in font["CFF "].cff.topDictIndex:
            top.FullName = name
            top.FamilyName = name
    font.save(destination)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cjk-source", type=Path, required=True)
    parser.add_argument("--egui-fonts", type=Path, required=True, help="epaint_default_fonts 0.37.0 fonts directory")
    args = parser.parse_args()
    emoji = args.egui_fonts / "NotoEmoji-Regular.ttf"
    icons = args.egui_fonts / "emoji-icon-font.ttf"
    verify(args.cjk_source, "dce08bd4fd91aa8aa76ed8fea4b694c2dfb8550f67871e326843212ddbeb88b4")
    verify(emoji, "415dc6290378574135b64c808dc640c1df7531973290c4970c51fdeb849cb0c5")
    verify(icons, "f426bec371f484646d002717e2c555a95cd5145e635d83a48bbef83f3abf18ca")
    out = ROOT / "tests/fixtures"
    subset_font(args.cjk_source, out / "MadEditTestSans.otf", "MadEdit Test Sans",
                list(range(32, 127)) + list(map(ord, "中文測試資料第一二行日本語新檔案未命名")))
    subset_font(emoji, out / "MadEditTestEmoji.ttf", "MadEdit Test Emoji", list(map(ord, "🌙☀🔒⚠✔")))
    (out / "emoji-icon-font.ttf").write_bytes(icons.read_bytes())
    (out / "emoji-icon-font-MIT.txt").write_bytes((args.egui_fonts / "emoji-icon-font-mit-license.txt").read_bytes())


if __name__ == "__main__":
    main()
