Status: ready-for-agent

# Issue 02: 搜尋導覽定位與取代操作

## Description

在 `EditorState` 與 `DocumentBuffer` 整合搜尋定位（`find_next`, `find_prev`）、選取高亮跳轉與單次/全部取代（`replace`, `replace_all`）。

## Tasks

1. 實作 `find_next` 與 `find_prev`（支援文件首尾環狀循環查找）。
2. 當搜尋到結果時，自動更新編輯器的 `Selection` 並呼叫 `scroll_caret_into_view`。
3. 實作單次取代 `replace_current` 與全部取代 `replace_all`，回傳精確取代次數。
4. 取代後自動調用 `sync_from_editor` 同步底層 `PieceTable`。
5. 單元測試覆蓋：環狀循環查找、取代次數驗證、Regex 分組替換。
