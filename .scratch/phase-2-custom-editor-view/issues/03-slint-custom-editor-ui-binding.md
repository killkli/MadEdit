# 03: Slint 可見行虛擬渲染、鍵盤滑鼠互動與狀態列整合

**What to build:** 在 Slint UI 層以宣告式可見行列表（`for line in visible-lines`）、左側行號欄、反白矩形與游標取代內建 `TextEdit`，連接鍵盤與滑鼠事件（含剪貼簿與滾輪捲動），並即時同步狀態列 `Ln X, Col Y`。

**Blocked by:** 02: 游標導覽、範圍選取與基礎文字編輯邏輯

**Status:** ready-for-agent

- [ ] `ui/appwindow.slint` 實作行號欄、可見行列表、選取反白區與游標渲染元件
- [ ] `src/main.rs` 處理鍵盤按鍵事件（方向鍵、編輯鍵、快捷鍵）與滑鼠點擊/拖曳/滾輪捲動事件
- [ ] 狀態列即時顯示游標位置 `Ln X, Col Y`，且 `cargo check` 與 `cargo test` 零警告通過
- [ ] 同步更新 `AGENTS.md` 第 5 節進度
