#!/usr/bin/env python3
"""在登入中的 macOS 桌面驗證正式 App 繪製與正常退出，保留截圖及執行記錄。"""

import argparse
import pathlib
import platform
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    root = pathlib.Path(__file__).resolve().parent.parent
    parser.add_argument("--app", type=pathlib.Path, default=root / "dist/MadEdit.app")
    parser.add_argument("--out", type=pathlib.Path, default=root / "dist/validation/runtime")
    args = parser.parse_args()
    if platform.system() != "Darwin":
        parser.error("此測試需要 macOS 與可用的桌面工作階段")
    binary = args.app.resolve() / "Contents/MacOS/MadEdit"
    if not binary.is_file():
        parser.error("找不到 App；請先執行 ./scripts/make_appbundle.sh")
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    text = out / "中文 空白 %25.txt"
    text.write_text("macOS 原生視窗測試\nHello 世界 日本語\n  第一欄\t第二欄\n", encoding="utf-8")
    binary_file = out / "binary.bin"
    binary_file.write_bytes(bytes(range(256)) * 8)
    # 連續啟動／退出覆蓋 AppKit 與無障礙轉接器的清理；--screenshot 不讀寫使用者設定。
    for name, document in [("text", text), ("hex", binary_file), ("text-repeat", text)]:
        screenshot = out / f"{name}.png"
        screenshot.unlink(missing_ok=True)
        with (out / f"{name}.log").open("w") as log:
            result = subprocess.run(
                [str(binary), str(document), "--screenshot", str(screenshot)],
                stdout=log, stderr=subprocess.STDOUT, timeout=45,
            )
        if result.returncode != 0:
            raise RuntimeError(f"{name} 執行失敗（{result.returncode}），請檢查 {out / (name + '.log')}")
        if not screenshot.is_file() or screenshot.read_bytes()[:8] != b"\x89PNG\r\n\x1a\n":
            raise RuntimeError(f"{name} 未產生有效 PNG 截圖")
        print(f"{name}：繪製與正常退出通過；截圖 {screenshot}")


if __name__ == "__main__":
    main()
