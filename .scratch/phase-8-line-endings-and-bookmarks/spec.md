Status: closed

# Phase 8: 換行格式轉換與書籤功能 (Line Endings Conversion & Bookmarks)

## Problem Statement

在跨平台檔案編輯與長代碼檢視中，目前 MadEdit-Rust 存在以下兩點限制：
1. **僅能偵測與顯示換行符號，無法主動轉換**：當使用者在 Windows 開啟了 LF 檔案想轉為 CRLF，或在 Unix/macOS 想將 CRLF 檔案轉為 LF 存檔時，編輯器缺乏轉換能力。
2. **缺乏行書籤導覽能力**：長篇檔案瀏覽時，無法標記感興趣的行或在多個關注點之間快速前後切換，影響閱讀效率。

## Solution

實作換行格式轉換與行書籤系統：

1. **換行格式切換與轉換（`src/core/model.rs` & `src/core/buffer.rs`）**：
   - `NewLineFormat` 擴充 `from_label(&str) -> Option<NewLineFormat>` 與 `SUPPORTED_NEWLINES`。
   - `DocumentBuffer::convert_newline_format(&mut self, new_format: NewLineFormat)`：
     轉換換行格式，以新換行符重建檔案內容並同步至 Piece Table，標記為已修改 `is_modified = true`。
   - 工具列提供換行格式切換下拉選單（`ComboBox`），狀態列同步顯示。

2. **編輯器行書籤核心（`src/core/editor.rs` & `src/core/buffer.rs`）**：
   - 在 `EditorState` 引入 `bookmarks: BTreeSet<usize>`。
   - 實作書籤切換（`toggle_bookmark`）、前後循環導覽（`next_bookmark`、`prev_bookmark`）與全部清除（`clear_all_bookmarks`）。
   - 在行移動、複製、插入與刪除操作中，即時維護書籤索引不變量。
   - `DocumentBuffer` 統一封裝書籤 API。

3. **Slint UI 視覺標記與快捷鍵整合（`ui/appwindow.slint` & `src/main.rs`）**：
   - 可見行資料結構擴展 `has_bookmark: bool`。
   - 在左側行號欄（Gutter）以藍色圓點醒目標示書籤所在行。
   - 工具列新增換行格式下拉選單。
   - 鍵盤快捷鍵分派：
     - `Ctrl+F2`：切換書籤
     - `F2`：跳至下一個書籤
     - `Shift+F2`：跳至上一個書籤
     - `Ctrl+Shift+F2`：清除全部書籤

## 使用者故事 (User Stories)

1. 身為使用者，當我開啟一個 Unix (LF) 的檔案時，我希望能在工具列或狀態列將換行格式切換為 CRLF (Windows)，編輯器會標記為已修改，並在存檔時以 CRLF 格式寫出。
2. 身為使用者，在閱讀長篇原始碼時，我希望按下 `Ctrl+F2` 可以在當前行加上藍色小圓點書籤；再次按下時移除該書籤。
3. 身為使用者，當我在不同段落加上多個書籤後，按下 `F2` 能依序跳至下一個書籤，按下 `Shift+F2` 能跳至上一個書籤，並能在文末與文首之間循環導覽。
4. 身為使用者，當我不再需要書籤時，按下 `Ctrl+Shift+F2` 能一鍵清除全檔所有書籤。
5. 身為使用者，當我在含書籤的程式碼中插入新行或刪除整行時，書籤標記能自動跟隨移動，不發生位移錯位。

## Implementation Tickets

1. `issues/01-line-endings-conversion-core-and-ui.md`：換行格式轉換核心邏輯、存檔同步與工具列/狀態列 UI 整合。
2. `issues/02-editor-bookmarks-core-and-maintenance.md`：書籤資料結構（BTreeSet）、切換、前後循環導覽、行編輯不變量維護與單元測試。
3. `issues/03-bookmarks-gutter-ui-and-shortcuts.md`：行號欄（Gutter）書籤藍點繪製、快捷鍵分派（Ctrl+F2, F2, Shift+F2, Ctrl+Shift+F2）與全功能整合。
