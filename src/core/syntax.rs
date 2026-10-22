//! 語法突顯：syntect 增量解析與逐行快取

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use syntect::highlighting::{
    FontStyle, HighlightIterator, HighlightState, Highlighter, Theme, ThemeSet,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};

/// 純文字（不突顯）
pub const PLAIN_TEXT: &str = "Plain Text";

/// 每隔多少行保存一次解析狀態檢查點
const CHECKPOINT_INTERVAL: usize = 32;

/// 需要往前補解析的行數超過此值時，改從目標行附近重新起算（避免大檔跳轉時長時間卡住）
const MAX_CATCH_UP_LINES: usize = 20_000;

/// 單一語法突顯區段
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub text: String,
    pub color_rgb: (u8, u8, u8),
    pub bold: bool,
    pub italic: bool,
}

impl HighlightSpan {
    pub fn plain(text: String, color_rgb: (u8, u8, u8)) -> Self {
        Self {
            text,
            color_rgb,
            bold: false,
            italic: false,
        }
    }
}

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

/// 所有可選語法名稱（純文字在最前，其餘依字母排序）
pub fn syntax_names() -> Vec<String> {
    let mut names: Vec<String> = syntax_set()
        .syntaxes()
        .iter()
        .filter(|s| !s.hidden && s.name != PLAIN_TEXT)
        .map(|s| s.name.clone())
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup();
    names.insert(0, PLAIN_TEXT.to_string());
    names
}

/// 所有可選的配色主題名稱
pub fn theme_names() -> Vec<String> {
    theme_set().themes.keys().cloned().collect()
}

/// 依名稱取得主題；找不到時依深淺色回退到內建預設
fn theme_by_name(name: &str, is_dark: bool) -> &'static Theme {
    let themes = &theme_set().themes;
    themes
        .get(name)
        .or_else(|| themes.get(default_theme_name(is_dark)))
        .or_else(|| themes.values().next())
        .expect("syntect 內建主題集不可能為空")
}

/// 主題的編輯區配色
pub struct ThemeColors {
    pub foreground: (u8, u8, u8),
    pub background: (u8, u8, u8),
    pub selection: Option<(u8, u8, u8)>,
    pub line_highlight: Option<(u8, u8, u8)>,
}

/// 取得主題的前景、背景、選取與目前行顏色（主題未定義時依深淺色給預設值）
pub fn theme_colors(name: &str, is_dark: bool) -> ThemeColors {
    let s = &theme_by_name(name, is_dark).settings;
    let rgb = |c: Option<syntect::highlighting::Color>| c.map(|c| (c.r, c.g, c.b));
    ThemeColors {
        foreground: rgb(s.foreground).unwrap_or(if is_dark { (216, 222, 233) } else { (34, 34, 34) }),
        background: rgb(s.background).unwrap_or(if is_dark { (43, 48, 59) } else { (255, 255, 255) }),
        selection: rgb(s.selection),
        line_highlight: rgb(s.line_highlight),
    }
}

/// 深色與淺色的預設主題
pub fn default_theme_name(is_dark: bool) -> &'static str {
    if is_dark { "base16-ocean.dark" } else { "InspiredGitHub" }
}

/// 依名稱找語法，並處理 syntect 預設集沒有的語言（以相近語法代替）
fn find_syntax(name: &str) -> Option<&'static SyntaxReference> {
    let ps = syntax_set();
    if name == PLAIN_TEXT {
        return None;
    }
    ps.find_syntax_by_name(name).or_else(|| match name {
        "TypeScript" => ps.find_syntax_by_name("JavaScript"),
        "TOML" => ps.find_syntax_by_name("YAML"),
        "Shell" => ps.find_syntax_by_extension("sh"),
        _ => ps.find_syntax_by_token(name),
    })
}

/// 依檔案路徑偵測語法名稱（副檔名 → 檔名 → 第一行，例如 shebang）
pub fn detect_syntax(path: &Path, first_line: &str) -> String {
    let ps = syntax_set();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    // syntect 預設集缺少或命名不同的副檔名
    let mapped = match ext.as_str() {
        "ts" | "mts" | "cts" | "tsx" => Some("JavaScript"),
        "toml" => Some("YAML"),
        "zsh" | "bash" => Some("Bourne Again Shell (bash)"),
        "kt" | "kts" => Some("Java"),
        "swift" => Some("Rust"),
        _ => None,
    };
    if let Some(name) = mapped.filter(|n| ps.find_syntax_by_name(n).is_some()) {
        return name.to_string();
    }
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    ps.find_syntax_by_extension(file_name)
        .or_else(|| ps.find_syntax_by_extension(&ext))
        .or_else(|| ps.find_syntax_by_first_line(first_line))
        .map(|s| s.name.clone())
        .unwrap_or_else(|| PLAIN_TEXT.to_string())
}

