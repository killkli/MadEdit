Status: closed

# Issue 01: 語法突顯引擎核心與分段色彩提取

## Description

引入 `syntect` 函式庫，建立 `src/core/syntax.rs` 核心模組，實作語法集與主題載入、副檔名自動偵測語言、單行文字高亮分段（`HighlightSpan`）演算法，並提供完整的單元測試。

## Tasks

1. 在 `Cargo.toml` 引入 `syntect = { version = "5.2", default-features = false, features = ["default-syntaxes", "default-themes", "regex-fancy"] }`。
2. 定義資料結構：
   ```rust
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub struct HighlightSpan {
       pub text: String,
       pub color_hex: String,
   }
   ```
3. 實作 `SyntaxHighlighter` 結構：
   - 快取 `SyntaxSet::load_defaults_newlines()` 與 `ThemeSet::load_defaults()`。
   - 預設色彩主題採用 `InspiredGitHub`。
   - 提供常用語言清單列表（Rust, C, C++, Python, JavaScript, TypeScript, JSON, Markdown, HTML, Plain Text 等）。
   - 實作依副檔名或檔名自動識別語言 `detect_syntax(path: &Path) -> String`。
   - 實作單行文字高亮 `highlight_line(&self, syntax_name: &str, line: &str) -> Vec<HighlightSpan>`。
4. 單元測試覆蓋：
   - 副檔名語言自動偵測（`.rs`、`.py`、`.c`、`.json`、`.md`、無副檔名）。
   - Rust 關鍵字（`fn`, `let`, `struct`）與字串著色測試。
   - Plain Text 模式無高亮回退測試。
