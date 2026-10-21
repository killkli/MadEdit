# MadEdit-Rust 專案指南 (AGENTS.md)

本文件專為接手本專案的 AI 代理人（Gemini 4 Argon 及後續協作者）撰寫，旨在提供完整的專案背景、架構設計、當前進度與後續開發指引。

---

## 1. 專案背景與願景

- **原版痛點**：原本 C++ / wxWidgets 版本的 MadEdit 在 Linux 與 macOS 平台上面臨依賴過於龐雜、編譯鏈難以維護且容易產生環境衝突等問題。
- **重製目標**：使用現代 Rust 重製 MadEdit，打造一個輕量、極速、低系統依賴且能在 Windows、macOS 與 Linux 跨平台順暢運行的專業文字與十六進位編輯器。
- **核心特色**：
  1. 文字編輯模式 (Text Mode)
  2. 列塊選取模式 (Column / Block Mode)
  3. 十六進位模式 (Hex Mode，三欄對照：Offset、Hex、ASCII)
  4. 多語系編碼支援 (UTF-8, UTF-16, Big5, GB18030, Shift-JIS 等)
  5. 支援超大檔案 (GB 級) 快速開啟與隨機存取

---

## 2. 核心技術選型

- **程式語言**：Rust (Edition 2024)
- **GUI 框架**：[Slint](https://slint.dev/) (v1.18.1)
  - 優勢：宣告式語法 (`.slint`)、輕量、純 Rust 原生編譯、不依賴繁重的系統 C++ 庫、跨平台 (Windows/macOS/Linux) 表現一致。
- **檔案對話框**：`rfd` (Rust File Dialog) - 支援跨平台原生系統視窗。
- **編碼轉換**：`encoding_rs` 與 `encoding_rs_io` - 高效能多字元編碼支援。
- **設計哲學**：**KISS 原則 (Keep It Simple, Stupid)**，避免過度抽象與過早最佳化；核心資料邏輯（`src/core/`）與 UI 展現層（`ui/`）保持分離。

---

## 3. 專案目錄結構

```text
D:\Git\MadEdit-Rust\
├── Cargo.toml              # 專案相依性與套件設定
├── build.rs                # Slint UI 編譯腳本 (slint_build::compile)
├── ui\
│   └── appwindow.slint     # 主視窗 Slint UI 介面（工具列、編輯區、狀態列）
├── src\
│   ├── main.rs             # 應用程式入口點，負責 Slint UI 與 Core 狀態雙向綁定
│   └── core\
│       ├── mod.rs          # 核心模組匯出
│       ├── model.rs        # 編輯模式 (EditMode) 與換行偵測 (NewLineFormat)
│       ├── hex.rs          # 十六進位 Hex Dump 格式化邏輯 (Offset / Hex / ASCII)
│       └── buffer.rs       # DocumentBuffer，封裝檔案 I/O、編碼解碼與內容同步
└── AGENTS.md               # 專案交接與開發指引文件（本檔案）
```

---

## 4. 常用開發指令

請在 `D:\Git\MadEdit-Rust` 目錄下執行：

- **語法與型別檢查**：
  ```bash
  cargo check
  ```
- **執行單元測試**：
  ```bash
  cargo test
  ```
- **啟動應用程式**：
  ```bash
  cargo run
  ```

---

## 5. 當前完成進度

- [x] 專案基礎骨架與 Git 版本控制初始化。
- [x] Slint UI 整合（視窗、工具列按鈕、文字編輯區、狀態列）。
- [x] 原生跨平台「開啟舊檔」與「儲存檔案」對話框。
- [x] 編輯模式切換（文字模式、列塊模式、十六進位模式）。
- [x] 十六進位即時 Dump（Offset 欄、Hex Byte 欄、ASCII 預覽欄）。
- [x] 換行格式自動偵測（CRLF / LF / CR）。
- [x] 狀態列即時顯示（長度、修改狀態標記、模式、換行與編碼）。
- [x] 單元測試涵蓋（Hex 轉換、換行偵測、Buffer 同步）。

---

## 6. 後續開發路線圖 (Roadmap for Next Agent)

接手的 Agent 可依據以下優先順序進行開發：

### Phase 1: 多編碼切換與轉換 (Encoding Support)
- 實作編碼選擇選單或下拉選單（UTF-8, UTF-16LE, Big5, GB18030, Shift-JIS, EUC-KR 等）。
- 支援「以指定編碼重新讀取」與「轉換為指定編碼儲存」。

### Phase 2: 大檔案與核心緩衝區最佳化 (Large File Engine)
- 現前版本 `DocumentBuffer` 採用記憶體內 `String` + `Vec<u8>`，適用於中小型檔案。
- 目標引入 Piece Table / Piece Tree 或分塊緩衝區 (Chunk Buffer)，支援數 GB 檔案秒級開啟與虛擬滾動 (Virtual Scrolling)。

### Phase 3: 列塊模式 (Column/Block Mode) 與游標系統
- 實作矩形區域選取（支援 Alt+滑鼠拖曳或鍵盤選取）。
- 多行游標或區塊插入/刪除。

### Phase 4: 搜尋與取代 (Find & Replace)
- 彈出式搜尋與取代面板。
- 支援大小寫匹配、全字匹配、正規表達式 (Regex)。
- 支援十六進位模式下的 Hex Byte 搜尋。

### Phase 5: 語法突顯 (Syntax Highlighting)
- 整合 `syntect` 或自建輕量詞法分析器，為各程式語言提供語法突顯功能。

---

## 7. 給接手 Agent 的協作原則

1. **嚴格遵循 KISS 原則**：不引進不必要的複雜抽象，每個功能以最直覺、最易維護的方式實作。
2. **維持繁體中文繁簡規範**：溝通及所有文件請使用 `zh-TW 繁體中文`。
3. **保持測試完整性**：新增核心邏輯時務必加入對應單元測試，並確保 `cargo test` 與 `cargo check` 零 Warning 通過。
4. **Git Commit 規範**：遵循 Conventional Commits (例如 `feat: ...`, `fix: ...`, `refactor: ...`)。

---

## Agent skills

### Issue tracker

Issues 與 Specs 以本地 Markdown 檔案形式存放於 `.scratch/` 目錄下。請參閱 `docs/agents/issue-tracker.md`。

### Triage labels

使用預設的五種標準分類狀態標籤（`needs-triage`、`needs-info`、`ready-for-agent`、`ready-for-human`、`wontfix`）。請參閱 `docs/agents/triage-labels.md`。

### Domain docs

採用單一語境（single-context）結構（根目錄 `GLOSSARY.md` 與 `docs/adr/`）。請參閱 `docs/agents/domain.md`。

