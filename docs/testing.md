# Testing MadEdit locally

The default suite uses Rust's test harness and `egui_kittest`. It does not open a
native window. Test builds disable the user settings path and system clipboard.
No CI/CD configuration is required by these commands.

| Command | Scope |
| --- | --- |
| `make test` | All default Rust tests, with the committed dependency lock |
| `make test-core` | Core tests, including model sequences and encoding matrix |
| `make test-ui` | App tests, including egui keyboard, IME and mouse interactions |
| `make test-visual` | Four offscreen image comparisons; requires a wgpu adapter |
| `make test-large` | Explicit 1 GiB random-access/edit/save regression |
| `make test-tools` | Python benchmark validation and comparison tests |
| `make bench` | Release benchmarks with raw samples and environment metadata |

Use `CARGO=/path/to/cargo` or `PYTHON=python3` to override the tools. Rust needs
the platform dependencies documented in the README, including a C compiler for
Oniguruma. Python tooling uses only the standard library.

## Core regression coverage

The encoding matrix exercises UTF-8, both UTF-16 byte orders, Big5, GB18030,
Shift-JIS and Windows-1252, every supported BOM choice, and LF/CRLF/CR. It checks
decoded text and metadata, then edits and saves, undoes and saves again, comparing
exact bytes. Editing is essential: an unchanged document can reuse pristine bytes
and would not exercise the encoder.

The decode-error regression uses invalid UTF-8 that is valid Windows-1252. It
unconditionally checks the error state, save/conversion rejection, preservation
of the original file, and lossless reinterpretation and save.

Fixed-seed model tests compare Piece Table operations with `Vec<u8>` for both
in-memory and file-backed data, including page boundaries, slices, streaming and
search. Document sequences use a character-vector model containing CJK, emoji,
tabs and newlines; undo and redo must restore exact content and selection. Failure
messages identify the seed/step so the sequence can be reproduced without an
additional property-testing dependency.

The large-file test creates a sparse source of 1 GiB plus 17 bytes. It opens in
hex-only mode without a text copy, edits across a 64 KiB page boundary and at the
tail, and saves to another file. The saved file is checked in 1 MiB chunks,
including all zero-filled regions. Allow at least 2 GiB of free temporary space:
the output is not sparse and source allocation depends on the filesystem. Test
directories are removed on normal return or panic. The test is ignored by default
because it streams a full GiB; it makes no timing or peak-RSS claim.

## Visual baselines

`make test-visual` draws text, rectangular selection, hex and side-by-side Markdown
scenes at 1000 × 700 points, 1 pixel per point, English UI and a light theme. Font
chains use bundled fixtures and egui's built-in fonts, not installed system fonts.
Missing glyphs fail the test rather than being silently replaced. The harness
disables cursor blinking and animations. Font sources/licenses are in
[`tests/fixtures/README.md`](../tests/fixtures/README.md).

Baselines live in `tests/visual/<os>/`. The initial reviewed images are for macOS;
Linux and Windows require their own reviewed baselines because platform UI labels
can differ. A missing baseline fails; normal runs never approve new images.

For an intentional UI change:

```bash
MADEDIT_UPDATE_VISUAL=1 make test-visual
make test-visual
```

Review **all four** PNGs before committing. An update is not proof that a changed
image is correct. Ordinary comparisons allow at most two levels of difference per
RGBA channel and zero pixels above that tolerance. Actual, expected and red-on-black
diff images are retained in `target/visual/<os>/`. Size changes fail too, preserving
actual and expected images. The default suite separately verifies that a single
changed pixel is detected. These four scenes do not establish native-window, IME
candidate positioning, or full Unicode/font-fallback correctness.

The existing configurable screenshot command remains available for investigation:

```bash
MADEDIT_SHOT_OUT=target/shot.png MADEDIT_SHOT_FILE=README.md cargo test --locked shot -- --ignored
```

That command uses system fonts and is not the baseline comparison suite.

## Performance history and comparisons

`make bench` generates synthetic long-line JavaScript and Markdown/Mermaid inputs,
then runs the three ignored benchmarks serially in release mode. Each run keeps
its fixtures, Cargo log, raw `samples.jsonl`, and `report.json` in a unique directory
under `target/benchmarks/`. It never overwrites an earlier run or promotes a baseline.
GPU presentation and native window startup are outside these measurements.

To compare with a reviewed earlier run on the same machine:

```bash
make bench BENCH_ARGS='--baseline target/benchmarks/<earlier-run>/report.json'
```

The report records source revision/dirty state/hash, OS, CPU, architecture, Rust
version, lockfile hash, profile, fixture hashes, raw samples, median and p90. Both
statistics must stay within the greater of a 25% relative budget and a 1 ms noise
floor; configure these with `--max-regression` and `--noise-ms`. Regressions,
incomplete runs, malformed samples, changed sample counts, incompatible environments
and different fixtures fail with a nonzero exit status. Reports retain the failure
reason. Source revisions may differ, as comparing revisions is the purpose.

Compiler-flag environment inputs (`RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`,
`CARGO_BUILD_RUSTFLAGS`, and `CARGO_TARGET_*_RUSTFLAGS`) are recorded verbatim.
Unset and explicitly empty values remain distinct because Cargo can give these
different precedence. Older reports without this metadata are incompatible;
generate a new baseline rather than assuming the old flags were empty.

Use `--environment <label>` to identify font/power settings; change the label when
these or Cargo configuration files change. Run on the same physical machine,
with the same installed fonts and power settings, without competing workloads.
Metadata cannot prove identical runtime conditions. Inspect noisy results rather
than approving a weaker budget
automatically. Reports and logs remain local under the ignored `target/` directory.

`make bench BENCH_ARGS='--profile debug'` is useful for checking the tool itself;
debug results cannot be compared with release results. Individual benchmarks still
work through the environment variables documented beside their tests. Set
`MADEDIT_BENCH_JSONL` to additionally append machine-readable metrics, and use
`--test-threads=1` when collecting them together.

## macOS-specific validation

These checks remain separate from the default suite:

```bash
cargo test -p accesskit_macos --locked --test integration
./scripts/test_macos_bundle.sh
```

The AccessKit harness runs on the main thread, as AppKit requires. The bundle test
uses a tiny executable and an isolated install destination. Native desktop checks
and their limitations are described in [macOS validation](macos-validation.md).
