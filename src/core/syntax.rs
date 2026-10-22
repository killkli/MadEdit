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
    "C#",
    "Java",
    "Go",
    "Python",
    "JavaScript",
    "TypeScript",
    "JSON",
    "TOML",
    "Markdown",
    "HTML",
    "CSS",
    "XML",
    "YAML",
    "SQL",
    "Shell",
];

/// 單一語法高亮文字區段（包含文字內容與 RGB 色彩數值）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub text: String,
    pub color_rgb: (u8, u8, u8),
}

impl HighlightSpan {
    pub fn new(text: String, color_rgb: (u8, u8, u8)) -> Self {
        Self { text, color_rgb }
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

/// 依據深淺色模式取得語法主題（深色採用 base16-ocean.dark，淺色採用 InspiredGitHub）
fn get_theme(is_dark: bool) -> &'static Theme {
    let themes = get_theme_set();
    if is_dark {
        themes
            .themes
            .get("base16-ocean.dark")
            .or_else(|| themes.themes.get("Solarized (dark)"))
            .or_else(|| themes.themes.get("base16-eighties.dark"))
            .or_else(|| themes.themes.values().next())
            .expect("主題集中至少應包含一個預設主題")
    } else {
        themes
            .themes
            .get("InspiredGitHub")
            .or_else(|| themes.themes.get("base16-ocean.light"))
            .or_else(|| themes.themes.values().next())
            .expect("主題集中至少應包含一個預設主題")
    }
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
        "cs" => "C#",
        "java" => "Java",
        "go" => "Go",
        "py" | "pyw" => "Python",
        "js" | "mjs" | "cjs" => "JavaScript",
        "ts" | "mts" | "cts" => "TypeScript",
        "json" => "JSON",
        "toml" => "TOML",
        "md" | "markdown" => "Markdown",
        "html" | "htm" => "HTML",
        "css" => "CSS",
        "xml" => "XML",
        "yaml" | "yml" => "YAML",
        "sql" => "SQL",
        "sh" | "bash" | "zsh" | "ps1" => "Shell",
        _ => "Plain Text",
    }
}

/// 批次將多行文字依指定語法與深淺色主題進行詞法分析並轉為色彩分段列表
pub fn highlight_lines_with_theme(
    syntax_name: &str,
    lines: &[String],
    is_dark: bool,
) -> Vec<Vec<HighlightSpan>> {
    if lines.is_empty() {
        return Vec::new();
    }

    let default_fg = if is_dark { (216, 222, 233) } else { (34, 34, 34) };

    if syntax_name == "Plain Text" {
        return lines
            .iter()
            .map(|l| vec![HighlightSpan::new(l.clone(), default_fg)])
            .collect();
    }

    let ps = get_syntax_set();
    let theme = get_theme(is_dark);

    let syntax = ps
        .find_syntax_by_name(syntax_name)
        .or_else(|| ps.find_syntax_by_extension(syntax_name.to_lowercase().as_str()))
        .or_else(|| match syntax_name {
            "C#" => ps
                .find_syntax_by_extension("cs")
                .or_else(|| ps.find_syntax_by_name("Java"))
                .or_else(|| ps.find_syntax_by_name("C++")),
            "TypeScript" => ps.find_syntax_by_name("JavaScript"),
            "TOML" => ps.find_syntax_by_name("YAML"),
            "Shell" => ps
                .find_syntax_by_extension("sh")
                .or_else(|| ps.find_syntax_by_name("Bourne Again Shell (bash)")),
            _ => None,
        });

    let Some(syntax) = syntax else {
        return lines
            .iter()
            .map(|l| vec![HighlightSpan::new(l.clone(), default_fg)])
            .collect();
    };

    let mut h = HighlightLines::new(syntax, theme);
    let mut result = Vec::with_capacity(lines.len());

    for line in lines {
        if line.is_empty() {
            result.push(vec![HighlightSpan::new(String::new(), default_fg)]);
            continue;
        }

        let line_with_nl = if line.ends_with('\n') {
            line.to_string()
        } else {
            format!("{}\n", line)
        };

        let Ok(ranges) = h.highlight_line(&line_with_nl, ps) else {
            result.push(vec![HighlightSpan::new(line.clone(), default_fg)]);
            continue;
        };

        let mut spans = Vec::new();
        for (style, text) in ranges {
            let clean_text = text.trim_end_matches(['\r', '\n']);
            if clean_text.is_empty() {
                continue;
            }
            let color = style.foreground;
            spans.push(HighlightSpan::new(
                clean_text.to_string(),
                (color.r, color.g, color.b),
            ));
        }

        if spans.is_empty() {
            spans.push(HighlightSpan::new(line.clone(), default_fg));
        }

        result.push(spans);
    }

    result
}

