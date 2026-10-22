# 0004: 搜尋與取代引擎及內嵌式面板架構

為了提供快速、靈活且跨模式的文字定位與大量修改能力，我們決定實作統一的搜尋與取代核心引擎（Find & Replace Engine），並在 Slint UI 採用內嵌式可收合面板。

## Context

搜尋與取代是專業文字編輯器的高頻核心功能。
在文字編輯模式、列塊模式與十六進位模式下，使用者對於搜尋的期待各有不同：
- 文字模式：需要支援大小寫敏感、全字匹配（Whole Word）以及強大的正規表達式（Regex）。
- 十六進位模式：需要支援位元組序列（Hex Byte，如 `48 65 6C 6C 6F`）搜尋。
- UI 互動：傳統獨立對話框容易遮蔽文字編輯視野，而主流現代編輯器（如 VSCode、Sublime Text）皆採用編輯區內嵌式浮動/固定面板，支援快捷鍵（Ctrl+F、Ctrl+H、Esc）秒級展開與收合。

## Decision

1. **核心搜尋引擎（SearchEngine）**：
   - 在 `src/core/search.rs` 中實作搜尋核心。
   - 定義 `SearchOptions` 結構：
     - `match_case: bool`（區分大小寫）
     - `whole_word: bool`（全字匹配）
     - `use_regex: bool`（正規表達式，引入 `regex` crate）
   - 實作 `SearchMatch`：包含匹配文字的起訖座標 `(TextPosition, TextPosition)` 或位元組位移 `(usize, usize)`。
   - 實作尋找演算法：
     - `find_matches(text: &str, query: &str, options: &SearchOptions) -> Vec<SearchMatch>`
     - `find_next(text, query, current_pos, options) -> Option<SearchMatch>`
     - `find_prev(text, query, current_pos, options) -> Option<SearchMatch>`
     - `replace_all(text, query, replace_with, options) -> (String, usize)` 回傳取代後字串與取代次數。
2. **十六進位位元組搜尋（Hex Byte Search）**：
   - 支援解析十六進位字串（如 `48 65 6C 6C 6F` 或 `48656C6C6F`），並在底層位元組串流（`PieceTable`）中進行位元組模式匹配（Byte Pattern Matching），精準定位 Offset。
3. **Slint 內嵌式可收合面板（Search Panel）**：
   - 在 `ui/appwindow.slint` 編輯區頂部嵌入條件渲染的搜尋工具列（`show_search_panel: bool` 與 `show_replace: bool`）。
   - 包含搜尋輸入框、取代輸入框、匹配狀態（如 `3/15`）、查找按鈕（上一個/下一個）、取代按鈕（取代/全部取代）、選項按鈕（大小寫/全字/Regex）與關閉按鈕。
   - 快捷鍵綁定：`Ctrl+F` 開啟搜尋列、`Ctrl+H` 開啟搜尋與取代列、`Esc` 關閉搜尋列、`F3` 尋找下一個、`Shift+F3` 尋找上一個。

## Consequences

- 搜尋與取代邏輯與 UI 展現層徹底解耦，核心邏輯由單元測試完整保證。
- 內嵌式面板操作流暢且不遮蔽文件主要視線，大幅提升使用者體驗。
