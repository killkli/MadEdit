# MadEdit 專案指南 (AGENTS.md)

本文件專為接手本專案的 AI 代理人撰寫，旨在提供完整的專案背景、架構設計、當前進度與後續開發指引。

---

## 1. 專案背景與願景

- **原版痛點**：原本 C++ / wxWidgets 版本的 MadEdit 在 Linux 與 macOS 平台上面臨依賴過於龐雜、編譯鏈難以維護且容易產生環境衝突等問題。
- **重製目標**：使用現代 Rust 重製 MadEdit，打造一個輕量、極速、低系統依賴且能在 Windows、macOS 與 Linux 跨平台順暢運行的專業文字與十六進位編輯器。
- **核心特色（目標，非現況；現況見第 5 節）**：
  1. 文字編輯模式 (Text Mode)
  2. 列塊選取模式 (Column / Block Mode)
  3. 十六進位模式 (Hex Mode，三欄對照：Offset、Hex、ASCII)
  4. 多語系編碼支援 (UTF-8, UTF-16, Big5, GB18030, Shift-JIS 等)
  5. 支援超大檔案 (GB 級) 快速開啟與隨機存取

---

## 2. 核心技術選型

- **程式語言**：Rust (Edition 2024，需 Rust 1.85 以上)
- **GUI 框架**：[eframe / egui](https://github.com/emilk/egui) (v0.35)，2026-10 由 Slint 改寫（見 [ADR 0011](docs/adr/0011-egui-rewrite.md)）
  - 選用理由：可逐字量測字寬，支援使用者任選系統字型（含比例字型）；輸入法事件（`Event::Ime`）直接交給自繪編輯區處理。
  - 編輯區、十六進位檢視、捲軸、尺標皆為自繪（`src/app/view.rs`、`src/app/hexview.rs`）。
- **系統字型**：`fontique` 列舉並載入系統字型，CJK 字元自動以系統中日韓字型補字。
- **系統剪貼簿**：`arboard`；**語法突顯**：`syntect`；**拼字檢查**：`spellbook`（Hunspell 字典）。
- **檔案對話框**：`rfd` (Rust File Dialog) - 支援跨平台原生系統視窗。
- **編碼轉換**：`encoding_rs` - 高效能多字元編碼支援。
- **大檔讀取**：Piece Table 以定位讀取（pread）分頁快取原始檔案，不使用 mmap（避免檔案被外部截短時 SIGBUS）。
- **應用程式圖示**：`embed-resource`（build-dependency） - 僅於 Windows 目標平台啟用，將 `assets/madedit-icon.ico` 嵌入 `.exe` 資源；其他平台為空操作。
- **設計哲學**：**KISS 原則 (Keep It Simple, Stupid)**，避免過度抽象與過早最佳化；核心資料邏輯（`src/core/`）與 UI 展現層（`src/app/`）保持分離。

---

## 3. 專案目錄結構

```text
D:\Git\MadEdit\
├── Cargo.toml              # 專案相依性與套件設定
├── Cargo.lock              # 相依性鎖定檔（需納入版控）
├── build.rs                # Windows 圖示資源嵌入
├── .gitignore
├── assets\                 # 圖示資產與桌面整合檔
│   ├── madedit-icon.png    # 視窗與 macOS 圖示（沿用原版 madedit-mod.png，include_bytes 內嵌）
│   ├── madedit-icon.ico    # Windows 執行檔圖示資源（沿用原版 images/Mad2.ico）
│   ├── madedit.desktop     # Linux 桌面整合項目
│   ├── icons-src\          # 原版 MadEdit 工具列圖示來源（XPM，勿手動修改）
│   │   └── README.md       # 圖示來源與 GNU GPL 授權說明
│   ├── icons\              # 由 icons-src 轉換而來的 PNG（勿手動修改）
│   ├── make_toolbar_icons.py # XPM → PNG 轉換腳本（純標準函式庫）
│   └── dictionaries\      # Hunspell 拼字字典（en_US 內嵌於執行檔，其餘執行時載入）
├── scripts\
│   └── make_appbundle.sh   # 產生 macOS MadEdit.app（含圖示與字典）
├── src\
│   ├── main.rs             # 進入點：解析命令列、建立視窗、啟動 eframe
│   ├── app\                # egui 介面層
│   │   ├── mod.rs          # App 狀態、分頁、存檔／關閉流程、狀態列、主迴圈
│   │   ├── commands.rs     # 命令表（依原版 CommandTable，165 個命令）與快捷鍵解析
│   │   ├── actions.rs      # 所有命令的實作（App::run）
│   │   ├── menus.rs        # 選單列與工具列
│   │   ├── dialogs.rs      # 選項、字型、排序、遞增數字、在檔案中尋找等對話框與確認視窗
│   │   ├── view.rs         # 自繪文字編輯區（格網／比例排版、換行、輸入法、拼字底線）
│   │   ├── hexview.rs      # 自繪十六進位檢視
│   │   ├── find.rs         # 搜尋／取代列與跳行列
│   │   ├── findfiles.rs    # 在多個檔案中尋找／取代（背景執行緒）
│   │   ├── macros.rs       # 巨集錄製與播放（JSON 步驟清單）
│   │   ├── print.rs        # 列印與預覽（產生 HTML 交給瀏覽器）
│   │   ├── fonts.rs        # 系統字型列舉與 egui 字型鏈
│   │   ├── ui_tests.rs     # egui_kittest 鍵盤／輸入法互動測試
│   │   └── visual_tests.rs # 離屏繪製截圖（預設略過）
│   └── core\               # 與 UI 無關的核心邏輯
│       ├── document.rs     # 單一文件：檔案 I/O、編碼、復原／重做、十六進位模式
│       ├── editor.rs       # 行陣列、游標、選取（含矩形）、字元格網
│       ├── piece_table.rs  # 十六進位模式與大檔用的 Piece Table（定位讀取分頁快取）
│       ├── hexbuf.rs       # 十六進位編輯緩衝區
│       ├── encoding.rs     # 編碼偵測、解碼與編碼
│       ├── search.rs       # 搜尋引擎（純文字／Regex／Hex）
│       ├── syntax.rs       # syntect 語法突顯與主題
│       ├── settings.rs     # 設定檔（JSON）、歷史紀錄、工作階段
│       ├── textops.rs      # 文字轉換（大小寫、全半形、排序、格式化 JSON/XML 等）
│       ├── chinese.rs      # 簡繁／日文漢字轉換（對照表 chinese_tables.rs 取自原版）
│       ├── spell.rs        # 拼字檢查
│       └── model.rs        # EditMode、NewLineFormat
├── docs\
│   ├── adr\                # 架構決策紀錄 (ADR)
│   └── agents\             # Agent skills 設定（issue tracker、triage labels、domain docs）
└── AGENTS.md               # 專案交接與開發指引文件（本檔案）
```

---

## 4. 常用開發指令

請在 `D:\Git\MadEdit` 目錄下執行（Windows 環境使用 PowerShell）；Linux 環境則為 `/home/tenyi/Projects/MadEdit`：

- **語法與型別檢查**：
  ```powershell
  cargo check
  ```
- **執行單元測試**（含 egui_kittest 互動測試）：
  ```powershell
  cargo test
  ```
- **離屏截圖**（無法開視窗時檢查畫面）：
  ```bash
  MADEDIT_SHOT_OUT=out.png MADEDIT_SHOT_FILE=檔案 MADEDIT_SHOT_CMDS=menuHexMode cargo test shot -- --ignored
  ```
- **打包 macOS App**：
  ```bash
  cargo build --release && ./scripts/make_appbundle.sh
  ```
- **啟動應用程式**：
  ```powershell
  cargo run
  ```
- **重新產生工具列圖示**（修改 `assets/icons-src/*.xpm` 後執行）：
  ```bash
  python3 assets/make_toolbar_icons.py
  ```

---

## 5. 當前完成進度

> **2026-10-23 起介面已改寫為 egui**（[ADR 0011](docs/adr/0011-egui-rewrite.md)）。以下 Phase 1–10 為 Slint 時期的歷程紀錄，
> 其中提到的 `.slint` 檔、`buffer.rs`、`hex.rs`、`memmap2` 已移除；功能皆已在 egui 版重新實作。

- [x] **egui 全面改寫與原版功能補齊**（feat/egui-rewrite）：
  - 多文件分頁、復原／重做、系統剪貼簿、設定檔與工作階段保存（最近檔案、游標位置、視窗尺寸）。
  - 使用者可選系統字型與字級（含比例字型）、CJK 自動補字、行距、深淺色主題與語法主題選擇。
  - macOS 輸入法（注音、倉頡等）預編輯顯示與候選窗定位；⌘ 與 Option 快捷鍵依 macOS 慣例。
  - 依原版命令表補齊 165 個命令：檔案（另存副本、全部儲存、關閉左／右／其他、唯讀旗標、最近檔案清單、列印與預覽）、
    編輯（剪下整行、合併行、刪除空行與行尾空白、書籤行操作、Hex 字串互轉、縮排、註解、大小寫、全半形、Tab 與空白互轉、
    遞增數字、排序、欄位對齊、日期時間）、搜尋（在多個檔案中尋找／取代、跳至對應括號、上一個／下一個位置）、
    檢視（自動換行三種模式、行號、書籤欄、尺標、控制字元、目前行、欄位提示線、打字機模式、RTL 鏡像、Markdown／HTML 預覽）、
    巨集（錄製、播放、多次播放、存檔、暫存巨集）、工具（選項、快捷鍵重新指派、拼字檢查與個人字典、字數統計、
    簡繁與日文漢字轉換、JSON／XML 格式化、Markdown 轉 HTML、HTML 轉純文字、AStyle 格式化）、視窗（視窗清單、全螢幕、置頂）。
  - 修正 problem.md 的 P0–P3 項目（輸入法、深色模式配色、CJK 補字、Hex 搜尋 panic、編碼損毀、大檔編輯與搜尋、
    存檔失敗清空內容、狀態訊息不顯示、外部變更橫幅、取代不更新書籤、存檔前檢查外部修改、mmap SIGBUS、暫存檔名、emoji 寬度、
    視窗尺寸未傳入可視範圍）。
  - 改寫期間另修：主題目前行底色與註解同色導致文字看不見、程式字型把 `->` `=>` 連字、第一幀字型家族未註冊造成 panic。
- [x] 專案基礎骨架與 Git 版本控制初始化。
- [x] 專案正式更名為 MadEdit（移除 `-Rust` 套件後綴）：
  - `Cargo.toml` 套件名 `MadEdit-Rust` → `MadEdit`，建置產出的執行檔命名為 `MadEdit`（Windows 為 `MadEdit.exe`）。
  - Slint 主視窗標題同步改為 `MadEdit`；專案目錄更名為 `MadEdit`。
  - 全面同步 `AGENTS.md`、`GLOSSARY.md`、ADR 與 `.scratch/` specs 中的專案名稱。
- [x] 應用程式圖示與工作列整合：
  - 圖示沿用原版 MadEdit（madedit-mod）：`madedit-icon.png` 取自 `madedit-mod.png`（視窗圖示與 macOS icns 來源），`madedit-icon.ico` 取自 `images/Mad2.ico`（Windows 執行檔圖示）。
  - Slint `Window` 設定 `icon` 屬性，工作列 / 標題列顯示專屬圖示。
  - `build.rs` 於 Windows 目標平台透過 `embed-resource` 將 `.ico` 嵌入 `.exe` 資源（缺少 RC.EXE / windres 時僅發警告、不中斷建置）；其他平台空操作。
  - 提供 `assets/madedit.desktop` 供 Linux 桌面環境（啟動器 / 工作列）整合。
- [x] Slint UI 整合（視窗、工具列按鈕、文字編輯區、狀態列）。
- [x] 原生跨平台「開啟舊檔」與「儲存檔案」對話框。
- [x] 模式切換 UI（文字 / 列塊 / 十六進位按鈕與狀態）。
- [x] 列塊模式 (Column/Block Mode) 與矩形選取（Phase 4 完成）：
  - 引入 `SelectionKind`（Stream 與 Block）與二維視覺矩形模型，基於固定字元格網以 Visual Column（全形 2 欄、Tab 對齊 4 欄）進行幾何矩形切割與高亮。
  - 支援工具列「列塊模式」切換與文字模式下 Alt+滑鼠拖曳 / Alt+Shift+方向鍵 觸發矩形選取。
  - 支援矩形選取複製（Ctrl+C）、剪下（Ctrl+X）、列塊對齊貼上（Ctrl+V）、多行同步打字與 Backspace/Delete 區塊切除，短行自動補齊與防越界安全防護。
  - 狀態列即時顯示矩形維度（如 `Block: 3x4`）。
- [x] 搜尋與取代引擎及內嵌式面板（Phase 5 完成）：
  - 實作核心搜尋引擎（`src/core/search.rs`），支援純文字、區分大小寫 (Match Case)、全字匹配 (Whole Word)、正規表達式 (Regex) 以及十六進位位元組 (Hex Byte Pattern) 搜尋。
  - 實作環狀循環導覽（`find_next`、`find_prev`）與單次/全部取代（`replace_current`、`replace_all`），自動高亮選取並捲動可視範圍，精確統計取代計數。
  - 在 Slint UI 實作編輯區頂部內嵌式可收合面板，包含搜尋框、取代框、核取方塊、即時匹配統計（如 `3/15`）、上一個/下一個、取代/全部取代與關閉按鈕。
  - 支援全套快捷鍵：`Ctrl+F`（開啟搜尋列並自動帶入選取文字與聚焦）、`Ctrl+H`（開啟搜尋與取代列）、`Esc`（收合面板）、`F3`（尋找下一個）、`Shift+F3`（尋找上一個）。
- [x] 語法突顯 (Syntax Highlighting)（Phase 6 完成）：
  - 整合 `syntect` 語法引擎（支援 Rust, C/C++, Python, JavaScript, TypeScript, JSON, Markdown, HTML, CSS, XML, YAML, SQL 等 30+ 種主流語言）與 `InspiredGitHub` 淺色主題。
  - 實作副檔名語言自動識別與「純文字 (Plain Text)」回退模式。
  - 可見行資料結構引入 `HighlightSpan`（`text` 與 `color`），在 Slint 自繪編輯器頂層實現文字分段多色著色渲染，幾何選取圖層與色彩層徹底解耦。
  - 工具列提供語言切換下拉選單，狀態列即時顯示當前語法名稱。
- [x] 列號跳轉與編輯增強 (Go to Line & Editing Enhancements)（Phase 7 完成）：
  - 實作頂部內嵌可收合指定行號與欄位跳轉面板（支援 `Ctrl+G` 開啟、`Enter` 跳轉、`Esc` 取消收合）。
  - 支援 `line` 與 `line:col` 語法，超出範圍自動 Clamp 邊界防護；在十六進位模式下自動切換為 Offset 跳轉（支援 `0x` 與十六進位位移解析，超大檔案零拷貝隨機分頁跳轉）。
  - 實作常用行級編輯操作（`Alt+Up` / `Alt+Down` 整行上下移動、`Ctrl+Shift+D` 複製行、`Ctrl+Shift+K` 刪除整行、`Home` 智慧縮排首字元與行首切換）。
  - `DocumentBuffer` 統一封裝行編輯與跳轉操作，自動同步底層緩衝區與 UI 渲染。
- [x] 十六進位 Dump 顯示（Offset 欄、Hex Byte 欄、ASCII 預覽欄）。
  - 已優化：直接自 Piece Table 隨機讀取前 1MB 預覽，無全檔記憶體拷貝。
- [x] 換行格式轉換與書籤功能 (Line Endings Conversion & Bookmarks)（Phase 8 完成）：
  - 支援 CRLF (Windows)、LF (Unix/macOS)、CR (Classic Mac) 換行格式切換、雙向轉換與原子存檔。
  - 工具列提供換行格式下拉選單，狀態列即時顯示當前格式名稱。
  - 實作基於 `BTreeSet` 之編輯器行書籤系統，支援切換書籤（`Ctrl+F2`）、跳至下一個書籤（`F2`）、跳至上一個書籤（`Shift+F2`）與清除全部書籤（`Ctrl+Shift+F2`）。
  - 在左側行號欄（Gutter）繪製醒目的藍色小圓點標記，並在行插入、移動、複製與刪除操作中精確維護書籤座標不變量。
- [x] 外部檔案變更監控與重新載入 (File Watcher & Auto Reload)（Phase 9 完成）：
  - `DocumentBuffer` 追蹤磁碟時間戳記（`last_modified`），實作零依賴、純標準庫之 `mtime` 比對機制（`check_external_change`、`reload_file`、`ignore_external_change`）。
  - 修復 Windows 平台小型檔案（<=512KB）因記憶體對映持有控制代碼導致外部寫入衝突（Win32 1224）之問題，小檔案自動轉為記憶體 PieceTable，徹底釋放檔案鎖定。
  - 於 Slint 主迴圈配置每 1.5 秒之輕量定時檢查，在編輯區頂部顯示醒目非侵入式琥珀黃色通知橫幅（`⚠️ 檔案已被外部程式修改。`），若有未儲存修改則加註深紅警示，支援「重新載入」與「忽略」。
  - 重新載入時自動保留游標行欄與書籤集合，並執行邊界自動防護。
- [x] 編輯器焦點輸入修復與原版 MadEdit 經典專業 UI（Phase 10 完成）：
  - 將 `editor-focus := FocusScope` 提升至外層常駐並配置 `focus-editor()`，徹底修復點擊工具列或編輯區後無法打字輸入的焦點遺失問題。
  - 實作深淺色雙主題系統（預設跟隨作業系統 `Palette.color-scheme`，支援一鍵切換），`syntect` 語法引擎同步切換深色（`base16-ocean.dark`）與淺色（`InspiredGitHub`）主題，並擴充 C# (`.cs`)、Java、Go、TOML、Shell 語法識別。
  - 實作原版 MadEdit 四大經典介面元素：頂部主選單列 + 緊湊型工具列（含字級縮放與內嵌快速搜尋框）、檔案分頁標籤列 + 水平欄位尺標（Column Ruler `0..10..20..`）、游標所在行外框高亮 + 可視控制字元（空白 `·`、Tab `»`、行尾綠色 `↵`）、以及底部 5 格專業狀態列（含 `Insert` / `Overwrite` 插入覆寫模式切換）。
- [x] 多編碼支援與自動偵測（Phase 1 完成）：
  - 開檔優先識別 BOM（UTF-8 / UTF-16LE / UTF-16BE），無 BOM 時先驗證嚴格 UTF-8，否則自動 fallback 至 Big5。
  - 工具列提供 10 種常用編碼選單與「重新解碼（Reinterpret）」、「轉換編碼（Convert）」按鈕。
  - 修正無法無損解碼時默默存檔毀損資料之問題（`has_decode_error` 防護）。
  - 修正 UTF-16LE / UTF-16BE 存檔變成 UTF-8 與遺失 BOM 之問題（自行實作 16-bit 編碼與 BOM 保留）。
- [x] 自繪編輯器元件（Phase 2 完成）：
  - 採用 Slint 宣告式可見行虛擬渲染（`for line in visible-lines`）結合 `FocusScope` 與 `TouchArea`，完全取代內建 `TextEdit`。
  - 實作固定字元格網（Character Cell Grid：半形 1 欄、CJK 全形 2 欄、Tab 展開對齊 4 欄），徹底解決跨平台等寬字型游標漂移問題。
  - 支援左側行號欄（Gutter）、方向鍵導覽、Shift 擴展選取、滑鼠點擊/拖曳選取、滾輪捲動、文字增刪、剪貼簿快捷鍵（Ctrl+C/X/V/A）與狀態列即時 `Ln X, Col Y` 顯示。
- [x] 大檔案與核心緩衝區最佳化（Phase 3 完成）：
  - 引入 `memmap2` 唯讀記憶體對映，實現大檔案秒級開啟與零拷貝隨機分頁讀取。
  - 實作 Piece Table（Original Buffer + Add Buffer + Piece 列表）與輕量 `LineIndex`（換行位移二分搜尋索引）。
  - 存檔直接透過 Piece 清單串流寫出，徹底消除全檔記憶體佔用與按鍵重複全篇編碼之負擔。
- [x] 狀態列即時顯示（長度、修改狀態標記、解碼警告、模式、換行與編碼）。
- [x] 單元測試涵蓋（Hex 轉換、換行偵測、Buffer 同步、自動編碼偵測、UTF-16 往返、防損毀存檔保護、重新解碼與轉換編碼、字元格網計算、游標選取導覽與編輯、Piece Table 編輯/刪除、LineIndex 按需讀取與 CR/LF/CRLF 支援、大檔 mmap 秒開、大檔編輯與原子存檔測試、相容性查詢介面、字元格網視覺切片提取、矩形選取高亮渲染、列塊複製/剪下/貼上/多行同步打字、Buffer 列塊模式渲染測試）。
- [x] 經典 MadEdit 風格介面重塑（Phase 10 完成）：
  - 新增 `ui/theme.slint` 經典樣式元件庫（配色、`ClassicToolButton`、`ClassicButton`、`ClassicLineEdit`、`ClassicCheckBox`、`ToolbarSeparator`、自繪提示框）。
  - 新增 `ui/menubar.slint` 經典選單列與彈出選單（`MenuItemDef` / `MenuDef` 資料模型、分隔線、核取記號、快捷鍵、停用項灰顯、單一共用 `PopupWindow` + `open-index`）。
  - 重寫 `ui/appwindow.slint`：經典選單列 → 原版圖示工具列（6 組 21 個圖示）→ 分頁列 → 搜尋／跳轉／外部變更橫幅 → 編輯區 → 六欄位狀態列（Ln / CharPos / SelSize / 編碼.DOS / 唯讀 / INS）。
  - `src/main.rs` 依原版 `CommandTable` 建構 7 個頂層選單（檔案／編輯／搜尋／檢視／工具／視窗／說明），以 Rust 端計算選單寬度、x-offset 與彈出選單尺寸，並以 `menu-action(string)` 回呼統一派送動作。
  - 語法突顯／字元編碼／換行格式／重新解碼／轉換編碼改置於選單，點選後開啟通用可捲動選擇面板。
  - 新增 `assets/icons-src/`（38 個原版 XPM 來源 + GPL 授權說明）與 `assets/make_toolbar_icons.py`（純標準函式庫 XPM→PNG 轉換器）。
  - 併入另一條並行 Phase 10 路線的完整功能（見 [ADR 0010](docs/adr/0010-classic-ui-consolidation.md)）：
    - 深色／淺色主題（`Theme.is-dark` 統一管理色票，預設跟隨系統配色，工具列可切換）。
    - 水平欄位尺標（與水平捲軸同步位移）、右側垂直捲軸與底部水平捲軸（滑塊拖曳 + 按鈕微調）。
    - 控制字元顯示（空白 `·`、Tab `»`、行尾 `↵`）與游標所在行外框高亮。
    - 插入／覆寫模式（`EditorState::overwrite_mode`，`Insert` 鍵切換，狀態列顯示 `INS`／`OVR`）。
    - 常駐 `FocusScope` 承接鍵盤事件並於開檔、存檔、切換模式後自動回復焦點（焦點修復）。
    - 新增 C#、Java、Go、Shell、TOML 語法辨識；`MadEdit <檔案>` 命令列開檔。

### 已知問題（優先處理）

1. ~~**非 UTF-8 檔案存檔即損毀**~~（已於 Phase 1 修正：當 `has_decode_error` 為真時拒絕存檔，並支援重新解碼與轉換編碼）。
2. ~~**UTF-16 存檔會變成 UTF-8**~~（已於 Phase 1 修正：已自行實作 UTF-16LE/BE 位元組編碼與 BOM 保留）。
3. ~~**大檔記憶體與效能**~~（已於 Phase 3 修正：引入 Piece Table + `memmap2` 記憶體對映，大檔秒開與串流存檔，不再全檔重複分配與全篇編碼）。

---

## 6. 後續開發路線圖 (Roadmap for Next Agent)

接手的 Agent 可依據以下優先順序進行開發：

### Phase 1: 多編碼切換與轉換 (Encoding Support) - [已完成]
- 已完成字元編碼偵測、轉換、防毀損存檔保護與工具列操作整合。

### Phase 2: 自繪編輯器元件 (Custom Editor View) - [已完成]
- 已完成 Slint 可見行虛擬渲染、固定字元格網、行號欄、游標/選取與鍵盤滑鼠互動。

### Phase 3: 大檔案與核心緩衝區最佳化 (Large File Engine) - [已完成]
- 已完成 Piece Table 資料結構、`memmap2` 大檔零拷貝開啟、`LineIndex` 行索引與串流存檔。

### Phase 4: 列塊模式 (Column/Block Mode) 與游標系統 - [已完成]
- 已完成二維視覺矩形選取模型、字元格網視覺切片、列塊剪貼簿（複製/剪下/貼上）、多行同步輸入與刪除、以及 Slint UI Alt 鍵拖曳整合。

### Phase 5: 搜尋與取代 (Find & Replace) - [已完成]
- 已完成核心搜尋引擎（純文字/大小寫/全字/Regex/Hex Byte）、環狀循環導覽、取代與全部取代。
- 已完成 Slint 內嵌可收合面板、即時進度（`3/15`）、快捷鍵（Ctrl+F、Ctrl+H、Esc、F3、Shift+F3）與自動聚焦。

### Phase 6: 語法突顯 (Syntax Highlighting) - [已完成]
- 已整合 `syntect` 語法引擎（30+ 種主流語言與 InspiredGitHub 主題）。
- 已完成開檔自動識別、工具列語言切換選單、可見行多色文字分段渲染與狀態列顯示。

### Phase 7: 列號跳轉與編輯增強 (Go to Line & Editing Enhancements) - [已完成]
- 已完成指定行號/欄位跳轉內嵌面板與 Hex Offset 隨機跳轉支援（Ctrl+G）。
- 已完成常用程式碼行編輯操作（Alt+Up/Down 整行上下移動、Ctrl+Shift+D 複製行、Ctrl+Shift+K 刪除整行、Home 智慧行首）。

### Phase 8: 換行格式轉換與書籤功能 (Line Endings Conversion & Bookmarks) - [已完成]
- 已完成 CRLF、LF、CR 換行格式雙向轉換、工具列選單與存檔同步。
- 已完成編輯器行書籤（Ctrl+F2 切換、F2 / Shift+F2 環狀導覽、Ctrl+Shift+F2 全清除、Gutter 藍點標記與行編輯座標維護）。

### Phase 9: 外部檔案變更監控 (File Watcher & Auto Reload) - [已完成]
- 已完成檔案時間戳記 (`mtime`) 比對、Windows 記憶體對映解鎖、Slint 定時器、頂部內嵌黃色通知橫幅與游標/書籤智慧保留重新載入。

### Phase 11: egui 全面改寫與原版功能補齊 - [已完成]
- 見第 5 節與 [ADR 0011](docs/adr/0011-egui-rewrite.md)。未實作項目：單一執行個體、介面語言切換、檔案關聯、真正的 bidi、
  語法配色逐項自訂、跨行 Regex、Python 巨集（改為 JSON 步驟清單）。

### Phase 10: 經典 MadEdit 風格介面重塑 (Classic UI Redesign) - [已完成]
- 已完成經典 Windows 風格選單列、彈出選單、原版圖示工具列、分頁列與六欄位狀態列。
- 已完成選單模型與核心狀態雙向同步（編輯模式核取記號、停用／啟用狀態、寬度與位置計算）。
- 已完成原版 MadEdit XPM 圖示沿用與標準函式庫 PNG 轉換工具鏈。
- 已完成與另一條並行 Phase 10 路線的整合：深淺色主題、欄位尺標、垂直／水平捲軸、控制字元、
  插入／覆寫模式、焦點修復與多語法支援（詳見 [ADR 0010](docs/adr/0010-classic-ui-consolidation.md)）。

---

## 7. 給接手 Agent 的協作原則

1. **嚴格遵循 KISS 原則**：不引進不必要的複雜抽象，每個功能以最直覺、最易維護的方式實作。
2. **一律使用繁體中文**：溝通、文件與程式碼註解皆使用 `zh-TW` 繁體中文，註解需詳細說明意圖。
3. **保持測試完整性**：新增核心邏輯時務必加入對應單元測試，並確保 `cargo test` 與 `cargo check` 零 Warning 通過。
4. **Git 規範**：
   - 主分支為 `main`；新功能請開 `feat/<名稱>` 分支開發。
   - Commit 遵循 Conventional Commits (例如 `feat: ...`, `fix: ...`, `refactor: ...`)。
5. **文件同步**：完成功能或發現新限制時，同步更新本檔第 5 節進度與已知問題。

---

## Agent skills

### Issue tracker

Issues 與 Specs 以本地 Markdown 檔案形式存放於 `.scratch/` 目錄下。請參閱 `docs/agents/issue-tracker.md`。

### Triage labels

使用預設的五種標準分類狀態標籤（`needs-triage`、`needs-info`、`ready-for-agent`、`ready-for-human`、`wontfix`）。請參閱 `docs/agents/triage-labels.md`。

### Domain docs

採用單一語境（single-context）結構（根目錄 `GLOSSARY.md` 與 `docs/adr/`）。請參閱 `docs/agents/domain.md`。
