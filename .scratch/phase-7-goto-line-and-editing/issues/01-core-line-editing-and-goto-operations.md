# Ticket 01: 核心行編輯操作與跳轉邏輯實作

- **狀態**：completed
- **依賴**：無

## 描述
在 `src/core/editor.rs` 與 `src/core/buffer.rs` 中實作核心行編輯操作與跳轉函式，並透過單元測試先行（TDD）驗證所有邊界條件。

## 需求
1. **指定跳轉（Go to Line & Column）**：
   - 實作 `EditorState::goto_line_col(&mut self, line: usize, col: usize)`：
     - `line` 與 `col` 為 0-based。
     - 若 `line >= lines.len()`，自動 Clamp 至 `lines.len() - 1`；若 `col > line_char_count(line)`，Clamp 至該行長度。
     - 清除選取（設為 caret）。
     - 滾動至游標可見（`scroll_caret_into_view()`）。
   - 在 `DocumentBuffer` 封裝 `goto_line_col(&mut self, line_1based: usize, col_1based: usize)`，提供以 1-based 為參數的介面。
2. **整行上下移動（Move Lines Up / Down）**：
   - 實作 `EditorState::move_lines_up(&mut self)`：
     - 若無選取，移動當前游標所在行：若 `line == 0` 則不作任何動作；若 `line > 0`，將 `line` 與 `line - 1` 兩行交換，游標 line 減 1，col 保持（或 clamp 至新行長度）。
     - 若有跨行選取，將選取範圍 `min_line..=max_line` 整塊上移（若 `min_line > 0`），選取與游標相對行號同步調整。
   - 實作 `EditorState::move_lines_down(&mut self)`：
     - 若無選取，移動當前游標所在行：若 `line + 1 >= lines.len()` 則不動作；若未到最後一行，將 `line` 與 `line + 1` 交換，游標 line 加 1，col 保持。
     - 若有跨行選取，將選取範圍整塊下移（若 `max_line + 1 < lines.len()`）。
3. **整行複製（Duplicate Lines）**：
   - 實作 `EditorState::duplicate_lines(&mut self)`：
     - 若無選取：複製當前行字串，插入至 `line + 1`，游標移動到新插入行 `line + 1`，col 保持。
     - 若有選取：複製選取區涵蓋的所有行（`min_line..=max_line`），整批插入至 `max_line + 1` 下方，游標選取區同步延伸或移至新複製塊。
4. **整行刪除（Delete Lines）**：
   - 實作 `EditorState::delete_lines(&mut self)`：
     - 刪除當前行（或選取涵蓋的多行）。
     - 若全檔所有行被刪除（`lines.is_empty()`），自動補入一行空字串 `vec![String::new()]`，避免系統 panic。
     - 游標停留在原行位置（若超出新的末行則停在新的末行），col 自動 clamp。
5. **智慧行首（Smart Home）**：
   - 實作 `EditorState::move_home_smart(&mut self, expand_selection: bool)`：
     - 尋找當前行第一個非空白字元之索引 `first_non_whitespace`。
     - 若當前游標位置大於 `first_non_whitespace`，則移動游標至 `first_non_whitespace`。
     - 若當前游標已經在 `first_non_whitespace` 或行內全為空白，則移動游標至欄位 0。
6. **測試涵蓋**：
   - 在 `editor.rs` 與 `buffer.rs` 中撰寫完整的單元測試，涵蓋首行、末行、空檔案、多行選取移動/複製/刪除、縮排智慧行首與跳轉 Clamp。
