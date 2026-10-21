# MadEdit-Rust

跨平台、輕量級的專業文字、列塊與十六進位編輯器。

## Language

**DocumentBuffer**:
代表單一開啟或未命名文件的核心狀態容器，維護底層原始位元組（`raw_bytes`）、文字內容（`text`）、字元編碼（`encoding`）、換行格式（`newline_format`）與編輯模式（`edit_mode`）。
_Avoid_: FileState, EditorModel, TextBuffer

**EditMode**:
編輯器的三種檢視與操作模式：`Text`（文字模式）、`Column`（列塊模式）與 `Hex`（十六進位模式）。
_Avoid_: ViewMode, DisplayMode

**NewLineFormat**:
文件中的行尾換行字元格式，區分為 `Dos`（CRLF）、`Unix`（LF）與 `Mac`（CR）。
_Avoid_: LineEnding, EOL

**Reinterpret Encoding (重新解碼)**:
保持底層原始位元組（`raw_bytes`）不變，改以新指定的字元編碼重新將 `raw_bytes` 解碼為 `text`，用於修正開啟檔案時的編碼誤判，不將文件標記為已修改。
_Avoid_: Reload Encoding, Switch Encoding

**Convert Encoding (轉換編碼)**:
保持當前文字內容（`text`）不變，將文件目標編碼改為新指定的字元編碼，並將 `text` 重新編碼更新至 `raw_bytes`，同時將文件標記為已修改（`is_modified = true`）。
_Avoid_: Transcode, Change Encoding

**TextPosition**:
文字在文件中的 0-based 邏輯座標 `(line, column)`，其中 `line` 為行索引，`column` 為行內的字元索引（Unicode scalar value 或 char offset）。
_Avoid_: TextCursor, BufferPoint

**VisualColumn**:
字元在畫面渲染時所佔據的視覺欄位索引。ASCII/半形佔 1 欄，CJK 全形佔 2 欄，Tab 對齊至下一個 4 欄倍數。
_Avoid_: DisplayColumn, CharWidth

**Selection**:
文件中的文字選取範圍，由錨點 `anchor: TextPosition` 與當前游標 `head: TextPosition` 組成；當 `anchor == head` 時代表無選取範圍的單一游標。
_Avoid_: Range, Region

**Caret**:
編輯器中閃爍的文字輸入游標，對應至 `Selection` 的 `head` 位置。
_Avoid_: Cursor, Pointer

**Viewport**:
目前編輯器可見視窗所涵蓋的行範圍，包含頂部可見行索引 `first_visible_line` 與可容納行數 `visible_lines`。
_Avoid_: VisibleWindow, ScrollWindow

**Character Cell Grid (字元格網)**:
以固定像素欄寬為單位的水平對齊格網，確保半形佔 1 格、全形佔 2 格，徹底解決不同平台等寬字型中英混排游標漂移與垂直對齊問題。
_Avoid_: CharBox, FontGrid
