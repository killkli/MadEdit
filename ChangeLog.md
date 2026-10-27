# Changelog

This file is the source of user-visible changes. GitHub Release notes are taken directly from the matching `## [version]` section (see the release instructions in README).

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and version numbers follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.6.3] - 2026-10-27

### Added
- Single instance: launching again or opening files from File Explorer sends files to the existing window, opens them in tabs, and restores and focuses the window. Reopening a file selects its existing tab. Different configuration directories remain independent, and screenshot tests run separately.

### Changed
- Syntax highlighting for very long lines now tokenizes only the visible part of a line, uses the Oniguruma regex backend, and keeps each line's layout across frames. On a 1.99 MB sample whose longest line is 158 KB: keystroke 65 → 6 ms, End key 61 → 22 ms, PageDown per frame 119 → 1 ms, open plus first render 517 → 131 ms. The visible part at the end of a long line is now coloured too (only the first 16 KB used to be), and lines with full-width characters no longer crash while scrolling. Generate the sample with `python3 scripts/make_example_js.py`; see `docs/adr/0012-long-line-syntax-highlighting.md`.

### Fixed
- Markdown preview now opens in a separate tab in the main window instead of a separate native window. Reopening selects the existing preview, and closing it keeps the source document and unsaved changes intact. Side-by-side and top-and-bottom layouts remain available.
- Opening a nonexistent file now asks whether to create it, as Notepad does, instead of immediately reporting an open failure. If created, saving writes to the same path. The confirmation message is translated into all supported UI languages.

## [0.6.2] - 2026-10-27

### Changed

- The default encoding is now UTF-8. A new fallback encoding option (Big5 by default) is used only when the content is not valid UTF-8. The default encoding from legacy settings is automatically migrated to the fallback encoding.

## [0.6.1] - 2026-10-27

### Fixed
- Linux AppImage packaging failed because the temporary desktop file lacked the `.desktop` extension.

## [0.6.0] - 2026-10-27

### Added
- The UI language can follow the system or be set to English, Traditional Chinese, Simplified Chinese, Japanese, Korean, German, Spanish, French, Italian or Portuguese. The selection applies to menus and the Options dialog.
- `Makefile` provides unified development and packaging commands (`make check`, `make test`, `make dist`).
- Linux AppImage packages for x86_64 and aarch64, including the executable and spelling dictionaries.
- Linux archives and Windows zip packages can target `x86_64` or `aarch64`.
- GitHub Actions builds Linux archives, Linux AppImages, macOS DMGs and Windows zip packages when a `v*` tag is pushed, using the matching section in this file as release notes. Linux and Windows include x64 and ARM builds; macOS includes Apple Silicon and Intel builds.
- Markdown preview supports two-way synchronized scrolling, enabled by default in side-by-side and separate-window layouts. Preview UI text is translated.
- Markdown preview reuses unchanged Mermaid diagrams instead of converting them again when body text is edited.
- `README.md` is now in English; the Traditional Chinese version has moved to `README_zh-TW.md`.

### Fixed
- Markdown preview sizing in side-by-side layouts and loading of ordinary and reference-style images, including file names with CJK characters, spaces and percent-encoding.

## [0.5.24] - 2026-10-26

### Added
- macOS native integration: Finder "Open With" and dropping files on the Dock icon. Quitting from the system menu or Dock asks about unsaved changes first.
- macOS App packaging now verifies the signature before replacing the old version. Added `scripts/make_dmg.sh` to create disk images.

### Fixed
- Crash when quitting on Touch Bar Macs, while preserving accessibility support.
- Save As and Save a Copy now trim leading and trailing whitespace from file names, along with extra trailing periods and spaces.

## [0.5.23] - 2026-10-26

### Added
- In-app Markdown preview (`egui_commonmark`), with ` ```mermaid ` diagrams rendered by the pure-Rust fermaid library.
- Preview zoom, fit-to-width diagrams, relative-path images, side-by-side and top-and-bottom layouts, and synchronized scrolling with the editor in split layouts.
- Preview state is saved per tab; preview cannot be opened for non-Markdown files.
- In-app print preview with A4 pages, paging and zoom.
- Windows release archive script and one-step Linux installation to `~/.local`.

### Fixed
- Upgraded egui / eframe to 0.37.
- Text overlap caused by multiple Markdown tables sharing column widths.
- Black preview background in light mode; split layouts now give the preview and editor half the space each.
- Panic when deleting a multiline selection with word wrap enabled.
- Saving, Save a Copy and background replacement now use atomic writes. Corrupt settings files are backed up as `.bak`.
- Closing now resumes correctly when saving requires an additional confirmation.
- Shortcut capture no longer binds editing keys such as Enter and Backspace.

## [0.5.22] - 2026-10-23

### Added
- Linux release archives (cargo-zigbuild, glibc 2.28) and an installation script targeting `~/.local`.
- One-step macOS build and installation to `/Applications`.
- `Ctrl` / `⌘` + `1`–`9` tab switching.

### Fixed
- Startup failure with older Mesa software rendering.
- Linux icons are installed to `48x48`; the application can also be upgraded while running.

## [0.5.21] - 2026-10-23

### Added
- The window and toolbar now use the original MadEdit icons.

### Fixed
- Dialogs are centered when first opened.
- UI fonts retain CJK fallback when the editor uses a CJK font.

## [0.2.0] - 2026-10-23

### Added
- Rewrote the UI from Slint to egui / eframe, with custom-drawn text and hex views, scrollbars and ruler.
- User-selectable system fonts, including proportional fonts, automatic CJK fallback, and macOS input method preedit and candidate window positioning.
- Implemented 165 commands from the original command table: multi-document tabs, undo / redo, find and replace, column mode, bookmarks, encodings, macros, spelling, printing and options.
- Large files now open through a Piece Table with positioned reads instead of mmap.

## [0.1.0] - 2026-10-21

### Added
- Initial project. The Slint version implemented encoding support, a custom-drawn editor, a large-file engine, rectangular selection, find and replace, syntax highlighting, go-to-line, line endings, bookmarks, external change monitoring and the classic UI.

[Unreleased]: https://github.com/tenyi/MadEdit/compare/v0.6.3...HEAD
[0.6.3]: https://github.com/tenyi/MadEdit/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/tenyi/MadEdit/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/tenyi/MadEdit/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/tenyi/MadEdit/compare/v0.5.24...v0.6.0
[0.5.24]: https://github.com/tenyi/MadEdit/compare/v0.5.23...v0.5.24
[0.5.23]: https://github.com/tenyi/MadEdit/compare/v0.5.22...v0.5.23
[0.5.22]: https://github.com/tenyi/MadEdit/compare/v0.5.21...v0.5.22
[0.5.21]: https://github.com/tenyi/MadEdit/compare/v0.2.0...v0.5.21
[0.2.0]: https://github.com/tenyi/MadEdit/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/tenyi/MadEdit/releases/tag/v0.1.0
