# Ticket 03: 鍵盤快捷鍵分派與編輯功能全面整合

- **狀態**：completed
- **依賴**：02-ui-goto-panel-and-hex-offset.md

## 描述
在 `src/main.rs` 的鍵盤事件分派邏輯中，完整整合跳轉面板快捷鍵（`Ctrl+G`）、行操作快捷鍵（`Alt+Up`、`Alt+Down`、`Ctrl+Shift+D`、`Ctrl+Shift+K`）、智慧行首（`Home`），並保證與搜尋面板、列塊模式與語法突顯平滑協同。

## 需求
1. **`Ctrl+G` 快捷鍵處理**：
   - 捕捉 `Ctrl+G`：
     - 若跳轉面板已開啟，則關閉；若未開啟，則展開。
     - 若搜尋面板正開啟，自動將搜尋面板收合，避免多面板佔用編輯視野。
     - 文字模式下：計算總行數與當前行號，設定標籤文字為 `移至行 (1 - {total_lines}):`，輸入框預設帶入當前游標行號（例如 `12`）。
     - Hex 模式下：計算最大 byte 位移，設定標籤文字為 `移至位移 Offset (0x0 - 0x{max_offset:X}):`，輸入框預設帶入當前預覽位移。
2. **行編輯快捷鍵處理**：
   - `Alt+Up`：呼叫 `d.editor.move_lines_up()`，並執行 `d.sync_from_editor()`。
   - `Alt+Down`：呼叫 `d.editor.move_lines_down()`，並執行 `d.sync_from_editor()`。
   - `Ctrl+Shift+D`：呼叫 `d.editor.duplicate_lines()`，並執行 `d.sync_from_editor()`。
   - `Ctrl+Shift+K`：呼叫 `d.editor.delete_lines()`，並執行 `d.sync_from_editor()`。
3. **智慧行首導覽（Smart Home）**：
   - 攔截 `Home` 鍵（含 `Shift+Home`），呼叫 `d.editor.move_home_smart(shift)` 取代原先直接跳至 0 欄的行為。
4. **狀態列與 UI 同步**：
   - 行操作後，即時呼叫 `sync_ui_from_doc(&ui, &d)`，更新狀態列長度、修改標記、可見行資料與游標位置。
5. **程式碼審查與測試驗證**：
   - 確保所有單元測試與回歸測試通過。
   - 執行雙軸代碼審查（Standards & Spec）。
