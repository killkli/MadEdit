Status: ready-for-agent

# Issue 03: Slint UI 多色分段渲染與工具列語言選單

## Description

在 `ui/appwindow.slint` 支援可見行多色文字分段渲染，並在工具列新增語言選擇下拉選單，於 `src/main.rs` 進行狀態同步與回呼綁定。

## Tasks

1. 修改 `ui/appwindow.slint`：
   - 定義 `export struct HighlightSpan { text: string, color_hex: string }`。
   - 更新 `export struct VisibleLine`，加入 `spans: [HighlightSpan]`。
   - 增加屬性：
     - `in-out property <[string]> syntax-list: ["Plain Text", "Rust", ...];`
     - `in-out property <string> current-syntax: "Plain Text";`
   - 增加回呼：`callback syntax-changed(string);`。
   - 工具列加入語言選擇 `ComboBox`。
   - 編輯區文字層：依據 `spans` 進行分段渲染或回退單色渲染。
2. 修改 `src/main.rs`：
   - 取得語法清單並初始化 UI `syntax-list`。
   - 在 `sync_ui_from_doc` 同步 `spans` 與 `current-syntax`。
   - 綁定 `on_syntax_changed` 回呼，使用者切換語言時更新 `DocumentBuffer` 並重繪。
3. 手動與視覺回歸驗證：開檔 `.rs` 檢驗多色關鍵字高亮，切換語言選單檢驗即時變色。
