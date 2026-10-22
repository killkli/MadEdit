# Issue 1: 檔案 mtime 時間戳記追蹤、變更偵測與重新載入核心邏輯

## 狀態
closed

## 目標
在 `DocumentBuffer` 中實作外部檔案變更偵測、安全重新載入與忽略功能，並撰寫完整的單元測試。

## 工作項目
1. `DocumentBuffer` 擴充 `last_modified: Option<std::time::SystemTime>`。
2. 在 `load_file` 與 `save_file` 中維護 `last_modified`。
3. 實作 `check_external_change(&self) -> bool`。
4. 實作 `reload_file(&mut self) -> std::io::Result<()>`，包含游標與書籤的智慧保留與邊界校正。
5. 實作 `ignore_external_change(&mut self)`。
6. 單元測試覆蓋：變更偵測、內容重新載入、修改狀態重設、忽略機制、游標/書籤保留。
