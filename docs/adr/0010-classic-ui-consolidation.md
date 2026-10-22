# ADR 0010：經典 MadEdit 介面主軸與 Phase 10 功能整合

- 狀態：已採納
- 日期：2026-10-22
- 關聯：[ADR 0009](0009-madedit-classic-ui-and-focus-fix.md)、`.scratch/phase-10-classic-ui-redesign/spec.md`

## 背景

本專案在 Phase 10 出現兩條並行的介面實作路線：

| 路線 | 內容 | 產出 |
| --- | --- | --- |
| A（經典風格重塑） | 自繪經典 Windows 風格元件庫（`theme.slint` / `menubar.slint`）、原版 MadEdit XPM 圖示工具列、依原版 `CommandTable` 建構的 7 個頂層選單、六欄位狀態列 | `.scratch/phase-10-classic-ui-redesign/spec.md` |
| B（焦點修復與主題） | 編輯區 FocusScope 焦點修復、插入／覆寫模式、深淺色主題、水平欄位尺標、垂直／水平捲軸、控制字元顯示、多語法（C#/Java/Go/Shell/TOML） | `.scratch/phase-10-madedit-classic-ui-and-input-fix/spec.md` |

兩者同樣以「貼近原版 MadEdit 外觀」為目標，但實作方式與取捨不同：路線 A 走「原版圖示 + 自繪元件 + 完整選單模型」，路線 B 走「Slint 內建元件 + 主題化配色 + 捲軸尺標」。
若任選其一直接覆蓋另一方，都會遺失對方已驗證完成的功能。

## 決策

**以路線 A（經典自繪介面）為介面主軸，完整吸收路線 B 的全部功能。**

具體作法：

1. **介面骨架採路線 A**：保留 `ui/theme.slint`（配色與共用元件）、`ui/menubar.slint`（選單列與彈出選單）、
   原版 XPM 圖示工具列、七個頂層選單（檔案／編輯／搜尋／檢視／工具／視窗／說明）與六欄位狀態列。
2. **功能採路線 B**：
   - `EditorState::overwrite_mode` 與 `Insert` 鍵切換（覆寫模式）。
   - `DocumentBuffer::is_dark_mode` / `show_control_chars` 與 `syntax::highlight_lines_with_theme`。
   - 水平欄位尺標、右側垂直捲軸、底部水平捲軸（含滑塊拖曳與按鈕微調）。
   - 游標所在行外框高亮、控制字元（`·` / `»`）與行尾 `↵` 標記。
   - 常駐 `FocusScope` 承接鍵盤事件並於開檔、存檔、切換模式後自動回復焦點。
   - `C#` / `Java` / `Go` / `Shell` / `TOML` 等語法辨識。
3. **配色統一由 `theme.slint` 的 `Theme` 管理**，以 `is-dark` 開關切換淺色（經典 Windows 灰）與深色配色，
   選單列、工具列、狀態列、編輯區、捲軸共用同一份色票，避免撞色與色彩碼散落各處。
4. **狀態列沿用路線 A 的六欄位版面**，但欄位內容改用路線 B 的核心計算
   （`DocumentBuffer::cursor_and_selection_metrics` 與 `encoding_status_code`），
   呈現 `CharPos`（含位元組位置）、`SelSize`（含位元組數）、`編碼.BOM.DOS` 與 `INS`／`OVR`。

## 理由

1. **功能不減**：使用者看見的成果是兩條路線的聯集，而非取其一。
2. **單一事實來源**：`Theme` 集中管理配色，Slint 元件僅從 `Theme` 取色，新增配色只需改一處。
3. **外觀一致性**：自繪元件可保證 Windows、macOS、Linux 外觀一致，避免 Slint 內建 Fluent 樣式
   與原版差異過大。
4. **鍵盤輸入必須經由 FocusScope**：自繪編輯器不依賴內建 `TextEdit`，
   鍵盤事件鏈路一旦在重寫中遺失就會完全無法打字，因此將 `FocusScope` 設為編輯區的常駐根元素，
   讓「焦點修復」與「經典版面」在結構上綁定，避免再次脫節。

## 後果

- `ui/appwindow.slint` 為單一大型版面檔案，編輯區（尺標 + 文字 + 捲軸）與十六進位模式共用同一個
  `editor-focus` FocusScope。
- 十六進位模式不再與文字區並存，兩者互斥顯示。
- 狀態列的 `INS`／`OVR` 指示由 `EditorState::overwrite_mode` 驅動，`Insert` 鍵與工具列按鈕皆可切換。
- 深色主題預設跟隨系統配色（`Palette.color-scheme`），亦可於工具列手動切換。
