# Visual test font fixtures

These fonts are loaded only by the offscreen regression suite. They keep snapshots
independent of installed fonts and allow missing glyphs to be treated as failures.
They do not change the application's production font selection.

- `MadEditTestSans.otf`: a renamed subset of **Noto Sans CJK TC Regular 2.004**,
  from the [`Sans2.004` release](https://github.com/notofonts/noto-cjk/tree/Sans2.004),
  source path `Sans/OTF/TraditionalChinese/NotoSansCJKtc-Regular.otf`.
  It contains printable ASCII and the CJK fixture characters. Source SHA-256:
  `dce08bd4fd91aa8aa76ed8fea4b694c2dfb8550f67871e326843212ddbeb88b4`.
- `MadEditTestEmoji.ttf`: a renamed subset of `NotoEmoji-Regular.ttf` shipped with
  **epaint_default_fonts 0.37.0**, keeping `🌙☀🔒⚠✔`. Source SHA-256:
  `415dc6290378574135b64c808dc640c1df7531973290c4970c51fdeb849cb0c5`.
- Both modified Noto fonts retain their original copyright metadata and use the
  **SIL Open Font License 1.1**, included as `OFL.txt`. New family/PostScript names
  identify these as test subsets rather than the original fonts.
- `emoji-icon-font.ttf`: the unmodified font from **epaint_default_fonts 0.37.0**,
  licensed under the MIT license in `emoji-icon-font-MIT.txt`. It supplies egui
  window controls and symbols not covered by the small Noto subsets. SHA-256:
  `f426bec371f484646d002717e2c555a95cd5145e635d83a48bbef83f3abf18ca`.

To regenerate after changing the fixture character set, install `fonttools`
(generation used 4.61.1), download the pinned Noto CJK source, and locate the locked
`epaint_default_fonts-0.37.0/fonts` directory in the Cargo registry. Then run:

```bash
python3 scripts/make_test_fonts.py --cjk-source /path/to/NotoSansCJKtc-Regular.otf --egui-fonts /path/to/epaint_default_fonts-0.37.0/fonts
```

The generator validates source hashes, preserves licensing records, and renames
the modified fonts. Running tests requires neither FontTools nor a download.
Regenerate and review the four visual baselines when fixture fonts change.
