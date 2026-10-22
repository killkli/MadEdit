Status: closed

# Issue 01: 擴充 SelectionKind 與視覺欄位矩形選取領域模型

## Description

為 `EditorState` 與 `Selection` 引入 `SelectionKind`（`Stream` 與 `Block`），並實作基於固定字元格網視覺欄位（Visual Column）的二維矩形模型計算。

## Tasks

1. 定義 `SelectionKind` 列舉（`Stream`, `Block`）。
2. 在 `Selection` 中加入 `kind: SelectionKind`，預設為 `Stream`。
3. 實作 `extract_visual_slice(line: &str, min_vcol: usize, max_vcol: usize) -> (String, usize, usize)`，傳回落在視覺欄位區間內的字串與字元索引範圍 `(char_start, char_end)`。
4. 實作 `Selection::visual_bounds` 計算矩形起訖 `(min_line, max_line, min_vcol, max_vcol)`。
5. 更新 `generate_visible_lines_with_text`：當 `kind == SelectionKind::Block` 時，各可見行依 `[min_vcol, max_vcol]` 獨立計算 `sel_start_x` 與 `sel_width`。
6. 單元測試覆蓋：純英文、全形 CJK、Tab 展開混排下的視覺切片提取與矩形渲染正確性。