/// 行註解與區塊註解符號（供「註解／取消註解」使用）
pub fn comment_tokens(syntax: &str) -> (Option<&'static str>, Option<(&'static str, &'static str)>) {
    let line = match syntax {
        "Python" | "Ruby" | "Perl" | "YAML" | "TOML" | "R" | "Makefile" | "Shell"
        | "Bourne Again Shell (bash)" | "Tcl" | "Ruby on Rails" | "Git Ignore" => Some("#"),
        "SQL" | "Lua" | "Haskell" | "Literate Haskell" => Some("--"),
        "Lisp" | "Clojure" => Some(";"),
        "LaTeX" | "TeX" | "Erlang" | "MATLAB" => Some("%"),
        "Batch File" => Some("REM "),
        "HTML" | "XML" | "Markdown" | "CSS" | "Plain Text" => None,
        _ => Some("//"),
    };
    let block = match syntax {
        "HTML" | "XML" | "Markdown" => Some(("<!--", "-->")),
        "CSS" | "C" | "C++" | "C#" | "Java" | "JavaScript" | "Rust" | "Go" | "Objective-C"
        | "Scala" | "PHP" | "D" | "Groovy" => Some(("/*", "*/")),
        _ => None,
    };
    (line, block)
}

/// 一份文件的語法突顯快取：保存解析狀態檢查點與已突顯的行
pub struct SyntaxCache {
    syntax_name: String,
    theme_name: String,
    is_dark: bool,
    /// checkpoints[i] 為第 i * CHECKPOINT_INTERVAL 行開頭的解析狀態
    checkpoints: Vec<(ParseState, HighlightState)>,
    lines: HashMap<usize, Vec<HighlightSpan>>,
}

impl Default for SyntaxCache {
    fn default() -> Self {
        Self {
            syntax_name: PLAIN_TEXT.to_string(),
            theme_name: String::new(),
            is_dark: false,
            checkpoints: Vec::new(),
            lines: HashMap::new(),
        }
    }
}

impl SyntaxCache {
    /// 自第 `line` 行起（含）的快取失效
    pub fn invalidate_from(&mut self, line: usize) {
        self.checkpoints.truncate(line / CHECKPOINT_INTERVAL + 1);
        self.lines.retain(|&l, _| l < line);
    }

    /// 清除全部快取
    pub fn clear(&mut self) {
        self.checkpoints.clear();
        self.lines.clear();
    }

    /// 文字預設前景色
    pub fn default_fg(&self) -> (u8, u8, u8) {
        let theme = theme_by_name(&self.theme_name, self.is_dark);
        theme
            .settings
            .foreground
            .map(|c| (c.r, c.g, c.b))
            .unwrap_or(if self.is_dark { (216, 222, 233) } else { (34, 34, 34) })
    }

