# 03: 工具列編碼選單與重新解碼／轉換編碼 UI 綁定與文件同步

**What to build:** 在 Slint UI 工具列新增編碼選擇器（`ComboBox`）與「重新解碼」、「轉換編碼」按鈕，於 `main.rs` 連接 `DocumentBuffer`，並在狀態列顯示解碼警告或拒絕存檔訊息；最後同步更新 `AGENTS.md` 第 5 節進度與已知問題。

**Blocked by:** 02: 重新解碼（Reinterpret Encoding）與轉換編碼（Convert Encoding）核心行為

**Status:** ready-for-agent

- [ ] `ui/appwindow.slint` 工具列提供編碼下拉選單與「重新解碼」、「轉換編碼」互動控制項
- [ ] `src/main.rs` 綁定開檔、存檔（含防損毀錯誤提示）、重新解碼、轉換編碼事件，並同步更新 UI 狀態
- [ ] 更新 `AGENTS.md` 第 5 節完成進度與已知問題，且 `cargo check` 與 `cargo test` 零警告通過
