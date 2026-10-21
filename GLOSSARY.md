# MadEdit-Rust

跨平台、輕量級的專業文字、列塊與十六進位編輯器。

## Language

**DocumentBuffer**:
代表單一開啟或未命名文件的核心狀態容器，同時維護底層原始位元組（`raw_bytes`）、解碼後的文字（`text`）、字元編碼（`encoding`）、換行格式（`newline_format`）與編輯模式（`edit_mode`）。
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
