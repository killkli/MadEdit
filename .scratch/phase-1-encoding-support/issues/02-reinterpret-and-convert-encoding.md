# 02: 重新解碼（Reinterpret Encoding）與轉換編碼（Convert Encoding）核心行為

**What to build:** 在 `DocumentBuffer` 提供 `reinterpret_with_encoding` 與 `convert_to_encoding` 兩個公開操作，分別支援「保留 `raw_bytes` 重新解碼為 `text`（更新 `has_decode_error` 且不變更修改狀態）」以及「保留 `text` 重新編碼更新 `raw_bytes`（清除 `has_decode_error` 並標記為已修改）」。

**Blocked by:** 01: 開檔自動編碼偵測、BOM 保留與防損毀存檔保護

**Status:** ready-for-agent

- [ ] `reinterpret_with_encoding` 能以新編碼（如 GB18030、Shift_JIS 等）重新解碼 `raw_bytes` 更新 `text` 與 `has_decode_error`，且 `raw_bytes` 與 `is_modified` 保持不變
- [ ] `convert_to_encoding` 能將目前 `text` 轉換為目標編碼（包含 UTF-8、Big5、GB18030、UTF-16LE、UTF-16BE 等）更新 `raw_bytes`，清除 `has_decode_error` 並將 `is_modified` 設為 `true`
- [ ] 所有核心程式碼皆加上清楚的 `zh-TW` 繁體中文註解說明意圖
