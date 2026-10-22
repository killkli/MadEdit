Status: ready-for-agent

# Issue 03: Slint 內嵌式搜尋取代面板與快捷鍵

## Description

在 `ui/appwindow.slint` 設計內嵌式搜尋與取代面板，於 `src/main.rs` 綁定快捷鍵（Ctrl+F、Ctrl+H、Esc、F3、Shift+F3）與即時匹配計數（如 `3/15`）。

## Tasks

1. 修改 `ui/appwindow.slint`：在自繪編輯區上方加入條件展開的搜尋/取代面板。
2. 面板包含：搜尋輸入框、取代輸入框、選項核取（大小寫/全字/Regex）、前一個/後一個按鈕、取代/全部取代按鈕、關閉按鈕與匹配進度標籤。
3. 在 `src/main.rs` 處理快捷鍵：
   - `Ctrl+F`：顯示搜尋列。
   - `Ctrl+H`：顯示搜尋與取代列。
   - `Esc`：關閉搜尋列。
   - `F3` / `Enter`：尋找下一個。
   - `Shift+F3` / `Shift+Enter`：尋找上一個。
4. 狀態列與面板即時同步當前選取匹配索引與總數。
