# Ticket 03: 行號欄書籤 UI 渲染與快捷鍵分派

- **狀態**：completed
- **依賴**：01-line-endings-conversion-core-and-ui.md, 02-editor-bookmarks-core-and-maintenance.md

## 描述
在 Slint UI 行號欄（Gutter）繪製書籤視覺標記，並在 `src/main.rs` 完整分派書籤快捷鍵。

## 需求
1. **Slint UI 行號欄書籤標記**：
   - 在 `ui/appwindow.slint` 的 `VisibleLine` 結構中加入 `has_bookmark: bool`。
   - 在左側行號欄（Gutter）為每一行繪製書籤標記：
     - 若 `line.has_bookmark`，在行號左方繪製半徑 4px 的藍色圓點（`#3b82f6`），垂直置中。
2. **快捷鍵分派**：
   - `Ctrl+F2`：切換書籤（`d.toggle_bookmark()`）
   - `F2`：跳至下一個書籤（`d.next_bookmark()`）
   - `Shift+F2`：跳至上一個書籤（`d.prev_bookmark()`）
   - `Ctrl+Shift+F2`：清除全部書籤（`d.clear_all_bookmarks()`）
3. **換行格式事件綁定**：
   - 綁定 `newline-format-changed(format_str)`：解析格式並調用 `d.convert_newline_format(fmt)`，更新 UI 與狀態列。
4. **程式碼審查與測試驗證**：
   - 確保所有測試零警告通過。
   - 執行雙軸代碼審查（Standards & Spec）。
