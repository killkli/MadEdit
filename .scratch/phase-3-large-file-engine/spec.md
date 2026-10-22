Status: ready-for-agent

# Phase 3: 大檔案與核心緩衝區最佳化 (Large File Engine)

## Problem Statement

目前 `DocumentBuffer::load_file` 將全檔一次讀入記憶體（`Vec<u8>`）並整份解碼為 `String`，每次編輯或存檔又整份重新編碼。這使得幾十 MB 以上的檔案在開啟時嚴重卡頓，1GB 檔案更是會耗用數 GB 記憶體甚至造成程式崩潰（OOM）。

## Solution

採用 **Piece Table（片段表）** 搭配 **`memmap2` 唯讀記憶體對映** 與 **LineIndex 行索引按需解碼** 架構：
1. **Piece Table 核心**：
   - `Original Buffer`：開啟實體檔案時使用 `memmap2::Mmap` 零拷貝對映，小檔或新建檔案使用唯讀位元組切片。
   - `Add Buffer`：儲存所有編輯新增的位元組序列，僅向前追加。
   - `Piece(source, offset, length)`：片段清單，支援任意位移的隨機讀取、插入與刪除。
2. **LineIndex 與按需解碼**：
   - 記錄每行起始位元組位移（`Vec<usize>`），支援二分搜尋快速定位指定行。
   - 自繪編輯器（Phase 2）請求可視行時，僅從 Piece Table 擷取該數十行的位元組片段並以當前編碼即時解碼為文字傳給 UI。
3. **無損存檔與串流寫出**：
   - 存檔時直接由 Piece 清單將位元組循序串流寫入磁碟暫存檔再原子替換，避免在 RAM 中分配全檔記憶體。

## User Stories

1. As a user, I want to open large files (100MB+) in milliseconds without UI freezing, so that I can immediately start reading and editing.
2. As a user, I want the editor's memory usage to remain lightweight (a few megabytes) regardless of file size, so that opening huge files does not exhaust system RAM.
3. As a user, I want to scroll smoothly to any line in a large file, so that the editor displays visible lines on-demand without lagging.
4. As a user, I want to insert, delete, and modify text in a large file, and have the changes preserved accurately when saving, so that large file editing is rock solid.
5. As a user, I want the Hex Dump view to work seamlessly with large files by reading slices directly from the Piece Table, so that hex inspection remains fast.

## Implementation Decisions

- **外部依賴**：引入 `memmap2 = "0.9"`。
- **模組結構**：在 `src/core/piece_table.rs` 中實作核心 `PieceTable` 與 `LineIndex`。
- **`DocumentBuffer` 整合**：
  - 將原有的 `raw_bytes: Vec<u8>` 與整檔 `text: String` 轉為以 `piece_table: PieceTable` 作為唯一資料真相來源（Single Source of Truth）。
  - 對於相容性需求（如小型文字檔案或測試），提供 `text()` 與 `raw_bytes()` 查詢介面。
  - `get_hex_dump` 直接透過 Piece Table 隨機讀取前 1MB，無任何記憶體浪費。

## Testing Decisions

- **測試接縫 (Seams)**：
  1. `PieceTable` 獨立單元測試：連續插入、中間刪除、跨 Piece 刪除、位移隨機讀取、串流輸出位元組正確性。
  2. `LineIndex` 單元測試：換行符識別、行號與位移互查、按需擷取行內容。
  3. `DocumentBuffer` 大檔模擬測試：開啟 10MB 虛擬/實體檔案，驗證秒級開啟、指定行隨機存取、編輯與存檔完整性。

## Out of Scope

- 列塊模式的矩形選取（Phase 4）。
- 語法著色 Tokenizer（Phase 6）。
