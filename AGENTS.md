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
  - 限制：內建 `TextEdit` 不支援虛擬捲動、矩形選取、多游標與分段著色（詳見第 6 節 Phase 2）。
- **檔案對話框**：`rfd` (Rust File Dialog) - 支援跨平台原生系統視窗。
- **編碼轉換**：`encoding_rs` - 高效能多字元編碼支援。
  - `encoding_rs_io` 已列於 `Cargo.toml` 但目前未使用（預留給大檔串流解碼）；若最終用不到請移除。
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
│   └── appwindow.slint     # 主視窗 Slint UI 介面（工具列、編輯區、狀態列）
├── src\
│   ├── main.rs             # 應用程式入口點，負責 Slint UI 與 Core 狀態雙向綁定
│   └── core\
│       ├── mod.rs          # 核心模組匯出
│       ├── model.rs        # 編輯模式 (EditMode) 與換行偵測 (NewLineFormat)
│       ├── hex.rs          # 十六進位 Hex Dump 格式化邏輯 (Offset / Hex / ASCII)
│       └── buffer.rs       # DocumentBuffer，封裝檔案 I/O、編碼解碼與內容同步
├── docs\
│   └── agents\             # Agent skills 設定（issue tracker、triage labels、domain docs，見文末）
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
- [ ] 列塊模式行為：**尚未實作**。目前 `edit-mode == 1` 與文字模式共用同一個 `TextEdit`，僅狀態列顯示不同。
- [x] 十六進位 Dump 顯示（Offset 欄、Hex Byte 欄、ASCII 預覽欄）。
  - 限制：**唯讀**，不可直接編輯 Hex；超過 1MB 的部分會被截斷（`DocumentBuffer::get_hex_dump`）。
- [x] 換行格式自動偵測（CRLF / LF / CR），目前僅顯示，不支援轉換。
- [x] 多編碼支援與自動偵測（Phase 1 完成）：
  - 開檔優先識別 BOM（UTF-8 / UTF-16LE / UTF-16BE），無 BOM 時先驗證嚴格 UTF-8，否則自動 fallback 至 Big5。
  - 工具列提供 10 種常用編碼選單（UTF-8、UTF-16LE、UTF-16BE、Big5、GBK、GB18030、Shift_JIS、EUC-JP、EUC-KR、windows-1252）與「重新解碼（Reinterpret）」、「轉換編碼（Convert）」按鈕。
  - 修正無法無損解碼時默默存檔毀損資料之問題（`has_decode_error` 為真時拒絕存檔並於狀態列提示）。
  - 修正 UTF-16LE / UTF-16BE 存檔變成 UTF-8 與遺失 BOM 之問題（自行實作 16-bit 編碼與 BOM 保留）。
- [x] 狀態列即時顯示（長度、修改狀態標記、解碼警告、模式、換行與編碼）。
- [x] 單元測試涵蓋（Hex 轉換、換行偵測、Buffer 同步、自動編碼偵測、UTF-16 往返、防損毀存檔保護、重新解碼與轉換編碼）。

### 已知問題（優先處理）

1. ~~**非 UTF-8 檔案存檔即損毀**~~（已於 Phase 1 修正：當 `has_decode_error` 為真時拒絕存檔，並支援重新解碼與轉換編碼）。
2. ~~**UTF-16 存檔會變成 UTF-8**~~（已於 Phase 1 修正：已自行實作 UTF-16LE/BE 位元組編碼與 BOM 保留）。
3. **大檔記憶體與效能**：`load_file` 整檔讀入 `Vec<u8>` 並另存一份 `String`；`update_text` 每次按鍵都重新編碼整份文字（預計於 Phase 2 / Phase 3 解決）。

---

## 6. 後續開發路線圖 (Roadmap for Next Agent)

接手的 Agent 可依據以下優先順序進行開發：

### Phase 1: 多編碼切換與轉換 (Encoding Support)
- 先修正第 5 節「已知問題」1、2：無法無損解碼時不可默默存檔（應警告或拒絕），UTF-16 需自行編碼並保留 BOM。
- 實作編碼選擇選單或下拉選單（UTF-8, UTF-16LE, Big5, GB18030, Shift-JIS, EUC-KR 等）。
- 支援「以指定編碼重新讀取」與「轉換為指定編碼儲存」。
- 視需要加入自動編碼偵測（例如 `chardetng`）。

### Phase 2: 自繪編輯器元件 (Custom Editor View)
- Slint 內建 `TextEdit` 無法滿足 Phase 3（大檔虛擬捲動）、Phase 4（矩形選取、多游標）、Phase 6（語法著色），必須先有自繪的文字檢視元件（只繪製可見行、自行處理游標與選取）。
- 這是後續多個 Phase 的前提，請勿在 `TextEdit` 上硬套上述功能。

### Phase 3: 大檔案與核心緩衝區最佳化 (Large File Engine)
- 目前版本 `DocumentBuffer` 採用記憶體內 `String` + `Vec<u8>`，適用於中小型檔案。
- 目標引入 Piece Table / Piece Tree 或分塊緩衝區 (Chunk Buffer)，支援數 GB 檔案秒級開啟與虛擬捲動 (Virtual Scrolling)。

### Phase 4: 列塊模式 (Column/Block Mode) 與游標系統
- 實作矩形區域選取（支援 Alt+滑鼠拖曳或鍵盤選取）。
- 多行游標或區塊插入/刪除。

### Phase 5: 搜尋與取代 (Find & Replace)
- 彈出式搜尋與取代面板。
- 支援大小寫匹配、全字匹配、正規表達式 (Regex)。
- 支援十六進位模式下的 Hex Byte 搜尋。

### Phase 6: 語法突顯 (Syntax Highlighting)
- 整合 `syntect` 或自建輕量詞法分析器，為各程式語言提供語法突顯功能。

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
