#!/usr/bin/env python3
"""Generate a synthetic minified-JavaScript sample with very long lines.

The file exists only to measure and demonstrate long-line behaviour (open a file,
typing, horizontal scrolling, caret moves). It is generated from this script, so
it contains no third-party or private code and it is reproducible:

    python3 scripts/make_example_js.py                  # -> example.js (~1.9 MB, 180 lines)
    python3 scripts/make_example_js.py --out /tmp/big.js --lines 60 --bytes 500000

Shape of the default output mirrors the case that motivated the windowed
highlighting work: ~1.99 MB in 180 lines, the longest line 158,953 bytes.
Part of a long line is a Chinese string literal (no `;`, `{` or `}` for tens of
kilobytes), which is what makes byte-aligned parse windows land inside a
multi-byte character — see docs/adr/0012-long-line-syntax-highlighting.md.

Usage as a benchmark input:

    MADEDIT_LONGLINE_BENCH_FILE=example.js cargo test --release benchmark_long_line_ -- --ignored --nocapture
"""

from __future__ import annotations

import argparse
import random

# Deterministic filler: a sentence repeated to an exact length.
CJK_SENTENCE = "資料載入完成，請稍候。檔案格式不支援，請改用 UTF-8 編碼。這個欄位必填。欄位長度超出上限。"

MESSAGES = [
    "loading",
    "unsupported-format",
    "required",
    "too-long",
    "unknown-error",
    "saved",
]

IDENTIFIERS = "etnrioaulscfdpbhmvgwyxjzkq"
MARKERS = ["", "!", "?"]


def cjk_text(length: int) -> str:
    """Deterministic Chinese text of exactly `length` characters."""
    repeats = length // len(CJK_SENTENCE) + 1
    return (CJK_SENTENCE * repeats)[:length]


def statement(rng: random.Random) -> str:
    """One synthetic minified statement (ASCII, `;`/`{}` dense)."""
    a, b, c, d = (rng.choice(IDENTIFIERS) + str(rng.randrange(1000)) for _ in range(4))
    msg = MESSAGES[rng.randrange(len(MESSAGES))]
    return rng.choice(
        [
            f'var {a}=Object.create({b});',
            f'{a}={c}.length+{rng.randrange(99)};',
            f'if({a}&&typeof {b}=="object"){{{c}.push({d});}}',
            f'function {a}(e,t){{return e?t(e):e}}',
            f'try{{{a}=JSON.parse({b})}}catch(e){{{a}=null}}',
            f'switch(typeof {a}){{case"string":{c}+={a};break;case"number":{c}*={a};break;}}',
            f'{a}=[{b},{c},{d}].map(function(e){{return e+1}});',
            f't.{a}="{msg}";',
        ]
    )


def module_line(index: int, budget: int, rng: random.Random, series: bool) -> str:
    """A minified module of exactly `budget` bytes.

    `series` lines carry a tens-of-kilobytes Chinese string literal: it contains
    no `;`/`{`/`}` and is the region that makes byte-aligned parse windows slice
    a multi-byte character.
    """
    head = f"var m{index}=((e,t)=>{{"
    tail = "return t}" + f")(m{max(index - 1, 0)}||{{}},{{}});"
    body = [head]
    used = len(head) + len(tail)
    if series:
        # 3 bytes per Chinese character in UTF-8
        chars = max(0, min((budget - used - 32) // 3, max(2000, budget // 12)))
        if chars:
            block = f't.title="{cjk_text(chars)}";'
            body.append(block)
            used += len(block.encode())
    while True:
        stmt = statement(rng)
        if used + len(stmt) > budget - 16:
            break
        body.append(stmt)
        used += len(stmt)
    # 補到剛好的位元組數（同步產生一段沒有 `;` 的字串，順便擴大長字串區段）
    pad = budget - used - 7  # 扣掉固定開銷：t.z=" 與 "; 共 7 位元組
    if pad > 0:
        body.append(f't.z="{"x" * pad}";')
        used += pad + 7
    body.append(tail)
    line = "".join(body)
    assert len(line.encode()) == budget, f"line {index}: {len(line.encode())} != {budget}"
    return line


def line_sizes(total: int, lines: int, longest: int, rng: random.Random) -> list[int]:
    """Per-line byte budget: a few very long lines, then medium and short ones."""
    if lines < 8:
        raise SystemExit("--lines must be at least 8")
    if longest > total // 2:
        raise SystemExit("--longest must stay below half of --bytes")

    sizes = [0] * lines
    sizes[3] = longest
    sizes[lines // 6] = longest * 3 // 4
    sizes[lines // 3] = longest // 2
    remaining = total - sum(sizes) - lines  # one newline per line
    if remaining < 0:
        raise SystemExit("--bytes is too small for the requested long lines")

    # Medium and short lines: exponential weights so most lines stay short.
    weights = [(rng.random() ** 3) or 1e-9 for _ in range(lines - 3)]
    scale = remaining / sum(weights)
    for slot, weight in zip((i for i in range(lines) if sizes[i] == 0), weights):
        sizes[slot] = max(64, int(weight * scale))
    # 把四捨五入的差額補到一條中型行，總位元組數才會剛好等於 --bytes
    diff = total - lines - sum(sizes)
    fix = max((i for i in range(lines) if sizes[i] < longest // 2), key=lambda i: sizes[i])
    sizes[fix] += diff
    return sizes


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out", default="example.js", help="output path (default: example.js)")
    parser.add_argument("--lines", type=int, default=180, help="number of lines (default: 180)")
    parser.add_argument("--bytes", type=int, default=1_993_854, help="total size in bytes (default: 1993854)")
    parser.add_argument("--longest", type=int, default=158_953, help="size of the longest line (default: 158953)")
    parser.add_argument("--seed", type=int, default=20_261_027, help="random seed (default: 20261027)")
    args = parser.parse_args()

    rng = random.Random(args.seed)
    sizes = line_sizes(args.bytes, args.lines, args.longest, rng)
    # 每 5 行有一段長中文字串；最長的那一行一定有
    series = {3} | {i for i in range(args.lines) if i % 5 == 1}
    lines = [module_line(i, size, rng, i in series) for i, size in enumerate(sizes)]
    data = "\n".join(lines) + "\n"
    with open(args.out, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(data)

    longest = max(len(line.encode()) for line in lines)
    print(f"{args.out}: {len(data.encode())} bytes, {len(lines)} lines, longest line {longest} bytes")


if __name__ == "__main__":
    main()
