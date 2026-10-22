# Ticket 02: 編輯器行書籤核心資料結構與編輯維護

- **狀態**：completed
- **依賴**：無

## 描述
在 `src/core/editor.rs` 中引入書籤資料結構，實作切換、前後循環跳轉與全清除功能，並在行編輯操作中維護書籤不變量。

## 需求
1. **資料結構**：
   - `EditorState` 增加 `pub bookmarks: std::collections::BTreeSet<usize>`。
2. **核心書籤方法**：
   - `toggle_bookmark(&mut self) -> bool`：若當前游標所在行已有書籤則移除並回傳 false；否則加入書籤並回傳 true。
   - `next_bookmark(&mut self) -> bool`：向後尋找大於當前行的第一個書籤；若無則循環至第一個書籤。若找到，游標移動到該行（列號維持 0），自動滾動視窗並回傳 true；若無任何書籤則回傳 false。
   - `prev_bookmark(&mut self) -> bool`：向前尋找小於當前行的最後一個書籤；若無則循環至最後一個書籤。
   - `clear_all_bookmarks(&mut self)`：清空所有書籤。
3. **行編輯不變量維護**：
   - `insert_newline()`：若在 line 處換行，所有行號大於 line 的書籤行號全部加 1。
   - `delete_lines()`：被刪除範圍內的書籤移除，大於刪除範圍的書籤行號扣減刪除行數。
   - `move_lines_up()` 與 `move_lines_down()`：交換行時，書籤行號跟隨正確更新。
   - `duplicate_lines()`：被複製行後方的書籤行號相應往後偏移。
4. **可見行繪製資料**：
   - `VisibleLineData` 新增 `pub has_bookmark: bool` 欄位。
   - 在產生可見行時，判斷該行（0-based）是否存在於 `self.bookmarks`。
5. **`DocumentBuffer` 封裝**：
   - 在 `DocumentBuffer` 封裝 `toggle_bookmark`, `next_bookmark`, `prev_bookmark`, `clear_all_bookmarks`。
6. **單元測試**：
   - 撰寫完整的單元測試覆蓋書籤增刪、環狀跳轉、整行移動/複製/刪除時書籤座標自動校正。
