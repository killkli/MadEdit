Status: ready-for-agent

# Phase 2: 自繪編輯器元件 (Custom Editor View)

## Problem Statement

Slint 內建的 `TextEdit` 元件無法取得與控制內部的游標座標、無法取得與繪製自訂選取範圍、亦不支援僅繪製可見行的虛擬捲動。這使得後續的 Phase 3（GB 級大檔案）、Phase 4（列塊/矩形選取）與 Phase 6（語法突顯）皆無法在 `TextEdit` 上推進。此外，不同作業系統在渲染西文等寬字型搭配 CJK 中文字元時，字元寬度比非嚴格 2:1，若無自訂字元格網將導致游標定位漂移與列塊無法垂直對齊。

## Solution

在核心層實作 `TextPosition`、`Selection`、`Viewport` 與 `Character Cell Grid`（字元格網視覺欄寬計算），並在 Slint UI 層以原生 `FocusScope` + `TouchArea` + 宣告式可見行列表取代內建 `TextEdit`：
1. **核心領域模型與文字操作**：
   - 定義 `TextPosition`、`Selection` 與 `Viewport`。
   - 實作字元欄寬計算（ASCII 1 欄、CJK 全形 2 欄、Tab 展開對齊至 4 欄倍數）。
   - 實作完整的游標移動、選取擴展、字元插入/刪除（Backspace/Delete/Enter/Tab）與剪貼簿（複製/剪下/貼上）。
2. **可見行切片產生與虛擬捲動**：
   - 核心根據 `Viewport` 產出目前可視行的行號、視覺內容、選取反白區間與當前行游標水平像素偏移。
   - 游標超出視窗時自動平滑捲動（Scroll into view）。
3. **Slint UI 整合與狀態列即時同步**：
   - 在 Slint 繪製行號欄（Gutter）、行文字、選取反白矩形與自繪閃爍游標（Caret）。
   - 捕捉方向鍵、Shift 選取、滑鼠點擊/拖曳與快捷鍵（Ctrl+A, Ctrl+C, Ctrl+V, Ctrl+X）。
   - 狀態列即時顯示游標位置 `Ln X, Col Y`（1-based 行號與欄號）。

## User Stories

1. As a user, I want to see a line-numbered gutter on the left side of the editor, so that I can easily track line locations in my document.
2. As a user, I want to navigate the caret using arrow keys, Home, End, PageUp, and PageDown, so that I can smoothly move around the document.
3. As a user, I want the viewport to automatically scroll when the caret moves off-screen, so that the editing point is always visible.
4. As a user, I want to select text using Shift+arrow keys or by clicking and dragging with the mouse, so that I can highlight and manipulate ranges of text.
5. As a user, I want CJK full-width characters and tabs to align strictly on a character cell grid, so that the cursor never drifts horizontally regardless of system fonts.
6. As a user, I want to type characters, press Enter for new lines, Backspace/Delete to remove text, and Tab to indent, so that I have a fully functional text editing experience.
7. As a user, I want to use standard clipboard shortcuts (Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+A), so that I can copy, cut, paste, and select all text.
8. As a user, I want the status bar to show the real-time caret position `Ln X, Col Y`, so that I know exactly where my cursor is.

## Implementation Decisions

- **資料結構分離**：
  - 文件內容以行清單（`lines: Vec<String>`）快取或動態分割，保持與底層 `text` / `raw_bytes` 同步。
  - `TextPosition { line: usize, col: usize }`，其中 `col` 為該行字元（Unicode char）索引。
  - `Selection { anchor: TextPosition, head: TextPosition }`。
  - `Viewport { first_visible_line: usize, visible_line_count: usize, first_visible_col: usize }`。
- **字元格網（Character Cell Grid）**：
  - 提供輔助函式 `char_visual_width(c: char) -> usize`（全形 2，半形 1）與 `char_col_to_visual_col(&line, char_idx) -> usize`（處理 Tab 展開至 4 欄倍數）。
  - 將視覺欄寬轉換為像素偏移：`pixel_x = visual_col * cell_width`。
- **可見行傳遞給 Slint**：
  - 定義 Slint 結構 `struct VisibleLine { line_number: int, text: string, has_selection: bool, sel_start_x: length, sel_width: length, has_caret: bool, caret_x: length }`。
  - 每次狀態變更或捲動時，只產出視窗可容納的行數（預設約 30~50 行）。

## Testing Decisions

- **測試接縫（Seams）**：在核心編輯狀態模型（`EditorState` 或擴充後的 `DocumentBuffer`）的公開 API 上進行 TDD 測試。
- **關鍵測試**：
  1. `char_visual_width` 與 Tab 展開之視覺欄寬計算正確性（ASCII、中文、Tab 混排）。
  2. 游標移動（上下左右、行首行尾、文件首尾）與邊界防護。
  3. 選取範圍建立、擴展與反向選取正規化。
  4. 編輯操作（插入字元、Enter 換行、Backspace、Delete、選取區替換刪除）。
  5. 剪貼簿操作與全選邏輯。

## Out of Scope

- 列塊模式的矩形反白（Phase 4）。
- GB 級大檔分塊 Piece Table 引擎（Phase 3）。
- 語法著色 Tokenizer（Phase 6）。
