Status: closed

# Phase 7: 列號跳轉與編輯增強 (Go to Line & Editing Enhancements)

## Problem Statement

當前 MadEdit 已具備自繪編輯器元件、大檔案 Piece Table 引擎、列塊模式、搜尋取代以及語法突顯功能。
然而在日常程式碼開發與長篇文字檢視時，尚缺乏以下現代編輯器的核心功能：
1. **無法精確快速跳轉到指定行號或欄位**：閱讀編譯錯誤或日誌時，需手動捲動或點擊尋找，效率較低。
2. **缺乏常用行編輯操作**：缺少整行上下移動（Move Line Up/Down）、整行複製（Duplicate Line）、整行刪除（Delete Line）與智慧行首（Smart Home），必須透過反白、剪下、換行貼上等多步驟完成，編輯流暢度受限。

## Solution

實作指定行號/欄位跳轉內嵌面板與常用行級編輯增強：

1. **核心行編輯 API（`src/core/editor.rs` & `src/core/buffer.rs`）**：
   - 實作 `EditorState::goto_line_col(line: usize, col: usize)`：行號與欄位 Clamp 邊界防護，清除選取並將目標游標自動捲入可見視窗。
   - 實作 `EditorState::move_lines_up()` 與 `EditorState::move_lines_down()`：支援單行或多行選取範圍上下移動，維持游標相對位置。
   - 實作 `EditorState::duplicate_lines()`：無選取時複製游標行至下一行；有選取時複製整個選取區行。
   - 實作 `EditorState::delete_lines()`：刪除游標所在整行（若為最後一剩餘行則清空字串，保證 lines 不為空）。
   - 實作 `EditorState::move_home_smart(expand_selection: bool)`：若游標在縮排非空白字元後則跳轉至第一個非空白字元；若已在第一個非空白字元則跳至欄位 0。
   - 在 `DocumentBuffer` 中封裝對應方法並自動觸發 `sync_from_editor`。
   - 在 Hex 模式下支援 `goto_offset(offset: usize)` 跳轉至指定 Offset。

2. **UI 內嵌跳轉面板（`ui/appwindow.slint`）**：
   - 在編輯區頂部新增可收合跳轉面板（`if root.show-goto-panel`）。
   - 包含提示文字（文字模式顯示「行號 (1 - 總行數):」，Hex 模式顯示「位移 Offset (0x0 - 0x...):」）、輸入框、跳轉按鈕與取消按鈕。
   - 支援 `Ctrl+G` 快捷鍵開啟跳轉面板並自動聚焦與填入目前行號。
   - 支援 `Enter` 執行跳轉並關閉面板；`Esc` 取消關閉面板。
   - 支援輸入格式 `line` 或 `line:col`（如 `42` 或 `42:10`）。

3. **鍵盤快捷鍵分派整合（`src/main.rs`）**：
   - `Ctrl+G`：展開跳轉面板並填入預設行號。
   - `Alt+Up` / `Alt+Down`：觸發整行上下移動。
   - `Ctrl+Shift+D`：觸發整行複製。
   - `Ctrl+Shift+K`：觸發整行刪除。
   - `Home` 鍵綁定智慧行首。

## 使用者故事 (User Stories)

1. 身為使用者，當我在編譯程式碼看到錯誤訊息 `line 125, column 18` 時，我希望按下 `Ctrl+G` 輸入 `125:18` 並按 Enter，編輯器游標能立刻定位到該行該欄，且視窗自動捲動至可見區域。
2. 身為使用者，在文字編輯時，我希望按下 `Alt+Up` 或 `Alt+Down` 可以將當前行或選取的行上下移動位置，無需剪下貼上。
3. 身為使用者，我希望按下 `Ctrl+Shift+D` 能迅速在下一行複製一份當前行，以方便撰寫相似的程式碼。
4. 身為使用者，我希望按下 `Ctrl+Shift+K` 能直接刪除當前行，游標保持在原行位置。
5. 身為使用者，當我按下 `Home` 鍵時，游標應優先跳到該行縮排後的第一個字元，再次按下時跳到行首欄位 0。
6. 身為使用者，在十六進位模式下按 `Ctrl+G`，我能輸入十六進位位移（如 `0x1F0`）直接跳轉到該 Byte。

## Implementation Tickets

1. `issues/01-core-line-editing-and-goto-operations.md`：核心行編輯操作與跳轉邏輯實作（單元測試先導）。
2. `issues/02-ui-goto-panel-and-hex-offset.md`：Slint UI 跳轉面板、格式解析與 Hex 模式 Offset 跳轉支援。
3. `issues/03-keyboard-shortcuts-and-editing-integration.md`：全套快捷鍵整合（Ctrl+G, Alt+Up/Down, Ctrl+Shift+D, Ctrl+Shift+K, Smart Home）與事件分派。
