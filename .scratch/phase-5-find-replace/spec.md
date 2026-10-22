Status: ready-for-agent

# Phase 5: 搜尋與取代 (Find & Replace)

## Problem Statement

目前編輯器無法在已開啟的檔案中快速定位文字關鍵字，亦無法進行批量文字取代。在長文件或大檔案中尋找資訊極為困難，且在十六進位模式下亦無法透過十六進位位元組特徵（Hex Byte Pattern）檢索二進位特徵碼。

## Solution

實作核心搜尋引擎（`SearchEngine`）與 Slint 內嵌式可收合面板：

1. **核心搜尋引擎（`src/core/search.rs`）**：
   - 支援四種搜尋模式：一般文字、區分大小寫（Match Case）、全字匹配（Whole Word）、正規表達式（Regex）。
   - 支援搜尋導覽：`find_next`、`find_prev`、`count_matches`。
   - 支援取代操作：單次取代（`replace`）、全部取代（`replace_all`），並精確統計取代計數。
   - 支援十六進位模式 Hex Byte 搜尋（解析 `48 65 6C 6C 6F` 並在 `PieceTable` 位元組串流中比對）。
2. **編輯器整合與自動選取定位**：
   - 當找到匹配項目時，自動更新編輯器的 `Selection`，高亮選中匹配範圍並平滑捲動視窗至該行（`scroll_caret_into_view`）。
   - 取代時同步更新底層文件緩衝區並標記為已修改。
3. **Slint 內嵌式面板與快捷鍵綁定**：
   - 編輯區上方嵌入可收合面板，包含搜尋框、取代框、匹配統計（如 `3/15` 或 `查無結果`）。
   - 支援快捷鍵：
     - `Ctrl+F`：開啟搜尋列並聚焦。
     - `Ctrl+H`：開啟搜尋與取代列並聚焦。
     - `Esc`：關閉搜尋列。
     - `F3` / `Enter`：尋找下一個。
     - `Shift+F3` / `Shift+Enter`：尋找上一個。

## User Stories

1. As a user, I want to press Ctrl+F to open an inline search bar, type a keyword, and see matches highlighted, so that I can easily find content.
2. As a user, I want to toggle Match Case, Whole Word, and Regular Expressions, so that I have advanced query flexibility.
3. As a user, I want to navigate between matches using F3 (Next) and Shift+F3 (Previous), so that I can step through results sequentially.
4. As a user, I want to press Ctrl+H to open the replace bar, and replace single or all occurrences with instant feedback on replacement counts.
5. As a user, I want to search for hex byte sequences (e.g., `48 65 6C 6C 6F`) while in Hex Mode, so that I can locate binary signatures.
6. As a user, I want the search panel to show real-time match indices (e.g. `2/10`), so that I know where I am in the document.

## Implementation Decisions

- **外部依賴**：在 `Cargo.toml` 加入 `regex = "1.11"`。
- **搜尋選項定義**：
  ```rust
  pub struct SearchOptions {
      pub match_case: bool,
      pub whole_word: bool,
      pub use_regex: bool,
  }
  ```
- **匹配結果定義**：
  ```rust
  pub struct SearchMatch {
      pub start: TextPosition,
      pub end: TextPosition,
      pub byte_offset: usize,
      pub byte_len: usize,
  }
  ```
- **Hex 模式匹配**：提供 `parse_hex_query(query: &str) -> Result<Vec<u8>, String>` 解析十六進位字串。

## Testing Decisions

- **測試接縫 (Seams)**：
  1. `SearchEngine` 文字比對測試：大小寫匹配、全字邊界匹配、正規表達式捕獲與替換。
  2. `find_next` 與 `find_prev` 環狀循環搜尋測試。
  3. `replace_all` 完整性測試與取代計數驗證。
  4. Hex Byte 序列搜尋解析與位元組匹配測試。
  5. `DocumentBuffer` 整合搜尋與自動選取滾動測試。

## Out of Scope

- 語法突顯（Phase 6）。
- 多檔案全專案搜尋（Phase 7+）。
