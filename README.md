# MadEdit Rust 重製版

**以現代 Rust 與 egui 重新打造的跨平台、輕量、極速文字 / 列塊 / 十六進位編輯器**  
*A modern, lightweight, cross-platform Text / Column / Hex editor inspired by classic MadEdit, written in pure Rust.*

---

## 📖 專案背景與願景

原版 C++ / wxWidgets 架構的 **MadEdit** 是許多開發者與資料處理人員心目中的經典工具（支援列塊編輯、Hex 檢視與多國語系編碼切換），但隨著時間推移，龐雜的 C++ 跨平台依賴與編譯鏈在現代 Windows、macOS 與 Linux 環境下日益難以維護。在下原本也由 MadEdit-Mod 的原始碼重新建置出 Mac 版本與 Linux 版本，但是速度慢，有許多功能在非Windows難以正常實現，尤其是字型顯示，修改後各平臺不一致，於是才使用 Rust 重製。

**MadEdit-Rust** 以 **Rust (Edition 2024)** 與即時模式 GUI 框架 **[egui / eframe](https://github.com/emilk/egui)** 重新打造，遵循 **KISS (Keep It Simple, Stupid)** 設計原則，在零 C++ GUI 系統庫依賴的前提下，重現原版 MadEdit 的經典介面與命令集（依原版命令表實作 165 個命令）。

> 介面曾以 Slint 實作，2026-10 改寫為 egui，以支援使用者任選系統字型（含比例字型）與更完整的輸入法處理，詳見 [ADR 0011](docs/adr/0011-egui-rewrite.md)。

---

## ✨ 核心功能特色

### 1. 三大編輯與檢視模式
- **文字模式 (Text Mode)**：一般串流文字與程式碼編輯，支援智慧縮排行首 (`Home`)、整行上下移動、複製行與刪除行。
- **列塊模式 (Column / Block Mode)**：
  - 以視覺欄位（半形 1 欄、CJK 全形 2 欄、Tab 對齊）進行二維矩形選取。
  - 支援工具列切換或按住 `Alt + 滑鼠拖曳` / `Alt + Shift + 方向鍵` 進行矩形選取。
  - 支援**矩形區塊複製、剪下、列塊對齊貼上、多行同步打字輸入與區塊切除**（短行自動補齊空白）。
- **十六進位模式 (Hex Mode)**：
  - 經典三欄對照檢視：**位址偏移 (Offset)**、**十六進位位元組 (Hex Bytes)**、**ASCII 可視預覽**。
  - 支援十六進位位移跳轉（如 `0x1A40`）與 Hex Byte Pattern 搜尋（如 `4D 5A 90`）。

### 2. 字型與輸入法
- 編輯區字型可任選系統字型與字級（含比例字型），CJK 字元自動以系統中日韓字型補字。
- 支援 macOS 輸入法（注音、倉頡等）預編輯顯示與候選窗定位。

### 3. 多語系編碼支援與防損毀保護
- 整合 `encoding_rs` 與自訂 UTF-16 編解碼器，支援 **UTF-8、UTF-8 BOM、UTF-16LE、UTF-16BE、Big5、GB18030、Shift_JIS、EUC-KR、windows-1252、ISO-8859-1** 等編碼。
- **智慧開檔偵測**：優先識別 BOM，無 BOM 時驗證嚴格 UTF-8，失敗則自動回退至 Big5。
- **無損防呆機制**：檔案含有無法無損解碼的位元組時，狀態列警告並拒絕覆寫存檔，提供**「重新解碼」**與**「轉換編碼」**。

### 4. 大檔案引擎
- **Piece Table** 以定位讀取（pread）分頁快取原始檔案，不使用 mmap（避免檔案被外部截短時發生 SIGBUS）。
- 超過門檻的大型檔案以十六進位模式開啟，資料留在磁碟上按需讀取。

### 5. 原版 MadEdit 命令集
- **檔案**：多文件分頁、另存副本、全部儲存、最近檔案、唯讀旗標、列印與預覽、工作階段保存。
- **編輯**：書籤行操作、Hex 字串互轉、縮排、註解、大小寫、全半形、Tab 與空白互轉、遞增數字、排序、欄位對齊。
- **搜尋**：純文字 / 大小寫 / 全字 / Regex / Hex 搜尋取代、在多個檔案中尋找／取代、跳至對應括號。
- **檢視**：自動換行、行號、書籤欄、尺標、控制字元、目前行、欄位提示線、深淺色主題、Markdown／HTML 預覽。
- **巨集與工具**：巨集錄製播放、快捷鍵重新指派、拼字檢查、字數統計、簡繁與日文漢字轉換、JSON／XML 格式化。

---

## 🏗️ 專案目錄結構

```text
MadEdit-Rust/
├── Cargo.toml / Cargo.lock
├── build.rs                # Windows 執行檔圖示嵌入
├── assets/                 # 應用程式圖示、工具列圖示（取自原版）、拼字字典
├── scripts/
│   ├── make_appbundle.sh   # 產生 macOS MadEdit.app
│   └── make_linux_dist.sh  # 產生 Linux 發行壓縮檔
├── src/
│   ├── main.rs             # 進入點：解析命令列、建立視窗、啟動 eframe
│   ├── app/                # egui 介面層（選單、工具列、自繪編輯區、Hex 檢視、對話框、命令表）
│   └── core/               # 與 UI 無關的核心邏輯（文件、編碼、Piece Table、搜尋、語法、設定）
├── docs/                   # 架構決策紀錄 (ADR) 與領域文件
├── GLOSSARY.md             # 專案領域詞彙表
└── AGENTS.md               # AI 協作與開發指南
```

---

## 🚀 建置與執行指南

### 環境需求
- **Rust 工具鏈**：Rust `1.95` 或以上版本（egui / eframe 0.37 的最低需求）
- 支援平台：**Windows**、**macOS**、**Linux**

### 開發與測試指令

```bash
cargo check            # 檢查語法與型別
cargo test             # 單元測試與 egui_kittest 介面互動測試
cargo run              # 啟動開發版
cargo build --release  # 編譯最佳化正式版
```

編譯完成後，可於 `target/release/MadEdit`（Windows 為 `target/release/MadEdit.exe`）取得獨立執行檔。

### macOS 建置

macOS 上的 GUI 程式需放在 `.app` bundle 內，才能正確顯示 Dock 圖示、選單列名稱並與系統互動。

1. 安裝 Xcode Command Line Tools（提供 `sips`、`iconutil`、`codesign`）：
   ```bash
   xcode-select --install
   ```
2. 編譯並產生 App：
   ```bash
   ./scripts/make_appbundle.sh
   ```
3. 產物位於 `dist/MadEdit.app`（已含圖示與拼字字典，並以 ad-hoc 簽章），可直接開啟或拖進「應用程式」資料夾：
   ```bash
   open dist/MadEdit.app
   ```

腳本會自動建置本機架構（Apple Silicon 或 Intel），支援 Cargo 自訂的 target-dir／build.target。
如已建好 universal binary，可用 `MADEDIT_BIN=/path/to/MadEdit ./scripts/make_appbundle.sh` 打包。
App 內含圖示與字典；打包、簽章及驗證全部完成後才取代舊版。

安裝到 `/Applications`：`./scripts/install_macos.sh`；也可用
`DEST="$HOME/Applications/MadEdit.app" ./scripts/install_macos.sh` 指定安裝位置。

支援 Finder「打開方式」、拖曳檔案至 Dock 圖示，以及
`open -a "$PWD/dist/MadEdit.app" 檔案.txt`。已啟動時會在同一個 App 開啟新分頁。
常用快捷鍵為 ⌘W 關閉分頁、⌘Q 結束（未儲存時先確認）、⌘⇧S 另存新檔、⌘⌥F 取代、
⌘/ 註解、Control+Tab 切換分頁；Option+方向鍵移動字詞、⌘+方向鍵移到行首尾／文件頭尾。
⌘H、⌘M、⌘Tab 保留給系統。

macOS 打包回歸測試：`./scripts/test_macos_bundle.sh`（不覆寫已安裝的 App）。
在已登入的 macOS 桌面執行 `python3 scripts/test_macos_runtime.py`，會連續啟動正式 App，
驗證文字／二進位文件繪製與正常退出，截圖與記錄留在 `dist/validation/runtime/`。
本次平台、測試結果與驗證範圍見 [macOS 移植驗證](docs/macos-validation.md)。
設定預設存於 `~/Library/Application Support/MadEdit/settings.json`；可用環境變數
`MADEDIT_CONFIG_DIR` 指定獨立設定目錄。`--screenshot 輸出.png` 使用預設設定並自動離開，
不讀寫日常工作階段，可供原生視窗驗證使用。

> 由於未經 Apple 公證，從網路下載的 App 第一次開啟時可能被 Gatekeeper 阻擋，
> 請在 Finder 中按右鍵選「打開」，或執行 `xattr -cr MadEdit.app` 移除隔離屬性。

### Linux 發行檔

可直接從 [Releases](https://github.com/tenyi/MadEdit/releases) 下載 `MadEdit-<版本>-linux-x86_64.tar.gz`，
解壓縮後執行 `./MadEdit`，或執行 `./install.sh` 安裝到 `~/.local`（含應用程式選單項目與圖示）。

自行打包時使用 [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild) 鎖定 glibc 2.28，
讓同一個執行檔可在較舊的發行版（如 Ubuntu 20.04）上執行：

```bash
cargo install cargo-zigbuild
# 安裝 zig（可用 uv tool install ziglang 或系統套件管理員）
./scripts/make_linux_dist.sh
```

產物位於 `dist/MadEdit-<版本>-linux-x86_64.tar.gz`（含執行檔、拼字字典、`.desktop`、圖示與 `install.sh`）。

---

## 🛠️ 核心技術棧

| 領域 | 套件 / 技術 | 用途 |
| :--- | :--- | :--- |
| **GUI 框架** | [`eframe` / `egui`](https://github.com/emilk/egui) (`v0.37`) | 即時模式 GUI、自繪編輯區與 Hex 檢視、輸入法事件 |
| **系統字型** | [`fontique`](https://crates.io/crates/fontique) | 列舉並載入系統字型 |
| **字元編碼** | [`encoding_rs`](https://crates.io/crates/encoding_rs) | 多語系字元編碼轉換與無損驗證 |
| **語法突顯** | [`syntect`](https://crates.io/crates/syntect) | Sublime Text 語法定義與主題 |
| **正規表達式** | [`regex`](https://crates.io/crates/regex) | 正規表達式搜尋與群組取代 |
| **拼字檢查** | [`spellbook`](https://crates.io/crates/spellbook) | Hunspell 字典拼字檢查 |
| **系統對話框** | [`rfd`](https://crates.io/crates/rfd) | 原生開啟舊檔與儲存檔案對話框 |
| **剪貼簿** | [`arboard`](https://crates.io/crates/arboard) | 跨平台系統剪貼簿 |

---

## 🙏 致謝

感謝原版 **MadEdit** 作者 Alston Chen，以及 **MadEdit-Mod** 維護者 Minggang Li 與所有貢獻者。本專案的圖示、工具列圖示、簡繁對照表與拼字字典皆取自原版，沒有他們多年的心血就沒有這個重製版。

---

## 📄 授權條款 (License)

本專案採用與原版開源精神一致之自由開源授權，歡迎提交 Issue 與 Pull Request！
