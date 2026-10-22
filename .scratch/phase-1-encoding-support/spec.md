Status: ready-for-agent

# Phase 1: 多編碼切換與轉換 (Encoding Support)

## Problem Statement

根據新版 `AGENTS.md` 第 5 節與第 6 節 Phase 1 規範，目前 MadEdit 存在兩個會造成資料毀損的已知問題與功能缺口：
1. **非 UTF-8 檔案存檔即損毀**：Big5 / GBK 等檔案若以錯誤編碼解碼產生替代字元（U+FFFD），存檔後原始內容無法復原。
2. **UTF-16 存檔會變成 UTF-8 且遺失 BOM**：`encoding_rs` 的 `Encoding::encode()` 對 UTF-16LE/BE 的輸出編碼為 UTF-8，且不會寫回 BOM，必須自行實作 UTF-16 編碼並保留 BOM。
3. **缺乏編碼選單與「重新解碼 / 轉換編碼」介面**：使用者無法在工具列選擇編碼以「以指定編碼重新讀取（Reinterpret Encoding）」或「轉換為指定編碼儲存（Convert Encoding）」。

## Solution

在核心層（`src/core/`）與 Slint UI 層（`ui/appwindow.slint`、`src/main.rs`）實作完整的多編碼支援與資料保護機制：
1. **支援 10 種常用字元編碼**：`UTF-8`、`UTF-16LE`、`UTF-16BE`、`Big5`、`GBK`、`GB18030`、`Shift_JIS`、`EUC-JP`、`EUC-KR`、`windows-1252`。
2. **開檔自動編碼偵測與 BOM / 解碼錯誤追蹤**：
   - 優先透過 `Encoding::for_bom` 辨識 UTF-8 / UTF-16LE / UTF-16BE BOM 並記錄 `has_bom`。
   - 無 BOM 時先以嚴格 UTF-8 驗證；若非合法 UTF-8 則自動 fallback 嘗試 `Big5` 解碼。
   - 記錄解碼過程是否發生無法無損解碼的錯誤（`has_decode_error`）。
3. **防資料毀損保護與 UTF-16 原生編碼**：
   - 當 `has_decode_error == true` 時，拒絕直接覆寫存檔（回傳 `InvalidData` 錯誤並在狀態列提示使用者先切換正確編碼或轉換編碼），避免默默將 `U+FFFD` 寫回磁碟造成損毀。
   - 自行實作 UTF-16LE / UTF-16BE 編碼邏輯，並在存檔或轉碼時正確寫出與保留 BOM。
4. **工具列編碼選單與操作按鈕**：
   - 提供編碼下拉選單（`ComboBox`）以及「重新解碼」、「轉換編碼」按鈕，並即時同步文字區、Hex 檢視區與狀態列。

## User Stories

1. As a user, I want the editor to detect UTF-8/UTF-16 BOM or fallback to Big5 when opening a file, and track whether the file had a BOM (`has_bom`) or decoding errors (`has_decode_error`), so that files open safely and accurately.
2. As a user, I want the editor to refuse silently overwriting a file when `has_decode_error` is true, so that opening a file with the wrong encoding never corrupts my original data on save.
3. As a user, I want UTF-16LE and UTF-16BE files to be encoded as true 16-bit code units with their BOM preserved on save or conversion, so that UTF-16 files are never silently turned into UTF-8.
4. As a user, I want to reinterpret the raw bytes using a selected encoding without marking the file as modified, so that I can view GB18030, Shift_JIS, or EUC-KR files without mojibake.
5. As a user, I want to convert the current text into a selected target encoding and mark the document as modified, so that I can transcode and save files across encodings.

## Implementation Decisions

- **支援編碼清單（`SUPPORTED_ENCODINGS`）**：提供 10 種常用編碼標籤常數陣列。
- **`DocumentBuffer` 新增狀態欄位**：
  - `has_bom: bool`：記錄開啟檔案是否帶有 BOM（或轉為 UTF-16LE/BE 時自動帶 BOM）。
  - `has_decode_error: bool`：記錄當前解碼是否產生替代字元（無法無損解碼）。
- **防毀損存檔檢查（`save_file`）**：
  - 若 `self.has_decode_error` 為 `true`，`save_file` 回傳 `std::io::ErrorKind::InvalidData`，防止覆寫損毀原始檔案。
- **UTF-16LE / UTF-16BE 編碼與 BOM 寫回（`encode_str`）**：
  - 對 `UTF_16LE` / `UTF_16BE` 使用 `str::encode_utf16()` 轉為 LE / BE 位元組，若 `has_bom` 為 `true`（UTF-16 轉碼時預設啟用 BOM），前置寫入 `0xFF, 0xFE` 或 `0xFE, 0xFF`。
  - 對 `UTF_8`，若原檔 `has_bom` 為 `true`，前置寫入 `0xEF, 0xBB, 0xBF`。
- **繁體中文註解與文件同步**：
  - 所有新增與修改的 Rust / Slint 程式碼註解皆使用 `zh-TW` 繁體中文並詳細說明設計意圖。
  - 完成後同步更新 `AGENTS.md` 第 5 節完成進度與已知問題。

## Testing Decisions

- **測試接縫（Seams）**：透過 `DocumentBuffer` 的公開介面（`load_file`、`save_file`、`reinterpret_with_encoding`、`convert_to_encoding`、`update_text`）進行測試。
- **關鍵測試案例**：
  1. 支援編碼清單、UTF-16LE BOM 開檔與存檔往返（確認寫回真實 UTF-16LE 位元組與 BOM，而非 UTF-8）。
  2. 無 BOM 之 Big5 檔案自動偵測與無損解碼驗證。
  3. 含非法位元組導致 `has_decode_error == true` 時，`save_file` 拒絕存檔以保護原始檔案不損毀；改用正確編碼 `reinterpret_with_encoding` 後即可正常存檔。
  4. `reinterpret_with_encoding` 與 `convert_to_encoding` 的狀態與位元組驗證。

## Out of Scope

- Phase 2 自繪編輯器元件（Custom Editor View）與 Phase 3 大檔案引擎。
