#!/usr/bin/env python3
"""Run local editor benchmarks, retain samples, and compare compatible baselines."""

from __future__ import annotations

import argparse
from collections.abc import Mapping
import datetime as dt
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
CASES = {
    *(f"long_line.{mode}.{case}" for mode in ("NoWrap", "ByWindow")
      for case in ("open", "keystroke", "end", "selection", "page_down", "click")),
    "markdown_ui.open", "markdown_ui.edit", "markdown_refresh.open", "markdown_refresh.edit",
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_hash() -> str:
    digest = hashlib.sha256()
    for path in [ROOT / "Cargo.toml", *sorted((ROOT / "src").rglob("*.rs"))]:
        digest.update(str(path.relative_to(ROOT)).encode())
        digest.update(b"\0")
        digest.update(path.read_bytes())
    return digest.hexdigest()


def rustflag_environment(environ: Mapping[str, str]) -> dict[str, str | None]:
    # Cargo can prefer encoded flags over RUSTFLAGS. Preserve unset versus empty:
    # an explicitly empty higher-priority value can suppress lower-priority flags.
    keys = {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS"}
    keys.update(key for key in environ if key.startswith("CARGO_TARGET_") and key.endswith("_RUSTFLAGS"))
    return {key: environ.get(key) for key in sorted(keys)}


def validate_metrics(metrics: list[dict]) -> dict[str, dict]:
    cases = {}
    for metric in metrics:
        name = metric["case"]
        if name in cases:
            raise ValueError(f"duplicate benchmark case: {name}")
        samples = metric["samples_ms"]
        if not samples or any(not isinstance(x, (int, float)) or not math.isfinite(x) or x < 0 for x in samples):
            raise ValueError(f"invalid samples: {name}")
        ordered = sorted(samples)
        for key, index in (("median_ms", len(samples) // 2), ("p90_ms", len(samples) * 9 // 10)):
            if metric[key] != ordered[index]:
                raise ValueError(f"invalid {key}: {name}")
        cases[name] = metric
    if set(cases) != CASES:
        raise ValueError(f"unexpected cases; missing={sorted(CASES - cases.keys())}, extra={sorted(cases.keys() - CASES)}")
    return cases


def compare(current: dict, baseline: dict, max_regression: float, noise_ms: float) -> list[str]:
    if current.get("schema") != 1 or baseline.get("schema") != 1:
        raise ValueError("unsupported report schema")
    # Revisions deliberately differ; environment and input bytes must agree.
    if current["environment"] != baseline["environment"]:
        raise ValueError("baseline environment differs (machine, OS, toolchain, dependencies or profile)")
    if current["fixtures"] != baseline["fixtures"]:
        raise ValueError("baseline fixture hashes differ")
    new = validate_metrics(current["metrics"])
    old = validate_metrics(baseline["metrics"])
    failures = []
    for case in sorted(CASES):
        if len(new[case]["samples_ms"]) != len(old[case]["samples_ms"]):
            raise ValueError(f"baseline sample count differs: {case}")
        for key in ("median_ms", "p90_ms"):
            before, after = old[case][key], new[case][key]
            if after - before > max(before * max_regression, noise_ms):
                failures.append(f"{case} {key}: {before:.3f} -> {after:.3f} ms")
    return failures


def cpu_model() -> str:
    if platform.system() == "Darwin":
        return subprocess.check_output(["sysctl", "-n", "machdep.cpu.brand_string"], text=True).strip()
    if platform.system() == "Linux":
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    return platform.processor() or platform.machine()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, help="previous report.json from the same environment")
    parser.add_argument("--out", type=Path, default=ROOT / "target/benchmarks")
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--profile", choices=("release", "debug"), default="release")
    parser.add_argument("--environment", default="local", help="label for fonts/power settings; change it when these change")
    parser.add_argument("--max-regression", type=float, default=0.25, help="relative budget (default: 0.25)")
    parser.add_argument("--noise-ms", type=float, default=1.0, help="absolute noise floor (default: 1 ms)")
    args = parser.parse_args()
    if any(not math.isfinite(x) or x < 0 for x in (args.max_regression, args.noise_ms)):
        parser.error("regression budget and noise floor must be finite and nonnegative")
    baseline = json.loads(args.baseline.read_text()) if args.baseline else None
    args.out.mkdir(parents=True, exist_ok=True)
    run = Path(tempfile.mkdtemp(prefix=dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ-"), dir=args.out.resolve()))
    fixture = run / "long-line.js"
    subprocess.run([sys.executable, str(ROOT / "scripts/make_example_js.py"), "--out", str(fixture)], check=True)
    markdown = run / "preview.md"
    markdown.write_text("# Benchmark\n\n" + "".join(
        f"## Section {i}\n\n中文 benchmark text {i}.\n\n```mermaid\ngraph LR\nA{i}-->B{i}\n```\n\n"
        for i in range(12)), encoding="utf-8")
    report = {
        "schema": 1,
        "created_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "source_sha256": source_hash(),
        "dirty": bool(subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"], cwd=ROOT, text=True)),
        "environment": {
            "os": platform.platform(), "arch": platform.machine(), "cpu": cpu_model(),
            "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
            "lock_sha256": sha256(ROOT / "Cargo.lock"), "profile": args.profile,
            "label": args.environment, "renderer": "egui software layout; no native window or GPU presentation",
            "rustflags": rustflag_environment(os.environ),
        },
        "fixtures": {"long_line": sha256(fixture), "markdown": sha256(markdown)},
        "metrics": [],
    }
    samples = run / "samples.jsonl"
    env = dict(os.environ, MADEDIT_BENCH_JSONL=str(samples),
               MADEDIT_LONGLINE_BENCH_FILE=str(fixture), MADEDIT_MD_BENCH_FILE=str(markdown),
               MADEDIT_CONFIG_DIR=str(run / "config"))
    command = [args.cargo, "test", "--locked"]
    if args.profile == "release":
        command.append("--release")
    command += ["benchmark_", "--", "--ignored", "--nocapture", "--test-threads=1"]
    print(f"Benchmark artifacts: {run}", flush=True)
    with (run / "cargo.log").open("w", encoding="utf-8") as log:
        result = subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
    report["cargo_exit_code"] = result.returncode
    failures = []
    try:
        if samples.exists():
            report["metrics"] = [json.loads(line) for line in samples.read_text().splitlines()]
        if result.returncode:
            raise ValueError(f"cargo failed ({result.returncode}); inspect {run / 'cargo.log'}")
        validate_metrics(report["metrics"])
        if baseline is not None:
            failures = compare(report, baseline, args.max_regression, args.noise_ms)
    except (ValueError, KeyError, TypeError) as error:
        failures = [str(error)]
    report["comparison"] = {"baseline": str(args.baseline) if args.baseline else None,
                            "max_regression": args.max_regression, "noise_ms": args.noise_ms,
                            "failures": failures}
    report["exit_code"] = 1 if failures else 0
    (run / "report.json").write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    for metric in report["metrics"]:
        print(f"{metric['case']}: median={metric['median_ms']:.3f} ms p90={metric['p90_ms']:.3f} ms")
    for failure in failures:
        print(f"FAIL: {failure}", file=sys.stderr)
    print(f"Report: {run / 'report.json'}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
