# Ticket 01: 換行格式轉換核心邏輯、存檔同步與 UI 整合

- **狀態**：completed
- **依賴**：無

## 描述
在 `src/core/model.rs` 與 `src/core/buffer.rs` 中實作換行格式轉換邏輯，並在 Slint UI 工具列新增換行格式切換選單。

## 需求
1. **`NewLineFormat` 擴充**：
   - 新增常數 `SUPPORTED_NEWLINES: [&str; 3] = ["CRLF (Windows)", "LF (Unix/macOS)", "CR (Classic Mac)"]`。
   - 實作 `from_label(label: &str) -> Option<NewLineFormat>`：支援完整名稱或簡稱（"CRLF", "LF", "CR"）。
2. **`DocumentBuffer::convert_newline_format` 實作**：
   - 接收目標 `new_format: NewLineFormat`。
   - 若與目前相同則不做多餘動作。
   - 若不同：
     - 更新 `self.newline_format = new_format`。
     - 以 `new_format.newline_str()` 重新拼裝最新文字：`self.text = self.editor.to_text(new_format.newline_str())`。
     - 標記 `self.is_modified = true`。
     - 同步至 `self.raw_bytes`、`self.piece_table` 與 `self.line_index`（呼叫 `sync_bytes_to_piece_table()`）。
3. **Slint UI 整合**：
   - 在 `ui/appwindow.slint` 新增屬性：
     - `in-out property <[string]> newline-list: ["CRLF (Windows)", "LF (Unix/macOS)", "CR (Classic Mac)"];`
     - 回呼 `callback newline-format-changed(string);`
   - 工具列中加入 `ComboBox` 選擇換行格式（緊鄰編碼選單）。
   - 底部狀態列即時同步當前選取之換行格式名稱。
4. **單元測試**：
   - 測試換行格式轉換（CRLF 轉 LF、LF 轉 CRLF、轉 CR）。
   - 測試轉換後存檔寫出的二進位位元組是否符合預期換行符號。
