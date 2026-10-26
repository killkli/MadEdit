# MadEdit 建置與發行。
# 開發用 check / test / run / build；打包用 dist-*。發行步驟見 README.md。

CARGO ?= cargo
DIST ?= dist
VERSION := $(shell sed -n 's/^version *= *"\([^"]*\)".*/\1/p' Cargo.toml | head -n 1)
HOST_ARCH := $(shell uname -m | sed -e 's/amd64/x86_64/' -e 's/arm64/aarch64/' -e 's/x64/x86_64/')
ARCH ?= $(HOST_ARCH)

.PHONY: help check test run build dist dist-linux dist-appimage dist-macos dist-windows release-notes clean

.DEFAULT_GOAL := help

help:
	@echo "MadEdit $(VERSION)"
	@echo ""
	@echo "開發："
	@echo "  make check              cargo check"
	@echo "  make test               cargo test"
	@echo "  make run                cargo run"
	@echo "  make build              cargo build --release --locked"
	@echo ""
	@echo "打包（ARCH 預設為本機 $(HOST_ARCH)；可設 x86_64 或 aarch64）："
	@echo "  make dist-linux         Linux tar.gz"
	@echo "  make dist-appimage      Linux AppImage（含同架構 tar.gz）"
	@echo "  make dist-macos         macOS .app 與 .dmg"
	@echo "  make dist-windows       Windows zip（在 Windows 上執行）"
	@echo "  make dist               依目前作業系統打包"
	@echo "  make release-notes      印出 ChangeLog.md 裡 VERSION 的發行說明"
	@echo "  make clean              清除 target 與 dist"
	@echo ""
	@echo "正式發行：把 ChangeLog.md 的 [Unreleased] 移到新版本、升版號、"
	@echo "推上 vX.Y.Z 標籤。GitHub Actions 會建置並以該章節作為發行說明。"

check:
	$(CARGO) check

test:
	$(CARGO) test

run:
	$(CARGO) run

build:
	$(CARGO) build --release --locked

dist-linux:
	./scripts/make_linux_dist.sh $(DIST) $(ARCH)

dist-appimage:
	./scripts/make_appimage.sh $(DIST) $(ARCH)

dist-macos:
	./scripts/make_appbundle.sh $(DIST)
	./scripts/make_dmg.sh $(DIST)

dist-windows:
	powershell -ExecutionPolicy Bypass -File ./scripts/make_windows_dist.ps1 -OutDir $(DIST) -Arch $(ARCH)

dist:
	@case "$$(uname -s)" in \
		Linux) $(MAKE) dist-appimage ;; \
		Darwin) $(MAKE) dist-macos ;; \
		MINGW*|MSYS*|CYGWIN*) $(MAKE) dist-windows ;; \
		*) echo "不支援的系統：$$(uname -s)" >&2; exit 1 ;; \
	esac

release-notes:
	./scripts/changelog_notes.sh $(VERSION)

clean:
	$(CARGO) clean
	rm -rf $(DIST)
