# Ticket 02: Slint UI 跳轉面板與 Hex Offset 跳轉支援

- **狀態**：completed
- **依賴**：01-core-line-editing-and-goto-operations.md

## 描述
在 Slint UI (`ui/appwindow.slint`) 中新增頂部內嵌式可收合跳轉面板，並在 `src/main.rs` 與 `DocumentBuffer` 實作字串解析（支援 `line`、`line:col`、Hex Offset）與跳轉回呼。

## 需求
1. **Slint UI 跳轉面板**：
   - 屬性：
     - `show-goto-panel: bool`（預設 false）
     - `goto-label-text: string`（預設 "移至行 (1 - ...):"）
     - `goto-input-text: string`
   - 回呼與函式：
     - `goto-confirmed(string)`：點擊跳轉或按 Enter 觸發
     - `goto-cancelled()`：點擊取消或按 Esc 觸發
   - UI 排版：
     - 與搜尋列風格一致的淺灰底色面板（`#f4f4f4`、圓角 4px、邊框 1px）。
     - 水平排列：標籤、輸入框（`LineEdit`，寬度 140px，自動聚焦）、跳轉按鈕、取消按鈕。
     - 點擊按鈕或按下 Enter 觸發 `goto-confirmed`，按下 Esc 觸發 `goto-cancelled`。
2. **文字模式跳轉字串解析**：
   - 支援輸入格式：
     - 單一數字，如 `42`：代表跳轉至第 42 行，欄位維持 1。
     - 包含欄位，如 `42:15` 或 `42, 15`：代表跳轉至第 42 行、第 15 欄。
   - 容錯防呆：過濾前後空白；若輸入包含非數字或非法格式，自動 fallback 至第 1 行或游標原行。
3. **十六進位模式 Offset 跳轉支援**：
   - 在 Hex 模式下，跳轉面板標籤顯示「移至位移 Offset (0x0 - 0x...):」。
   - 支援十六進位字串（如 `0x1A0` 或 `1A0`）與十進位數字解析。
   - `DocumentBuffer` 實作 `goto_offset(offset: usize)`：更新 Hex 檢視滾動位置，確保該 offset 的行出現在檢視頂部或視野內。
4. **面板生命週期**：
   - 跳轉完成後自動收合面板（`show-goto-panel = false`），並將畫面焦點帶回編輯器。
