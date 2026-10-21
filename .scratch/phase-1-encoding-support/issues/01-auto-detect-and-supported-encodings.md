# 01: 開檔自動編碼偵測、BOM 保留與防損毀存檔保護

**What to build:** 在核心提供 10 種常用編碼清單（`SUPPORTED_ENCODINGS`），實作開檔時的 BOM 識別與自動編碼偵測，記錄 `has_bom` 與 `has_decode_error`；自行實作 UTF-16LE/BE 編碼與 BOM 保留，並在 `has_decode_error` 為真時拒絕覆寫存檔以防止非 UTF-8 檔案損毀。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 提供包含 10 種標準編碼名稱的 `SUPPORTED_ENCODINGS` 常數清單
- [x] 開啟含 BOM（UTF-8 / UTF-16LE / UTF-16BE）檔案時正確識別編碼與 `has_bom = true`，存檔時完整保留 UTF-16 位元組與 BOM（修正已知問題 2）
- [x] 開啟無 BOM 檔案時先驗證嚴格 UTF-8，失敗則 fallback 至 Big5，並記錄 `has_decode_error`
- [x] 當 `has_decode_error == true` 時，`save_file` 回傳錯誤拒絕存檔，防止將亂碼 `U+FFFD` 寫回磁碟毀損原檔（修正已知問題 1）
