# 0003: 基於固定字元格網視覺欄位的列塊模式與矩形選取架構

為了提供專業文字編輯器必備的列塊選取（Column / Block Selection）功能，我們決定採用基於固定字元格網（Character Cell Grid）視覺欄位（Visual Column）的二維矩形選取模型。

## Context

MadEdit 的核心特色之一是強大的「列塊編輯模式 (Column Mode)」。
在傳統串流模式（Stream Selection）下，選取是從起始行/字元連續跨行覆蓋至結束行/字元；而在列塊模式下，選取必須是嚴格的幾何矩形（二維區塊）。
如果僅以字元索引（Character Index）計算列塊寬度，遇上中英混排（全形 CJK 佔 2 欄寬）或 Tab（展開為 4 欄寬）時，各行的文字邊界將嚴重歪斜，無法維持直挺的矩形視覺對齊。

## Decision

1. **二維視覺欄位選取模型（Visual Rectangular Selection）**：
   - 矩形選取以二維座標 `(start_line, start_visual_col)` 與 `(end_line, end_visual_col)` 定義。
   - 選取區域的垂直範圍為 `[min_line, max_line]`，水平視覺範圍為 `[min_vcol, max_vcol]`。
2. **字元與視覺格網雙向映射**：
   - 利用 Phase 2 實作的 `char_col_to_visual_col` 與 `visual_col_to_char_col`，在每一行獨立將視覺範圍 `[min_vcol, max_vcol]` 轉換為精確的字元起訖切片。
   - 若短行文字長度小於 `min_vcol` 或落在區間內，視需要以空格虛擬補齊，維持矩形對齊。
3. **觸發與切換機制**：
   - 工具列或選單切換至 `EditMode::Column` 時，所有滑鼠拖曳與鍵盤選取預設為矩形列塊選取。
   - 在 `EditMode::Text` 模式下，按住 `Alt` 鍵拖曳滑鼠或 `Alt + Shift + 方向鍵` 亦可進行臨時矩形選取。
4. **列塊編輯與剪貼簿操作**：
   - **複製 (Ctrl+C)**：依序提取各行落於矩形內的子字串，以換行符串接送入剪貼簿。
   - **剪下 (Ctrl+X)**：提取各行矩形文字後刪除該區塊，保持各行縮排與對齊。
   - **貼上 (Ctrl+V)**：多行區塊文字對齊貼入；若每行文字數量與目標行數相符，逐行對齊插入。
   - **多行同步輸入與刪除**：打字時在選取矩形各行的起始位置同步插入字元；Backspace / Delete 同步刪除區塊。

## Consequences

- 徹底解決等寬字型中英日混排與 Tab 展開時矩形選取漂移問題，與原版 MadEdit / UltraEdit 行為體驗一致。
- 現有的自繪虛擬渲染（`VisibleLine` 之 `sel_start_x` 與 `sel_width`）天然支援每行不同起訖位置的高亮，與 UI 渲染層無縫對接。
