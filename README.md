# MadEdit-Rust

**以現代 Rust 與 Slint 打造的跨平台、輕量、極速文字 / 列塊 / 十六進位編輯器**  
*A modern, lightweight, cross-platform Text / Column / Hex editor inspired by classic MadEdit, written in pure Rust.*

---

## 📖 專案背景與願景

原版 C++ / wxWidgets 架構的 **MadEdit** 是許多開發者與資料處理人員心目中的經典工具（支援列塊編輯、Hex 檢視與多國語系編碼切換），但隨著時間推移，龐雜的 C++ 跨平台依賴與編譯鏈在現代 Windows、macOS 與 Linux 環境下日益難以維護。

**MadEdit-Rust** 以 **Rust (Edition 2024)** 與宣告式原生 GUI 框架 **[Slint](https://slint.dev/)** 重新打造，遵循 **KISS (Keep It Simple, Stupid)** 設計原則，在零 C++ GUI 系統庫依賴的前提下，重現原版 MadEdit 的經典專業介面與核心編輯能力。

---

## ✨ 核心功能特色

### 1. 三大編輯與檢視模式
- **文字模式 (Text Mode)**：一般串流文字與程式碼編輯，支援智慧縮排行首 (`Home`)、整行上下移動、複製行與刪除行。
- **列塊模式 (Column / Block Mode)**：
  - 基於**固定等寬字元格網 (Character Cell Grid)**（半形 1 欄、CJK 全形 2 欄、Tab 自動對齊 4 欄），徹底解決跨平台中英混排對齊與游標漂移問題。
  - 支援工具列切換或按住 `Alt + 滑鼠拖曳` / `Alt + Shift + 方向鍵` 進行二維視覺矩形選取。
  - 支援**矩形區塊複製、剪下、列塊對齊貼上、多行同步打字輸入與區塊切除**（短行自動補齊空白）。
- **十六進位模式 (Hex Mode)**：
  - 經典三欄對照檢視：**位址偏移 (Offset)**、**十六進位位元組 (Hex Bytes)**、**ASCII 可視預覽**。
  - 支援十六進位位移跳轉（如 `0x1A40`）與 Hex Byte Pattern 搜尋（如 `4D 5A 90`）。

### 2. 多語系編碼支援與防損毀保護
- 整合 `encoding_rs` 與自訂 UTF-16 編解碼器，支援 **UTF-8、UTF-8 BOM、UTF-16LE、UTF-16BE、Big5 (繁體中文)、GB18030 (簡體中文)、Shift_JIS (日文)、EUC-KR (韓文)、windows-1252、ISO-8859-1**。
- **智慧開檔偵測**：優先識別 BOM，無 BOM 時驗證嚴格 UTF-8，失敗則自動回退至 Big5。
- **無損防呆機制**：當檔案含有當前編碼無法無損解碼的位元組時，狀態列主動發出警告並保護原檔不被覆寫損毀，提供一鍵**「重解碼 (Reinterpret)」**與**「轉編碼 (Convert)」**。

### 3. 大檔案引擎 (Piece Table + Memory Mapping)
- 底層採用 **Piece Table** 資料結構結合 `memmap2` 唯讀記憶體對映與二分搜尋行索引 (`LineIndex`)。
- 大型檔案零拷貝秒開、按需分頁讀取（On-demand Line Fetching）與串流原子存檔。
- 針對小型檔案（`<= 512 KB`）自動載入記憶體並釋放作業系統檔案控制代碼，避免 Windows 外部程式寫入鎖定衝突。

### 4. 原版 MadEdit 經典專業 UI 與深淺色主題
- **深色 / 淺色雙主題**：預設跟隨系統配色，支援選單列一鍵切換（`🌙 深色主題` / `☀️ 淺色主題`），徹底杜絕深淺色混用吃字問題。
- **水平欄位尺標 (Column Ruler)**：頂部 `0..10..20..` 刻度尺標，與字元格網及下方水平捲軸即時同步滑動。
- **可視控制字元 (Control Characters)**：支援顯示/隱藏空白中點 (`·`)、Tab 導引符 (`»`) 與行尾綠色換行標記 (`↵`)。
- **雙向虛擬捲軸 (Vertical & Horizontal Scrollbars)**：支援滑塊拖曳、軌道點擊跳轉、微調按鈕與游標超出邊界自動跟隨捲動。
- **原版 5 格專業狀態列**：即時顯示 `行 / 總行 與 列 [列塊維度]`、`字元位置 (十進位 / 十六進位) / 總長`、`選取字元與位元組數`、`編碼.BOM.換行格式 [語法]` 與 `插入 / 覆寫模式`。

### 5. 進階搜尋與取代、語法突顯、書籤與外部變更監控
- **搜尋與取代引擎**：支援區分大小寫 (Match Case)、全字匹配 (Whole Word)、正規表達式 (Regex) 與 Hex 位元組搜尋，提供即時匹配計數（如 `3/15`）、環狀導覽、單筆取代與全部取代。
- **語法突顯 (Syntax Highlighting)**：整合 `syntect` 引擎，支援 Rust、C/C++、C#、Java、Go、Python、JavaScript、TypeScript、JSON、TOML、YAML、Markdown、HTML、CSS、SQL、Shell 等 30+ 種主流語言，隨深淺色主題自動切換配色（`base16-ocean.dark` / `InspiredGitHub`）。
- **換行格式轉換與行書籤**：支援 `CRLF (Windows)`、`LF (Unix/macOS)`、`CR (Classic Mac)` 即時轉換；支援左側行號欄藍色圓點書籤標記與環狀跳轉，並在增刪移動行時自動維護書籤位置。
- **外部檔案變更監控 (File Watcher)**：每 1.5 秒輕量檢查磁碟 `mtime`，外部程式修改檔案時於頂部彈出非侵入式醒目橫幅，重新載入時自動保留游標與書籤位置。

---

## ⌨️ 常用快捷鍵一覽

| 分類 | 快捷鍵 / 操作 | 功能說明 |
| :--- | :--- | :--- |
| **檔案操作** | `Ctrl + O` | 開啟檔案 |
| | `Ctrl + S` | 儲存檔案 |
| **選取與列塊** | `Alt + 滑鼠拖曳` | 在文字模式下直接拉出矩形列塊選取 (Block Selection) |
| | `Alt + Shift + 方向鍵` | 透過鍵盤擴展矩形列塊選取 |
| | `Ctrl + A` / `C` / `X` / `V` | 全選 / 複製 / 剪下 / 貼上（列塊選取下自動執行矩形複製與對齊貼上） |
| **搜尋與跳轉** | `Ctrl + F` | 開啟/聚焦搜尋面板（自動帶入當前選取文字） |
| | `Ctrl + H` | 開啟搜尋與取代面板 |
| | `F3` / `Shift + F3` | 尋找下一個 / 尋找上一個 |
| | `Ctrl + G` | 指定行號與欄位跳轉（`line` 或 `line:col`；Hex 模式下為位移 `0x...` 跳轉） |
| | `Esc` | 收合搜尋或跳轉面板並將鍵盤焦點歸還編輯區 |
| **程式碼行編輯** | `Alt + Up` / `Alt + Down` | 將當前行（或選取範圍行）整行向上 / 向下移動 |
| | `Ctrl + Shift + D` | 向下複製當前行（或選取區塊行） |
| | `Ctrl + Shift + K` | 刪除當前整行（或選取區塊行） |
| | `Home` | 智慧行首切換（於「第一個非空白縮排字元」與「絕對行首第 0 欄」間切換） |
| | `Insert` | 切換「插入 (Insert)」與「覆寫 (Overwrite)」模式 |
| **書籤導覽** | `Ctrl + F2` | 在當前行切換書籤（新增 / 移除） |
| | `F2` / `Shift + F2` | 跳至下一個 / 上一個書籤（環狀循環） |
| | `Ctrl + Shift + F2` | 清除全部書籤 |

---

## 🏗️ 專案目錄結構

```text
MadEdit-Rust/
├── Cargo.toml              # Rust 專案設定與相依套件
├── Cargo.lock              # 相依版本鎖定檔
├── build.rs                # Slint UI 編譯腳本 (slint_build::compile)
├── ui/
│   └── appwindow.slint     # Slint 主視窗介面（選單列、工具列、尺標、自繪編輯區、雙捲軸、5 格狀態列）
├── src/
│   ├── main.rs             # 應用程式入口，負責 Slint UI 與核心狀態雙向綁定及事件調度
│   └── core/
│       ├── mod.rs          # 核心模組匯出
│       ├── model.rs        # 編輯模式 (EditMode) 與換行格式 (NewLineFormat)
│       ├── buffer.rs       # DocumentBuffer：封裝檔案 I/O、多編碼解碼、外部監控與操作協調
│       ├── editor.rs       # 游標模型、串流/矩形選取、字元格網、可視控制字元與雙向 Viewport
│       ├── piece_table.rs  # Piece Table 核心資料結構與 LineIndex 二分搜尋行索引
│       ├── search.rs       # 搜尋與取代引擎（純文字 / 大小寫 / 全字 / Regex / Hex 位元組）
│       ├── syntax.rs       # syntect 語法突顯整合（支援深淺色雙主題與跨行狀態快取）
│       └── hex.rs          # 十六進位 Hex Dump 格式化邏輯 (Offset / Hex / ASCII)
├── docs/                   # 架構決策紀錄 (ADR) 與領域文件
├── GLOSSARY.md             # 專案領域詞彙表
└── AGENTS.md               # AI 協作與開發路線圖指南
```

---

## 🚀 建置與執行指南

### 環境需求
- **Rust 工具鏈**：Rust `1.85` 或以上版本（支援 Rust Edition 2024）
- 支援平台：**Windows**、**macOS**、**Linux**

### 開發與測試指令

```bash
# 1. 檢查語法與型別
cargo check

# 2. 執行完整單元測試（涵蓋編碼、Piece Table、列塊編輯、搜尋、語法、書籤與捲軸等 57 項測試）
cargo test

# 3. 啟動開發版應用程式
cargo run

# 4. 編譯最佳化 Release 正式版執行檔
cargo build --release
```

編譯完成後，可於 `target/release/MadEdit`（Windows 為 `target/release/MadEdit.exe`）取得獨立執行檔。

---

## 🛠️ 核心技術棧

| 領域 | 套件 / 技術 | 用途 |
| :--- | :--- | :--- |
| **GUI 框架** | [`slint`](https://slint.dev/) (`v1.18.1`) | 宣告式原生 GUI 渲染、自繪虛擬編輯區與跨平台視窗事件處理 |
| **字元編碼** | [`encoding_rs`](https://crates.io/crates/encoding_rs) | 高效能多語系字元編碼轉換與無損驗證 |
| **大檔對映** | [`memmap2`](https://crates.io/crates/memmap2) | 跨平台唯讀記憶體對映 (Memory-Mapped File I/O) |
| **語法突顯** | [`syntect`](https://crates.io/crates/syntect) | Sublime Text 語法定義解析與深淺色主題多色著色 |
| **正規表達式** | [`regex`](https://crates.io/crates/regex) | 正規表達式搜尋與群組取代 |
| **系統對話框** | [`rfd`](https://crates.io/crates/rfd) | 原生跨平台開啟舊檔與儲存檔案對話框 |
| **剪貼簿與字寬** | [`arboard`](https://crates.io/crates/arboard) / [`unicode-width`](https://crates.io/crates/unicode-width) | 跨平台系統剪貼簿存取與 CJK 東亞全形字元寬度計算 |

---

## 📄 授權條款 (License)

本專案採用與原版開源精神一致之自由開源授權，歡迎提交 Issue 與 Pull Request！
