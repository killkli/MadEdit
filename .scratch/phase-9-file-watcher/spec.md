# Spec: Phase 9 外部檔案變更監控與自動/手動重新載入

## 1. 目標
實作跨平台的外部檔案變更監控與友善的非侵入式提示橫幅，偵測磁碟上的檔案是否被外部編輯器、Git 或腳本修改，並提供安全的重新載入或忽略機制。

## 2. 規格細節

### 2.1 核心偵測與緩衝區同步 (`src/core/buffer.rs`)
1. **中繼資料追蹤**：
   - `DocumentBuffer` 新增 `last_modified: Option<std::time::SystemTime>` 欄位。
   - 在 `load_file` 與 `save_file` 成功時，以 `fs::metadata(&path).and_then(|m| m.modified()).ok()` 記錄最新時間戳記。
2. **外部變更檢測**：
   - 實作 `check_external_change(&self) -> bool`。
   - 若 `file_path` 或 `last_modified` 為 None 則回傳 `false`。
   - 若磁碟檔案的目前 `modified()` 大於 `last_modified`，回傳 `true`；若檔案不存在或讀取失敗回傳 `false`。
3. **重新載入**：
   - 實作 `reload_file(&mut self) -> std::io::Result<()>`。
   - 重新載入最新內容，重設 `is_modified = false`，更新 `last_modified`。
   - 盡可能保留原本的游標位置（line, col）與行書籤集合（clamp 於新行數範圍內）。
4. **忽略變更**：
   - 實作 `ignore_external_change(&mut self)`。
   - 將 `last_modified` 更新為磁碟目前時間，使後續檢查不再提示。

### 2.2 Slint UI 頂部橫幅與操作 (`ui/appwindow.slint`)
1. **通知屬性與回呼**：
   - `in-out property <bool> has-external-change: false;`
   - `callback reload-file();`
   - `callback ignore-external-change();`
2. **視覺外觀**：
   - 工具列下方展示琥珀黃色背景橫幅（`#fffbeb` / 邊框 `#fde68a`）。
   - 醒目標示「⚠️ 檔案已被外部程式修改。」
   - 若 `is-modified` 為真，以顯眼的深橘紅文字提示「（當前有未儲存修改，重新載入將捨棄變更！）」。
   - 提供「重新載入」與「忽略」按鈕。

### 2.3 週期性排程偵測 (`src/main.rs`)
1. 使用 `slint::Timer` 每 1.5 秒執行一次無負載的 `doc.lock().unwrap().check_external_change()`。
2. 若為真且 `has-external-change` 尚未開啟，透過 `upgrade_in_event_loop` 啟用橫幅提示。
3. 綁定 `reload_file` 與 `ignore_external_change` 回呼，重新同步文字、可視行、語法、狀態列並關閉橫幅。
