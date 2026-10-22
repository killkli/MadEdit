Status: ready-for-agent

# Phase 6: 語法突顯 (Syntax Highlighting)

## Problem Statement

目前編輯器所有文字內容皆以單一黑色字型（`#222222`）呈現，無論是關鍵字、字串、型別、註解或常數皆無色彩區分。這在瀏覽或編輯程式碼（如 Rust, C++, Python, JavaScript 等）與資料交換格式（如 JSON, Markdown）時，視覺結構不易辨識，影響閱讀與編輯效率。

## Solution

整合 `syntect` 語法分析引擎，並在 Slint 自繪編輯器實作多色分段渲染：

1. **語法引擎整合（`src/core/syntax.rs`）**：
   - 載入 `syntect` 的預設語法集（`SyntaxSet`）與預設主題集（`ThemeSet`，預設採用 `InspiredGitHub` 淺色主題）。
   - 實作語言識別：依據副檔名（`.rs`, `.c`, `.cpp`, `.h`, `.py`, `.js`, `.ts`, `.json`, `.md`, `.html` 等）自動切換對應語法定義，並支援 `Plain Text`。
   - 實作按行分段著色：將單行文字轉換為多個 `HighlightSpan`（包含 `text` 與 `color_hex`，如 `#D73A49` 關鍵字、`#032F62` 字串、`#6A737D` 註解）。
2. **緩衝區與編輯器整合（`src/core/editor.rs` & `src/core/buffer.rs`）**：
   - 在可見行繪製資料結構（`VisibleLineData`）加入 `spans: Vec<HighlightSpan>`。
   - 在 `DocumentBuffer` 中增加目前語言狀態管理（`current_syntax: String`）與語法高亮轉換邏輯。
   - 保留現有固定字元格網、游標矩形、串流選取與列塊選取之幾何計算，幾何圖層與色彩圖層徹底解耦。
3. **Slint UI 多色渲染與語言切換（`ui/appwindow.slint` & `src/main.rs`）**：
   - `VisibleLine` 結構擴展 `spans: [HighlightSpan]`。
   - 編輯區文字層：依據 `line.spans` 水平排列或若無 spans 時回退至單色整行文字。
   - 工具列新增「語言選擇」下拉選單（`ComboBox`），支援使用者隨時手動覆寫語言。
   - 狀態列支援即時顯示當前語言名稱。

## 使用者故事 (User Stories)

1. 身為使用者，當我開啟 `.rs`、`.py`、`.c` 或 `.json` 等程式碼檔案時，我希望編輯器能自動為關鍵字、字串、註解等語法元素加上不同顏色，讓程式碼層次分明。
2. 身為使用者，當我開啟未命名檔案或附檔名無對應語法時，我希望能夠透過工具列下拉選單手動選擇語法定義（例如手動選取「Rust」）。
3. 身為使用者，我希望在列塊模式或進行文字選取時，選取底色與游標位置依舊精準對齊，不因語法著色而發生文字跳動或漂移。
4. 身為使用者，當我選擇「純文字 (Plain Text)」時，文字能恢復單色高效顯示。

## Implementation Decisions

- **相依套件**：在 `Cargo.toml` 加入：
  ```toml
  syntect = { version = "5.2", default-features = false, features = ["default-syntaxes", "default-themes", "regex-fancy"] }
  ```
- **核心資料型別**：
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct HighlightSpan {
      pub text: String,
      pub color_hex: String,
  }
  ```
- **可見行結構擴充**：
  ```rust
  pub struct VisibleLineData {
      pub line_number: i32,
      pub text: String,
      pub spans: Vec<HighlightSpan>,
      pub has_selection: bool,
      pub sel_start_x: f32,
      pub sel_width: f32,
      pub has_caret: bool,
      pub caret_x: f32,
  }
  ```

## Testing Decisions

- **測試接縫 (Seams)**：
  1. `syntax.rs` 語法引擎單元測試：驗證依副檔名自動識別語言（`.rs` -> Rust, `.py` -> Python, `.json` -> JSON 等）。
  2. `syntax.rs` 單行高亮測試：驗證關鍵字（如 `fn`, `let`, `pub`）與字串（`"hello"`）能正確拆分為包含顏色的 `HighlightSpan`。
  3. `buffer.rs` 整合測試：驗證切換語言後產生之可見行資料包含預期的 `spans`，且文字內容與原始行內容完全一致。
  4. 回退機制測試：驗證「Plain Text」模式下 `spans` 為空或單一純色，不影響既有幾何格網與編輯功能。

## Out of Scope

- 語法錯誤即時紅波浪線診斷（LSP 範疇）。
- 自訂外部 `.tmTheme` 主題匯入檔案對話框（先內建 GitHub 淺色主題）。
