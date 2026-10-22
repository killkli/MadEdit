# 0005: 語法突顯引擎與多色文字段渲染架構

為了使 MadEdit 在編輯各類程式語言原始碼與標記語言時具備現代化語法突顯（Syntax Highlighting）體驗，我們決定引入 `syntect` 語法引擎，並在 Slint 自繪編輯器中實現文字分段著色（Highlighted Spans）架構。

## Context

在純文字編輯器中，語法著色能顯著提升程式碼可讀性，幫助開發者迅速區分關鍵字、字串、註解、型別、數字與識別碼。
- 原生 Slint 限制：Slint 原生 `Text` 元件僅支援單一前景色（`color`），無法在單一字串中原生進行富文字或標籤著色。
- 大檔案與效能考量：語法分析通常需要狀態維護（跨行註解、多行字串等）。若每次按鍵或捲動都全檔重新高亮，在數萬行檔案時會造成嚴重卡頓；因此必須配合 Viewport（可見行）按需著色，或維護輕量快取。
- 多語言支援度：若手刻每種語言的詞法分析器，規則繁瑣且難以覆蓋主流語言的邊緣語法。Sublime Text 語法定義（`.sublime-syntax`）是業界標準之一。

## Decision

1. **引進 `syntect` 作為語法突顯核心引擎**：
   - 採用 `syntect = { version = "5.2", default-features = false, features = ["default-syntaxes", "default-themes", "regex-fancy"] }`。
   - `SyntaxSet` 提供主流程式語言（Rust, C/C++, Python, JavaScript, HTML, CSS, Markdown, JSON, XML, YAML, SQL 等 30+ 種）之語法定義。
   - `ThemeSet` 提供標準編輯器色彩主題，預設採用淺色主題 `InspiredGitHub` 或 `base16-ocean.light`，與編輯器白色底色協調。
2. **抽象語法高亮管理器（`SyntaxHighlighter`）**：
   - 在 `src/core/syntax.rs` 中實作高亮核心。
   - 根據副檔名（`.rs`, `.py`, `.c`, `.cpp`, `.js`, `.json` 等）自動識別語言。
   - 支援「純文字 (Plain Text)」模式以關閉語法突顯。
   - 提供方法：`highlight_line(line_text: &str, line_idx: usize, state: &mut HighlightState) -> Vec<HighlightSpan>`。
3. **文字分段著色結構（`HighlightSpan`）與 Slint 整合**：
   - 定義 `HighlightSpan`：包含 `text: String` 與 `color: String`（如十六進位 `#0000FF` 或 `#008000`）。
   - 在 `ui/appwindow.slint` 中擴展可見行資料結構：
     ```slint
     export struct HighlightSpan {
         text: string,
         color_hex: string,
     }
     export struct VisibleLine {
         line_number: int,
         text: string,
         spans: [HighlightSpan],
         ...
     }
     ```
   - 繪製層依序水平排列或在無 spans 時 fallback 至單色 `Text`，維持既有字元格網幾何游標與矩形選取不受干擾。
4. **語言選擇 UI**：
   - 在主工具列新增語言選擇下拉選單（`ComboBox`），即時顯示當前語言並支援手動切換。

## Consequences

- 支援豐富且精確的主流語言語法突顯，編輯體驗媲美現代 IDE。
- 幾何圖層（游標、串流選取、列塊矩形選取）與文字渲染圖層清晰分離，保證列塊模式與游標對齊不受語法著色影響。
- 透過 Viewport 按需著色，維持大檔案與高頻輸入時的高效流暢。
