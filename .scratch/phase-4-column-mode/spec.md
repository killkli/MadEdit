Status: ready-for-agent

# Phase 4: 列塊模式 (Column/Block Mode) 與矩形選取

## Problem Statement

目前 `EditorState` 的選取模型僅支援一維連續文字流（Stream Selection）。當使用者切換至「列塊模式 (Column Mode)」時，選取行為與文字模式毫無差異。在處理對齊文字、多行註解、二進位日誌或排版表格時，使用者無法垂直選取矩形區塊進行批量編輯、複製或刪除。此外，若無基於固定字元格網的視覺欄位（Visual Column）計算，中英混排與 Tab 縮排會導致列塊選取歪斜漂移。

## Solution

採用**基於字元格網視覺欄位的二維矩形選取模型（Visual Block Selection）**，無縫串聯 Phase 2 的自繪格網與 Phase 3 的大檔案按需行供應架構：

1. **核心選取模型擴充**：
   - 擴充 `Selection`，支援選取類型（`SelectionKind::Stream` 與 `SelectionKind::Block`）。
   - 矩形選取記錄垂直起訖行 `(anchor.line, head.line)` 與水平視覺欄位 `(anchor_vcol, head_vcol)`。
2. **多行可見行矩形渲染**：
   - 在 `generate_visible_lines_with_text` 中，針對落於選取垂直範圍 `[min_line, max_line]` 的每一行，計算該行落於視覺範圍 `[min_vcol, max_vcol]` 內的 `sel_start_x` 與 `sel_width`。
   - 每行游標與反白精準繪製於 Slint UI。
3. **列塊剪貼簿與文字操作**：
   - **選取文字提取（Copy）**：提取各行落在視覺欄位內的字元切片，以 `\n` 連接送入剪貼簿。
   - **列塊刪除（Cut / Delete / Backspace）**：對每一行落於視覺欄內的字元區段進行切除，若短行不超出則安全略過。
   - **多行同步打字與插入**：若選取為多行矩形或垂直游標線，打字時在選取範圍的各行同步插入字元。
   - **列塊貼上（Paste）**：多行剪貼簿文字逐行對齊貼入目標行。
4. **UI 觸發與 Alt 快捷鍵支援**：
   - 工具列模式切換至 `EditMode::Column`（模式 1）時，預設觸發矩形選取。
   - 在 `EditMode::Text` 模式下，按住 `Alt` 鍵拖曳滑鼠或 `Alt + Shift + 方向鍵` 亦觸發矩形選取。
   - Slint UI 回傳滑鼠拖曳與按鍵事件中的 `alt` 鍵修飾狀態。

## User Stories

1. As a user, I want to select a 2D rectangular block of text across multiple lines when in Column Mode or when holding Alt in Text Mode, so that I can manipulate columnar data.
2. As a user, I want the rectangular selection to align strictly on visual columns (accounting for full-width CJK characters and 4-space tabs), so that the selection boundary never distorts horizontally.
3. As a user, I want to copy (Ctrl+C) and cut (Ctrl+X) the rectangular block, so that only text within the 2D bounding box is extracted to the clipboard.
4. As a user, I want to paste (Ctrl+V) multi-line columnar text into the editor, aligning seamlessly across the target lines.
5. As a user, I want to type characters while a column block or vertical caret line is active, so that the typed text is inserted simultaneously on every line in the block.
6. As a user, I want Backspace and Delete to remove the text inside the rectangular selection across all selected lines in one action.
7. As a user, I want lines shorter than the column selection rectangle to be handled safely without program crashes or out-of-bounds panics.

## Implementation Decisions

- **選取模型定義**：
  - 定義 `SelectionKind` 列舉（`Stream`, `Block`）。
  - `Selection` 結構新增 `kind: SelectionKind`。
  - 提供輔助方法：`is_block(&self) -> bool`、`visual_bounds(&self, editor: &EditorState) -> (usize, usize, usize, usize)` 傳回 `(min_line, max_line, min_vcol, max_vcol)`。
- **行文字與視覺欄位互轉**：
  - 利用現有的 `char_col_to_visual_col` 與 `visual_col_to_char_col`。
  - 新增行字元切片輔助函式 `extract_visual_slice(&str, min_vcol, max_vcol) -> (String, usize, usize)` 傳回落於視覺區間的子字串以及對應的字元起訖索引。
- **UI 事件傳遞修飾鍵**：
  - Slint `appwindow.slint` 的編輯區 `TouchArea` 與 `FocusScope` 傳遞 `alt: bool` 參數至 Rust 後端。

## Testing Decisions

- **測試接縫 (Seams)**：
  1. **字元格網視覺切片測試**：英文、中英文混排、Tab 展開下的 `extract_visual_slice` 精確度測試。
  2. **矩形選取模型與導覽測試**：方向鍵擴展矩形選取、視覺起訖計算、短行處理測試。
  3. **列塊剪貼簿與編輯測試**：列塊複製（Ctrl+C）、剪下（Ctrl+X）、貼上（Ctrl+V）、多行同步打字、Backspace/Delete 區塊刪除測試。
  4. **大檔案按需可見行渲染測試**：大檔案在列塊模式下 `generate_visible_lines` 矩形反白資料產生正確性測試。

## Out of Scope

- 搜尋與取代（Phase 5）。
- 語法突顯（Phase 6）。
