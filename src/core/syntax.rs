use std::path::Path;
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

/// 編輯器支援之常見程式語言與標記語言清單
#[allow(dead_code)]
pub const SUPPORTED_SYNTAXES: &[&str] = &[
    "Plain Text",
    "Rust",
    "C",
    "C++",
    "Python",
    "JavaScript",
    "TypeScript",
    "JSON",
    "Markdown",
    "HTML",
    "CSS",
    "XML",
    "YAML",
    "SQL",
];

/// 單一語法高亮文字區段（包含文字內容與顏色十六進位碼）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub text: String,
    pub color_hex: String,
}

impl HighlightSpan {
    pub fn new(text: String, color_hex: String) -> Self {
        Self { text, color_hex }
    }
}

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

/// 取得全域快取之 SyntaxSet
fn get_syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// 取得全域快取之 ThemeSet
fn get_theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

/// 取得預設淺色語法主題（優先使用 InspiredGitHub 或 base16-ocean.light）
fn get_default_theme() -> &'static Theme {
    let themes = get_theme_set();
    themes
        .themes
        .get("InspiredGitHub")
        .or_else(|| themes.themes.get("base16-ocean.light"))
        .or_else(|| themes.themes.values().next())
        .expect("主題集中至少應包含一個預設主題")
}

/// 根據檔案路徑副檔名自動識別對應之語法名稱
pub fn detect_syntax_from_path<P: AsRef<Path>>(path: P) -> &'static str {
    let path = path.as_ref();
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "rs" => "Rust",
        "c" | "h" => "C",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => "C++",
        "py" | "pyw" => "Python",
        "js" | "mjs" | "cjs" => "JavaScript",
        "ts" | "mts" | "cts" => "TypeScript",
        "json" => "JSON",
        "md" | "markdown" => "Markdown",
        "html" | "htm" => "HTML",
        "css" => "CSS",
        "xml" => "XML",
        "yaml" | "yml" => "YAML",
        "sql" => "SQL",
        _ => "Plain Text",
    }
}

/// 將單行文字依指定語法進行詞法分析並轉為色彩分段列表
pub fn highlight_line(syntax_name: &str, line: &str) -> Vec<HighlightSpan> {
    if syntax_name == "Plain Text" || line.is_empty() {
        return vec![HighlightSpan::new(line.to_string(), "#222222".to_string())];
    }

    let ps = get_syntax_set();
    let theme = get_default_theme();

    let syntax = ps
        .find_syntax_by_name(syntax_name)
        .or_else(|| ps.find_syntax_by_extension(syntax_name.to_lowercase().as_str()));

    let Some(syntax) = syntax else {
        return vec![HighlightSpan::new(line.to_string(), "#222222".to_string())];
    };

    let mut h = HighlightLines::new(syntax, theme);
    let line_with_nl = if line.ends_with('\n') {
        line.to_string()
    } else {
        format!("{}\n", line)
    };

    let Ok(ranges) = h.highlight_line(&line_with_nl, ps) else {
        return vec![HighlightSpan::new(line.to_string(), "#222222".to_string())];
    };

    let mut spans = Vec::new();
    for (style, text) in ranges {
        let clean_text = text.trim_end_matches(['\r', '\n']);
        if clean_text.is_empty() {
            continue;
        }
        let color = style.foreground;
        let hex = format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b);
        spans.push(HighlightSpan::new(clean_text.to_string(), hex));
    }

    if spans.is_empty() && !line.is_empty() {
        spans.push(HighlightSpan::new(line.to_string(), "#222222".to_string()));
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_syntax_from_path() {
        assert_eq!(detect_syntax_from_path("main.rs"), "Rust");
        assert_eq!(detect_syntax_from_path("script.py"), "Python");
        assert_eq!(detect_syntax_from_path("hello.c"), "C");
        assert_eq!(detect_syntax_from_path("header.h"), "C");
        assert_eq!(detect_syntax_from_path("native.cpp"), "C++");
        assert_eq!(detect_syntax_from_path("app.js"), "JavaScript");
        assert_eq!(detect_syntax_from_path("index.ts"), "TypeScript");
        assert_eq!(detect_syntax_from_path("data.json"), "JSON");
        assert_eq!(detect_syntax_from_path("README.md"), "Markdown");
        assert_eq!(detect_syntax_from_path("unknown.xyz"), "Plain Text");
        assert_eq!(detect_syntax_from_path("no_ext"), "Plain Text");
    }

    #[test]
    fn test_plain_text_highlighting() {
        let line = "let mut x = 42;";
        let spans = highlight_line("Plain Text", line);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, line);
        assert_eq!(spans[0].color_hex, "#222222");
    }

    #[test]
    fn test_rust_syntax_highlighting() {
        let line = "    let message = \"Hello World\";";
        let spans = highlight_line("Rust", line);
        assert!(spans.len() > 1);

        // 驗證分段拼接後完整等於原行內容
        let joined: String = spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(joined, line);

        // 驗證關鍵字與字串有不同的顏色
        let has_distinct_colors = spans
            .windows(2)
            .any(|w| w[0].color_hex != w[1].color_hex);
        assert!(has_distinct_colors);
    }
}
