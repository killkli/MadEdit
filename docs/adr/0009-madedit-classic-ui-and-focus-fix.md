# 0009: 編輯器焦點輸入修復與原版 MadEdit 經典專業 UI 架構

為了解決自繪編輯區無法接收鍵盤輸入的焦點問題，並徹底消除系統深淺色主題造成的文字吃色缺陷、對齊原版 MadEdit (MadEdit-Mod) 的經典專業視覺體驗，我們決定進行編輯器焦點架構修正與 UI 全面升級。

## Context

1. **鍵盤輸入失效根因 (`FocusScope` 焦點遺失)**：
   - 先前 `FocusScope` 置於條件區塊 `if root.edit-mode != 2` 內，既無 `init => { self.focus(); }`，且 `TouchArea` 點擊時未呼叫 `.focus()` 歸還焦點。當使用者點擊工具列按鈕（如「開啟舊檔」）後，焦點永久停留在按鈕上，導致編輯區無法接收任何鍵盤輸入。
2. **字型與語法識別缺陷**：
   - Slint `font-family` 不支援 CSS 逗號分隔 fallback 清單（`"Consolas, Courier New, monospace"`），導致退回系統比例字型，造成字元寬度與游標座標脫節。
   - `syntax.rs` 副檔名識別缺少 C# (`.cs`)、Java、Go、Shell、TOML 等常用語言。
3. **UI 主題撞色與專業度不足**：
   - 在 Windows 深色模式下，部分元件背景寫死為淺色而文字繼承系統白色（或反之），造成狀態列與檔名幾乎不可見。
   - 缺乏原版 MadEdit 的核心專業介面元素：頂部主選單列 (Menu Bar)、緊湊工具列、檔案分頁標籤 (Tab Bar)、水平欄位尺標 (Column Ruler)、控制字元顯示（Tab `→`、空白 `·`、換行 `↵`）、游標所在行高亮框、以及原版風格多格狀態列。

## Decision

1. **焦點架構修正 (`editor-focus := FocusScope`)**：
   - 將 `editor-focus := FocusScope` 提升至編輯檢視區最外層（脫離 `if` 條件分支），配置 `init => { self.focus(); }`。
   - 暴露 `public function focus-editor() { editor-focus.focus(); }`，於滑鼠點擊 `TouchArea`、工具列操作、開啟檔案、關閉面板時主動歸還焦點。
2. **深淺色雙主題系統（預設跟隨系統，支援一鍵切換）**：
   - 利用 Slint `Palette.color-scheme` 取得系統初始深淺色設定，並提供明確的深色/淺色色票屬性與一鍵切換按鈕。
   - `syntect` 語法突顯同步支援深色主題（`base16-ocean.dark`）與淺色主題（`InspiredGitHub`）。
3. **原版 MadEdit 四大經典介面元素**：
   - **主選單列與雙排緊湊工具列**：包含檔案/編輯/尋找/檢視/工具選單動作、內嵌快速搜尋框、編碼/語法/換行選單、字級大小調整（12px–22px，動態連動 `cell_width` 與 `line_height`）、控制字元開關與深淺色切換。
   - **檔案分頁標籤列與水平欄位尺標 (Column Ruler)**：於編輯區上方繪製當前檔案分頁標籤與 `0..10..20..30..` 精準等寬刻度尺。
   - **游標行高亮與可視控制字元**：高亮游標所在行背景與邊框；開啟控制字元顯示時，在每行尾端繪製綠色換行標記 `↵`，並將視覺呈現中的空白與 Tab 標示出來。
   - **原版多格狀態列**：底部以分隔格線清晰展示：`行: X/Y 列: Z`、`字元位置: Offset (0xHex) / Total`、`選取長度: N`、`編碼.BOM.換行`、`插入/覆寫 (INS/OVR)`。

## Consequences

- 徹底修復鍵盤打字、快捷鍵與滑鼠點擊後的焦點問題。
- 介面視覺對齊原版 MadEdit-Mod 的專業質感，深色與淺色模式下皆具備極佳對比度與可讀性。
