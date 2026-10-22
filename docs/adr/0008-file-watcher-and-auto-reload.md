# 0008: 外部檔案變更監控與自動/手動重新載入架構

為了使 MadEdit-Rust 在多程式協同作業（例如與 Git、外部腳本或第三方工具並行）時，能即時察知檔案在磁碟上的變更並防止覆寫衝突，我們決定實作「外部檔案變更監控（File Watcher）」與「非侵入式頂部通知與重新載入（Banner & Reload）」。

## Context

在日常開發情境中：
1. **外部程式修改**：檔案常在編輯器之外被變更（例如執行 `git checkout`、`git pull`、編譯器產生輸出、格式化工具或另一款編輯器儲存）。
2. **覆寫風險與資料遺失**：若編輯器未能察覺磁碟檔案已變更，使用者若隨手按下 `Ctrl+S`，會將舊內容直接覆寫磁碟上的最新版本，造成資料遺失。
3. **KISS 原則與跨平台穩定性**：
   - 傳統基於檔案系統事件的監控庫（如 `notify`）在跨平台（尤其 Windows）上容易遇到檔案寫入鎖競爭、多次重複觸發（需要複雜 debounce）以及背景多執行緒與 Slint UI 事件迴圈同步之繁複度。
   - 經評估，基於檔案中繼資料時間戳記（`mtime`）定期檢查與焦點檢查的方案，零外部依賴、純標準庫，極致簡潔且完全消除鎖競爭風險。

## Decision

1. **核心時間戳記比對機制 (`DocumentBuffer`)**：
   - 在 `DocumentBuffer`（`src/core/buffer.rs`）中記錄 `last_modified: Option<std::time::SystemTime>`。
   - 當呼叫 `load_file` 開啟檔案或 `save_file` 儲存檔案成功後，同步更新 `last_modified = fs::metadata(&path).and_then(|m| m.modified()).ok()`。
   - 提供核心檢測 API：
     - `check_external_change(&self) -> bool`：比對當前磁碟上的 `modified()` 時間是否大於 `last_modified`。若大於則回傳 `true`。
     - `reload_file(&mut self) -> std::io::Result<()>`：
       - 重新自磁碟載入檔案內容（自動繼承編碼與語法識別）。
       - 盡可能保留使用者的游標行欄位置與書籤集合（自動 Clamp 防越界）。
       - 更新 `last_modified` 並重置 `is_modified = false`。
     - `ignore_external_change(&mut self)`：
       - 將 `last_modified` 更新為磁碟當前時間戳記，避免重複觸發提示。

2. **週期性計時與焦點觸發 (`main.rs`)**：
   - 採用 Slint `Timer`，以 1.5 秒為週期進行無負載的輕量檔案中繼資料檢查。
   - 當偵測到 `check_external_change() == true` 時，通知 Slint UI 顯示警告橫幅。

3. **頂部內嵌通知橫幅 (`ui/appwindow.slint`)**：
   - 在工具列與編輯區之間，實作醒目的琥珀黃色通知條（`has-external-change`）：
     - 訊息：「⚠️ 檔案已被外部程式修改。」
     - 若目前緩衝區有未儲存修改（`is_modified == true`），額外顯示警示：「（當前有未儲存修改，重新載入將捨棄變更！）」
     - 操作按鈕：`[ 重新載入 (Reload) ]` 與 `[ 忽略 (Ignore) ]`。
   - 點擊「重新載入」：執行 `reload_file`、更新 Viewport 渲染與狀態列、關閉橫幅。
   - 點擊「忽略」：執行 `ignore_external_change`、關閉橫幅。

## Consequences

- 徹底杜絕因不知情而覆寫外部變更的風險。
- 零外部依賴，符合 KISS 原則，各平台行為一致且無鎖定干擾。
- 頂部通知條採用非侵入式設計，不阻斷使用者當前打字或閱讀操作，體驗現代流暢。
