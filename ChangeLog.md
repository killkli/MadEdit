# 變更紀錄

本檔是使用者可見變更的來源。GitHub Release 的說明直接取自對應的 `## [版本]` 章節（見 README 的發行步驟）。

格式依 [Keep a Changelog](https://keepachangelog.com/zh-TW/1.1.0/)，版本號遵循 [語意化版本](https://semver.org/lang/zh-TW/)。

## [Unreleased]

## [0.6.0] - 2026-10-27

### 新增
- 介面語言可跟隨系統，或改為 English、繁體中文、简体中文、日本語、한국어、Deutsch、Español、Français、Italiano、Português。選單與選項對話框都會套用。
- `Makefile` 統一開發與打包指令（`make check`、`make test`、`make dist`）。
- Linux AppImage（x86_64 與 aarch64），內含執行檔與拼字字典。
- Linux 壓縮檔與 Windows zip 可指定 `x86_64` 或 `aarch64`。
- GitHub Actions：推上 `v*` 標籤時建置 Linux 壓縮檔、Linux AppImage、macOS DMG 與 Windows zip，並以本檔對應章節作為發行說明。Linux 與 Windows 含 x64 與 ARM；macOS 含 Apple Silicon 與 Intel。
- Markdown 預覽支援雙向同步捲動，並排與獨立視窗預設開啟；預覽介面文字已翻譯。
- Markdown 預覽重用未修改的 Mermaid 圖表，編輯內文時不再重新轉換。
- `README.md` 改為英文，繁體中文版移至 `README_zh-TW.md`。

### 修正
- Markdown 預覽的並排尺寸、一般與參照式圖片（含中文、空白與百分比編碼檔名）載入。

## [0.5.24] - 2026-10-26

### 新增
- macOS 原生整合：Finder「打開方式」與拖曳到 Dock 開檔，系統選單與 Dock 的結束會先確認未儲存修改。
- macOS App 打包改為簽章驗證完成後才取代舊版，並提供 `scripts/make_dmg.sh` 產生磁碟映像檔。

### 修正
- Touch Bar Mac 結束時的崩潰（保留無障礙支援）。
- 另存新檔與另存副本時去除檔名前後空白，以及結尾多餘的句點與空白。

## [0.5.23] - 2026-10-26

### 新增
- App 內 Markdown 預覽（`egui_commonmark`），` ```mermaid ` 圖表由純 Rust 的 fermaid 繪製。
- 預覽可縮放、圖表符合寬度、相對路徑圖片、左右或上下並排，並在並排時跟隨編輯區捲動。
- 預覽狀態依分頁各自保存；不是 Markdown 的檔案無法開啟預覽。
- App 內預覽列印視窗（A4 分頁、翻頁、縮放）。
- Windows 發行壓縮檔腳本，以及 Linux 一鍵安裝到 `~/.local`。

### 修正
- 升級 egui / eframe 至 0.37。
- 多個 Markdown 表格共用欄寬導致文字重疊。
- 淺色模式預覽黑底；並排配置改為與編輯區各佔一半。
- 自動換行時刪除多行選取造成 panic。
- 存檔、另存副本與背景取代改為原子寫入；設定檔損毀時備份為 `.bak`。
- 關閉時若存檔還需確認，會接續完成關閉流程。
- 快捷鍵擷取不再綁走 Enter、Backspace 等編輯鍵。

## [0.5.22] - 2026-10-23

### 新增
- Linux 發行壓縮檔（cargo-zigbuild，glibc 2.28）與安裝到 `~/.local` 的腳本。
- macOS 一鍵建置並安裝到 `/Applications`。
- `Ctrl` / `⌘` + `1`～`9` 切換分頁。

### 修正
- 舊版 Mesa 軟體繪製下無法啟動。
- Linux 圖示安裝到 `48x48`，程式執行中也可以升級。

## [0.5.21] - 2026-10-23

### 新增
- 視窗與工具列改用原版 MadEdit 圖示。

### 修正
- 對話框第一次出現時置中。
- 編輯區改用中日韓字型時，介面字型仍保留 CJK 補字。

## [0.2.0] - 2026-10-23

### 新增
- 介面由 Slint 改寫為 egui / eframe，編輯區、十六進位檢視、捲軸與尺標改為自繪。
- 可選擇系統字型（含比例字型），CJK 自動補字，macOS 輸入法可預編輯並定位候選窗。
- 依原版命令表實作 165 個命令：多文件分頁、復原／重做、搜尋取代、列塊模式、書籤、編碼、巨集、拼字、列印與選項。
- 大檔改以定位讀取的 Piece Table 開啟，不再使用 mmap。

## [0.1.0] - 2026-10-21

### 新增
- 專案起步。Slint 時期完成編碼、自繪編輯區、大檔引擎、列塊選取、搜尋取代、語法突顯、跳行、換行格式、書籤、外部變更監控與經典介面。

[Unreleased]: https://github.com/tenyi/MadEdit/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/tenyi/MadEdit/compare/v0.5.24...v0.6.0
[0.5.24]: https://github.com/tenyi/MadEdit/compare/v0.5.23...v0.5.24
[0.5.23]: https://github.com/tenyi/MadEdit/compare/v0.5.22...v0.5.23
[0.5.22]: https://github.com/tenyi/MadEdit/compare/v0.5.21...v0.5.22
[0.5.21]: https://github.com/tenyi/MadEdit/compare/v0.2.0...v0.5.21
[0.2.0]: https://github.com/tenyi/MadEdit/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/tenyi/MadEdit/releases/tag/v0.1.0
