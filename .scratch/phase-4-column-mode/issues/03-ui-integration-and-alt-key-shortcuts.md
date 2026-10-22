Status: closed

# Issue 03: Slint UI 整合、Alt 鍵拖曳支援與狀態列

## Description

將列塊模式的矩形選取與 Slint UI 編輯器元件完整綁定，包含模式切換（`EditMode::Column`）、文字模式下的 `Alt` 鍵滑鼠拖曳、鍵盤導覽擴展選取與狀態列即時資訊。

## Tasks

1. 修改 `ui/appwindow.slint`：編輯區域的 `TouchArea` 支援傳遞 `alt` 修飾鍵狀態（或滑鼠事件傳遞 `modifiers.alt`）。
2. 在 `src/main.rs` 處理滑鼠點擊與拖曳：
   - 當 `doc.edit_mode == EditMode::Column` 或 `alt == true` 時，將選取設定為 `SelectionKind::Block`。
3. 鍵盤事件支援 `Alt + Shift + 方向鍵` 擴展矩形選取。
4. 狀態列顯示：在矩形選取時，狀態列若有選取可顯示選取的矩形大小（如 `Sel: 5x10` 或保持 `Ln X, Col Y`）。
5. 整合測試：驗證模式切換、Alt 鍵拖曳與大檔案按需渲染聯動。
