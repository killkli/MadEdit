# Spec: Phase 10 編輯器焦點輸入修復與原版 MadEdit 經典專業 UI

## 1. 目標
1. 修復自繪編輯區無法打字輸入之焦點遺失問題，以及等寬字型失效與 C# 語法未識別問題。
2. 打造對齊原版 MadEdit (MadEdit-Mod) 的專業編輯器介面，支援深色/淺色主題一鍵切換（預設跟隨系統設定），消除所有主題撞色問題。

## 2. 規格細節

### 2.1 焦點與輸入修復 (`ui/appwindow.slint`, `src/main.rs`, `src/core/editor.rs`)
1. **常駐 `FocusScope`**：
   - `editor-focus := FocusScope` 置於編輯區外層，設定 `init => { self.focus(); }`。
   - `MainWindow` 提供 `public function focus-editor() { editor-focus.focus(); }`。
   - 滑鼠點擊編輯區 `TouchArea` 時立即觸發 `editor-focus.focus()`。
   - 開檔、存檔、切換模式、關閉搜尋/跳轉面板後自動呼叫 `ui.invoke_focus_editor()`。
2. **支援插入/覆寫模式 (`Insert` / `Overwrite`)**：
   - `EditorState` 新增 `overwrite_mode: bool`（預設 `false` 即插入模式）。
   - 按下 `Insert` 鍵切換 `overwrite_mode`；當 `overwrite_mode == true` 且無選取範圍時，輸入單一字元會覆寫游標當前位置的字元（若在行尾則追加）。
3. **擴充快捷鍵**：
   - `Ctrl+O` 開啟舊檔、`Ctrl+S` 儲存檔案。

### 2.2 語法突顯與深淺色主題 (`src/core/syntax.rs`, `src/core/buffer.rs`)
1. **擴充語言支援**：
   - `SUPPORTED_SYNTAXES` 與 `detect_syntax_from_path` 新增 `C#` (`.cs`)、`Java` (`.java`)、`Go` (`.go`)、`Shell` (`.sh`, `.bash`, `.zsh`, `.ps1`)、`TOML` (`.toml`)。
2. **深淺色語法主題**：
   - 新增 `highlight_lines_with_theme(syntax_name: &str, lines: &[String], is_dark: bool) -> Vec<Vec<HighlightSpan>>`。
   - 深色模式採用 `base16-ocean.dark`（純文字預設色 `(216, 222, 233)`），淺色模式採用 `InspiredGitHub`（純文字預設色 `(34, 34, 34)`）。
   - `DocumentBuffer` 新增 `is_dark_mode: bool` 與 `show_control_chars: bool`。

### 2.3 原版 MadEdit 四大經典介面元素 (`ui/appwindow.slint`, `src/main.rs`)
1. **頂部主選單列 + 緊湊型工具列**：
   - 第一排：主選單列（`檔案(F)`、`編輯(E)`、`尋找(S)`、`檢視(V)`、`工具(T)` 快捷操作按鈕）與右側深淺色切換、控制字元 (`¶`) 開關。
   - 第二排：緊湊工具列（開檔、存檔、模式切換、編碼與重新解碼/轉換、語法下拉、換行下拉、字級大小調整 `12..22`、快速搜尋框）。
2. **檔案分頁標籤列 (Tab Bar) + 水平欄位尺標 (Column Ruler)**：
   - 顯示當前開啟檔名分頁標籤（含修改星號 `*`）。
   - 編輯區上方固定顯示水平尺標（與 Gutter 對齊，依 `cell-width` 繪製 `0..10..20..30..` 刻度與每 5 欄短刻線）。
3. **游標所在行外框/底色高亮 + 可視控制字元**：
   - 游標所在行（`line.has_caret`）繪製醒目底色與外框線（如原版 MadEdit 的深灰底與細框）。
   - 當開啟控制字元顯示（預設開啟，可一鍵切換）時：
     - 將呈現層的空白顯示為輕量中點 `·`、Tab 展開處首字元顯示為 `→`。
     - 每行結尾處繪製綠色換行符號 `↵`（CRLF/LF/CR）。
4. **原版風格多格狀態列 (Multi-pane Status Bar)**：
   - 分隔為 5 個清楚的狀態格：
     1. `行: {cur_line} / {total_lines}   列: {cur_col}`（若為列塊選取則附註 `Block: RxC`）
     2. `字元位置: {char_offset} (0x{byte_hex}) / {total_bytes}`
     3. `選取長度: {sel_chars} ({sel_bytes} bytes)`
     4. `{encoding}{bom_str}.{newline_short}`（如 `UTF-8.DOS` 或 `UTF-8.BOM.UNIX`）
     5. `{插入 / 覆寫}`（可點擊切換）
