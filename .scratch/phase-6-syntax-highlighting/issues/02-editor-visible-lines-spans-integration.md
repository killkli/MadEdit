Status: ready-for-agent

# Issue 02: 編輯器可見行分段色彩結構整合

## Description

在 `src/core/editor.rs` 與 `src/core/buffer.rs` 整合語法突顯，使可見行切片產生時能夾帶 `HighlightSpan` 資料，同時保持固定字元格網與矩形選取不受影響。

## Tasks

1. 修改 `VisibleLineData`：新增欄位 `pub spans: Vec<HighlightSpan>`。
2. 在 `EditorState` 調整可見行產生方法，支援傳入可選的語法分段提供者或在 `DocumentBuffer` 處組裝。
3. 在 `DocumentBuffer` 中增加：
   - 語法引擎執行個體（或透過全域靜態 / 延遲初始化快取避免重複建立）。
   - 當前語言名稱屬性：`pub current_syntax: String`。
   - `set_syntax(&mut self, syntax_name: &str)` 手動切換語言。
   - 在開檔 `load_file` 時自動呼叫 `detect_syntax` 設定語言。
   - 更新 `generate_visible_lines`：將可見行文字進行語法分析，轉換為 `spans` 填入 `VisibleLineData`。
4. 單元測試覆蓋：
   - 可見行資料包含精確的 `spans`。
   - 列塊模式下矩形幾何與語法高亮並存測試。
