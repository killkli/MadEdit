# Issue 2: Slint 頂部內嵌警告橫幅 UI、定時器排程與事件回呼綁定

## 狀態
closed

## 目標
在 Slint UI 與 `main.rs` 中實作頂部警告橫幅、週期性排程偵測以及重新載入/忽略回呼綁定。

## 工作項目
1. 在 `ui/appwindow.slint` 宣告 `has-external-change`、`reload-file`、`ignore-external-change`。
2. 實作醒目的琥珀黃色頂部通知橫幅，包含未儲存警示與操作按鈕。
3. 在 `src/main.rs` 設定 `slint::Timer` 週期檢查。
4. 實作 `reload-file` 與 `ignore-external-change` 回呼，同步緩衝區狀態至 Slint UI。
5. 在檔案開啟與儲存操作時確保重設橫幅狀態。
