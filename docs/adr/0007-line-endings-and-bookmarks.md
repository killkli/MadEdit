# 0007: 換行格式轉換與編輯器行書籤架構

為了使 MadEdit-Rust 在跨平台協同開發與長代碼檢視中具備完整的行尾格式控制能力與快速標記導覽體驗，我們決定實作「換行格式切換與轉換（Line Endings Conversion）」與「編輯器行書籤系統（Editor Bookmarks）」。

## Context

在現代多平台文字編輯與程式碼維護中：
1. **換行格式多樣性**：Windows 採用 `CRLF (\r\n)`，Unix/Linux/macOS 採用 `LF (\n)`，舊版 Mac 採用 `CR (\r)`。跨平台協作時，開發者常需要檢視並將整份文件的行尾換行格式統一轉換。
2. **長篇代碼快速導覽**：閱讀成千上萬行的源代碼或 Log 時，開發者需要臨時標記重點行，並能一鍵在各個標記點之間快速穿梭跳轉。

## Decision

1. **換行格式切換與轉換 (Line Endings Conversion)**：
   - 擴充 `src/core/model.rs` 中的 `NewLineFormat`：
     - 提供 `SUPPORTED_NEWLINES` 常數清單。
     - 提供 `from_label(&str) -> Option<NewLineFormat>` 字串解析方法。
   - 在 `DocumentBuffer`（`src/core/buffer.rs`）中實作：
     - `convert_newline_format(&mut self, new_format: NewLineFormat)`：
       當換行格式變更時，更新 `self.newline_format`，將內容重新以新換行符重構並同步至底層 Piece Table 與位元組，標記文件為已修改 `is_modified = true`。
     - 存檔時統一依據當前 `newline_format` 寫出。
   - Slint UI 工具列 / 狀態列：
     - 工具列新增換行格式下拉選單（`ComboBox`），即時顯示並支援手動切換與轉換。
     - 底部狀態列即時同步當前換行格式名稱。

2. **編輯器行書籤系統 (Editor Bookmarks)**：
   - 在 `EditorState`（`src/core/editor.rs`）中引入 `bookmarks: BTreeSet<usize>`（0-based 行索引集合），天然具備有序性與無重複特性。
   - 提供核心 API：
     - `toggle_bookmark(&mut self) -> bool`：在游標當前行新增或移除書籤，回傳該行目前是否具有書籤。
     - `next_bookmark(&mut self) -> bool`：以當前行為起點，向後二分查找下一個書籤行；若已到底則環狀循環回第一個書籤，自動定位游標並捲動視窗。
     - `prev_bookmark(&mut self) -> bool`：向前二分查找上一個書籤行；若已到頂則環狀循環至最後一個書籤。
     - `clear_all_bookmarks(&mut self)`：清除所有書籤。
   - 行級編輯不變量維護：
     - 在行插入、整行上下移動、行複製與行刪除操作中，同步校正受影響行的書籤索引，避免書籤錯位或越界。
   - `DocumentBuffer` 統一封裝書籤操作並觸發 UI 同步。
   - 快捷鍵整合：
     - `Ctrl+F2`：切換書籤（Toggle Bookmark）。
     - `F2`：跳至下一個書籤（Next Bookmark）。
     - `Shift+F2`：跳至上一個書籤（Prev Bookmark）。
     - `Ctrl+Shift+F2`：清除全部書籤（Clear All Bookmarks）。
   - UI 視覺呈現：
     - 在 `VisibleLineData` 擴充 `has_bookmark: bool`。
     - 在左側行號欄（Gutter）中，若該行含有書籤，繪製醒目的藍色小圓點標記，視覺效果清晰自然。

## Consequences

- 滿足跨平台專案對換行一致性的嚴格要求（如 Git autocrlf 或 CI 檢查）。
- 賦予開發者快速定位與回訪代碼段落的強大能力。
- 基於 `BTreeSet` 實作的書籤系統時間複雜度為 $O(\log N)$，記憶體佔用極低，且與 Viewport 虛擬捲動完美相容。