/// 批次將多行文字依指定語法進行詞法分析並轉為色彩分段列表（預設淺色主題）
pub fn highlight_lines(syntax_name: &str, lines: &[String]) -> Vec<Vec<HighlightSpan>> {
    highlight_lines_with_theme(syntax_name, lines, false)
}

/// 將單行文字依指定語法進行詞法分析並轉為色彩分段列表
#[allow(dead_code)]
pub fn highlight_line(syntax_name: &str, line: &str) -> Vec<HighlightSpan> {
    highlight_lines(syntax_name, &[line.to_string()])
        .into_iter()
        .next()
        .unwrap_or_else(|| vec![HighlightSpan::new(line.to_string(), (34, 34, 34))])
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
        assert_eq!(detect_syntax_from_path("RequestPasswordExtension.cs"), "C#");
        assert_eq!(detect_syntax_from_path("Main.java"), "Java");
        assert_eq!(detect_syntax_from_path("server.go"), "Go");
        assert_eq!(detect_syntax_from_path("Cargo.toml"), "TOML");
        assert_eq!(detect_syntax_from_path("deploy.sh"), "Shell");
        assert_eq!(detect_syntax_from_path("app.js"), "JavaScript");
        assert_eq!(detect_syntax_from_path("index.ts"), "TypeScript");
        assert_eq!(detect_syntax_from_path("data.json"), "JSON");
        assert_eq!(detect_syntax_from_path("README.md"), "Markdown");
        assert_eq!(detect_syntax_from_path("unknown.xyz"), "Plain Text");
        assert_eq!(detect_syntax_from_path("no_ext"), "Plain Text");
    }

    #[test]
    fn test_csharp_and_dark_theme_highlighting() {
        let lines = vec![
            "using Microsoft.AspNetCore.Builder;".to_string(),
            "public static class RequestPasswordExtension {".to_string(),
        ];
        let dark_spans = highlight_lines_with_theme("C#", &lines, true);
        let light_spans = highlight_lines_with_theme("C#", &lines, false);
        assert_eq!(dark_spans.len(), 2);
        assert_eq!(light_spans.len(), 2);
        assert!(dark_spans[1].len() > 1);
        // 深淺色主題的關鍵字色彩應有所不同
        assert_ne!(dark_spans[1][0].color_rgb, light_spans[1][0].color_rgb);
    }

    #[test]
    fn test_plain_text_highlighting() {
        let line = "let mut x = 42;";
        let spans = highlight_line("Plain Text", line);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, line);
        assert_eq!(spans[0].color_rgb, (34, 34, 34));
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
            .any(|w| w[0].color_rgb != w[1].color_rgb);
        assert!(has_distinct_colors);
    }

    #[test]
    fn test_multiline_syntax_state_propagation() {
        let lines = vec![
            "/* 開始多行註解".to_string(),
            "   這是第二行註解".to_string(),
            "   這是第三行註解 */".to_string(),
            "let code = 100;".to_string(),
        ];
        let highlighted = highlight_lines("Rust", &lines);
        assert_eq!(highlighted.len(), 4);

        // 驗證第二行（註解內部）與第一行的顏色一致（皆為註解色）
        let comment_color_line1 = highlighted[0][0].color_rgb;
        let comment_color_line2 = highlighted[1][0].color_rgb;
        assert_eq!(comment_color_line1, comment_color_line2);

        // 驗證第四行的 let 關鍵字顏色與註解色不同
        let let_keyword_color = highlighted[3][0].color_rgb;
        assert_ne!(let_keyword_color, comment_color_line1);
    }
}