    /// 取得 [first, first + count) 行的突顯結果；語法或主題改變時自動重建
    pub fn highlight(
        &mut self,
        lines: &[String],
        first: usize,
        count: usize,
        syntax_name: &str,
        theme_name: &str,
        is_dark: bool,
    ) -> Vec<Vec<HighlightSpan>> {
        if self.syntax_name != syntax_name || self.theme_name != theme_name || self.is_dark != is_dark {
            self.syntax_name = syntax_name.to_string();
            self.theme_name = theme_name.to_string();
            self.is_dark = is_dark;
            self.clear();
        }
        let end = (first + count).min(lines.len());
        if first >= end {
            return Vec::new();
        }
        let fg = self.default_fg();
        let Some(syntax) = find_syntax(syntax_name) else {
            return lines[first..end]
                .iter()
                .map(|l| vec![HighlightSpan::plain(l.clone(), fg)])
                .collect();
        };
        if (first..end).all(|l| self.lines.contains_key(&l)) {
            return (first..end).map(|l| self.lines[&l].clone()).collect();
        }

        // 逐行結果快取只需涵蓋最近看過的範圍，過大時清空避免記憶體無限成長
        if self.lines.len() > 5000 {
            self.lines.clear();
        }
        let theme = theme_by_name(theme_name, is_dark);
        let highlighter = Highlighter::new(theme);
        let ps = syntax_set();

        // 找出起算點：最近的有效檢查點；太遠則從目標附近重新起算
        let mut cp_idx = (first / CHECKPOINT_INTERVAL).min(self.checkpoints.len().saturating_sub(1));
        let (mut parse, mut hl, mut line_no) =
            if !self.checkpoints.is_empty() && first - cp_idx * CHECKPOINT_INTERVAL <= MAX_CATCH_UP_LINES {
                let (p, h) = self.checkpoints[cp_idx].clone();
                (p, h, cp_idx * CHECKPOINT_INTERVAL)
            } else {
                let start = if first <= MAX_CATCH_UP_LINES { 0 } else { first - first % CHECKPOINT_INTERVAL };
                if start == 0 {
                    self.checkpoints.clear();
                    cp_idx = 0;
                } else {
                    // 遠距跳轉：不保存檢查點，避免與真正由開頭解析的結果混用
                    cp_idx = usize::MAX;
                }
                (
                    ParseState::new(syntax),
                    HighlightState::new(&highlighter, ScopeStack::new()),
                    start,
                )
            };
        if self.checkpoints.is_empty() && cp_idx == 0 {
            self.checkpoints.push((parse.clone(), hl.clone()));
        }

        let mut out = Vec::with_capacity(end - first);
        while line_no < end {
            let save_checkpoints = cp_idx != usize::MAX;
            if save_checkpoints
                && line_no % CHECKPOINT_INTERVAL == 0
                && line_no / CHECKPOINT_INTERVAL == self.checkpoints.len()
            {
                self.checkpoints.push((parse.clone(), hl.clone()));
            }
            let text = &lines[line_no];
            let with_nl = format!("{}\n", text);
            let ops = parse.parse_line(&with_nl, ps).unwrap_or_default();
            if line_no >= first {
                let mut spans = Vec::new();
                for (style, piece) in HighlightIterator::new(&mut hl, &ops, &with_nl, &highlighter) {
                    let piece = piece.trim_end_matches('\n');
                    if piece.is_empty() {
                        continue;
                    }
                    let c = style.foreground;
                    spans.push(HighlightSpan {
                        text: piece.to_string(),
                        color_rgb: (c.r, c.g, c.b),
                        bold: style.font_style.contains(FontStyle::BOLD),
                        italic: style.font_style.contains(FontStyle::ITALIC),
                    });
                }
                if spans.is_empty() {
                    spans.push(HighlightSpan::plain(text.clone(), fg));
                }
                self.lines.insert(line_no, spans.clone());
                out.push(spans);
            } else {
                // 只推進突顯狀態，不產生區段
                for _ in HighlightIterator::new(&mut hl, &ops, &with_nl, &highlighter) {}
            }
            line_no += 1;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_syntax() {
        assert_eq!(detect_syntax(Path::new("main.rs"), ""), "Rust");
        assert_eq!(detect_syntax(Path::new("a.py"), ""), "Python");
        assert_eq!(detect_syntax(Path::new("script"), "#!/bin/bash"), "Bourne Again Shell (bash)");
        assert_eq!(detect_syntax(Path::new("notes.unknownext"), "hello"), PLAIN_TEXT);
    }

    #[test]
    fn test_multiline_comment_state_carries_across_lines() {
        let lines: Vec<String> = ["/* start", "still comment", "*/ fn main() {}"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut cache = SyntaxCache::default();
        // 只請求第 2 行，也必須知道它位於註解內（與從頭突顯的結果一致）
        let only_second = cache.highlight(&lines, 1, 1, "Rust", "InspiredGitHub", false);
        let mut fresh = SyntaxCache::default();
        let all = fresh.highlight(&lines, 0, 3, "Rust", "InspiredGitHub", false);
        assert_eq!(only_second[0], all[1]);
        // 註解顏色應不同於一般程式碼 fn 關鍵字
        assert_ne!(all[1][0].color_rgb, all[2].last().unwrap().color_rgb);
    }

    #[test]
    fn test_invalidate_after_edit() {
        let mut lines: Vec<String> = vec!["let a = 1;".into(), "let b = 2;".into()];
        let mut cache = SyntaxCache::default();
        cache.highlight(&lines, 0, 2, "Rust", "InspiredGitHub", false);
        lines[1] = "// comment".into();
        cache.invalidate_from(1);
        let out = cache.highlight(&lines, 1, 1, "Rust", "InspiredGitHub", false);
        let text: String = out[0].iter().map(|s| s.text.as_str()).collect();
        assert_eq!(text, "// comment");
        assert_eq!(out[0][0].color_rgb, out[0].last().unwrap().color_rgb);
    }

    #[test]
    fn test_syntax_names_contains_common_languages() {
        let names = syntax_names();
        assert_eq!(names[0], PLAIN_TEXT);
        for n in ["Rust", "Python", "C++", "JavaScript", "HTML"] {
            assert!(names.iter().any(|x| x == n), "{n}");
        }
    }
}
