# MadEdit-Rust 專案指南 (AGENTS.md)

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
- **GUI 框架**：[Slint](https://slint.dev/) (v1.18.1)
  - 優勢：宣告式語法 (`.slint`)、輕量、純 Rust 原生編譯、不依賴繁重的系統 C++ 庫、跨平台 (Windows/macOS/Linux) 表現一致。
  - 限制：內建 `TextEdit` 不支援虛擬捲動、矩形選取、多游標與分段著色（已於 Phase 2 自繪編輯器元件中完全取代）。
- **檔案對話框**：`rfd` (Rust File Dialog) - 支援跨平台原生系統視窗。
- **編碼轉換**：`encoding_rs` - 高效能多字元編碼支援。
- **記憶體對映**：`memmap2` - 支援超大檔案零拷貝秒開與隨機分頁存取。
- **設計哲學**：**KISS 原則 (Keep It Simple, Stupid)**，避免過度抽象與過早最佳化；核心資料邏輯（`src/core/`）與 UI 展現層（`ui/`）保持分離。

---

## 3. 專案目錄結構

```text
D:\Git\MadEdit-Rust\
├── Cargo.toml              # 專案相依性與套件設定
├── Cargo.lock              # 相依性鎖定檔（需納入版控）
├── build.rs                # Slint UI 編譯腳本 (slint_build::compile)
├── .gitignore
├── ui\
│   └── appwindow.slint     # 主視窗 Slint UI 介面（工具列、自繪編輯區、狀態列）
├── src\
│   ├── main.rs             # 應用程式入口點，負責 Slint UI 與 Core 狀態雙向綁定
│   └── core\
│       ├── mod.rs          # 核心模組匯出
│       ├── model.rs        # 編輯模式 (EditMode) 與換行偵測 (NewLineFormat)
│       ├── editor.rs       # 游標模型、選取範圍、字元格網與可見行切片產生
│       ├── piece_table.rs  # Piece Table 核心資料結構與 LineIndex 行索引
│       ├── hex.rs          # 十六進位 Hex Dump 格式化邏輯 (Offset / Hex / ASCII)
│       └── buffer.rs       # DocumentBuffer，封裝檔案 I/O、編碼解碼與內容同步
├── docs\
│   ├── adr\                # 架構決策紀錄 (ADR)
│   └── agents\             # Agent skills 設定（issue tracker、triage labels、domain docs）
└── AGENTS.md               # 專案交接與開發指引文件（本檔案）
```

---

## 4. 常用開發指令

請在 `D:\Git\MadEdit-Rust` 目錄下執行（Windows 環境使用 PowerShell）：

- **語法與型別檢查**：
  ```powershell
  cargo check
  ```
- **執行單元測試**：
  ```powershell
  cargo test
  ```
- **啟動應用程式**：
  ```powershell
  cargo run
  ```

---

## 5. 當前完成進度

- [x] 專案基礎骨架與 Git 版本控制初始化。
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

### Phase 9: 外部檔案變更監控 (File Watcher & Auto Reload)
- 實作背景檔案監控，偵測磁碟上的檔案是否被外部編輯器或程序修改。
- 彈出重新載入確認對話框，或在無未儲存修改時智慧自動重新載入。

---

## 7. 給接手 Agent 的協作原則

1. **嚴格遵循 KISS 原則**：不引進不必要的複雜抽象，每個功能以最直覺、最易維護的方式實作。
2. **一律使用繁體中文**：溝通、文件與程式碼註解皆使用 `zh-TW` 繁體中文，註解需詳細說明意圖。
3. **保持測試完整性**：新增核心邏輯時務必加入對應單元測試，並確保 `cargo test` 與 `cargo check` 零 Warning 通過。
4. **Git 規範**：
   - 主分支為 `master`；新功能請開 `feat/<名稱>` 分支開發。
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
