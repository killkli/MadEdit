//! 語法突顯：syntect 增量解析與逐行快取

use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use syntect::highlighting::{
    FontStyle, HighlightIterator, HighlightState, Highlighter, Theme, ThemeSet,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};

/// 純文字（不突顯）
pub const PLAIN_TEXT: &str = "Plain Text";

/// 每隔多少行保存一次解析狀態檢查點
const CHECKPOINT_INTERVAL: usize = 32;

/// 行位元組數達此值就在該行後保存檢查點（長行不再等 32 行才存，起算點貼著長行）
const CHECKPOINT_MIN_BYTES: usize = 4096;

/// 檢查點數量上限，超過時只保留整段界線的檢查點
const CHECKPOINT_CAP: usize = 8192;

/// 需要往前補解析的行數超過此值時，改從目標行附近重新起算（避免大檔跳轉時長時間卡住）
const MAX_CATCH_UP_LINES: usize = 20_000;

/// 超長行只解析此前綴（位元組）；超過的部分以前景色顯示，避免每次編輯都對整行跑 syntect
const MAX_PARSE_LINE_BYTES: usize = 16 * 1024;

/// 單幀解析預算：超過就分時中斷，剩下的行先以前景色顯示、下幀續跑
const PARSE_BUDGET: Duration = Duration::from_millis(8);

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

/// 單一檢查點：進入 `line` 行前的解析狀態（= 前一行結束後）
struct Checkpoint {
    line: usize,
    parse: ParseState,
    hl: HighlightState,
}

/// 一份文件的語法突顯快取：保存解析狀態檢查點與已突顯的行
///
/// 編輯只標記 `dirty_from`，不丟棄後續行的快取；重解析時與編輯前留下的
/// 檢查點比對狀態，一致（收斂）即停止——典型編輯只需重解析被編輯的行。
pub struct SyntaxCache {
    syntax_name: String,
    theme_name: String,
    is_dark: bool,
    /// 依行號遞增；含編輯後仍未重驗證的「舊」檢查點（僅供收斂比對，不作為起算點）
    checkpoints: Vec<Checkpoint>,
    /// 自此行（含）起的快取不可信；None = 沒有失效
    dirty_from: Option<usize>,
    /// 分時解析中斷點：此行（含）之後尚未突顯；None = 沒有未完成的工作
    parse_frontier: Option<usize>,
    /// 單次 highlight 的解析預算（測試可調）；None = 不限時
    parse_budget: Option<Duration>,
    lines: HashMap<usize, Rc<Vec<HighlightSpan>>>,
}

impl Default for SyntaxCache {
    fn default() -> Self {
        Self {
            syntax_name: PLAIN_TEXT.to_string(),
            theme_name: String::new(),
            is_dark: false,
            checkpoints: Vec::new(),
            dirty_from: None,
            parse_frontier: None,
            parse_budget: Some(PARSE_BUDGET),
            lines: HashMap::new(),
        }
    }
}

impl SyntaxCache {
    /// 自第 `line` 行起（含）的快取失效
    ///
    /// 不立即重解析也不丟棄後續快取：保留舊檢查點供收斂比對，待下次
    /// [`Self::highlight`] 時才重解析受影響的範圍。
    pub fn invalidate_from(&mut self, line: usize) {
        self.dirty_from = Some(self.dirty_from.map_or(line, |d| d.min(line)));
    }

    /// 清除全部快取
    pub fn clear(&mut self) {
        self.checkpoints.clear();
        self.dirty_from = None;
        self.parse_frontier = None;
        self.lines.clear();
    }

    /// 檢查點數量上限；過多時只保留整段界線的檢查點
    fn trim_checkpoints(&mut self) {
        if self.checkpoints.len() > CHECKPOINT_CAP {
            self.checkpoints.retain(|c| c.line % CHECKPOINT_INTERVAL == 0);
        }
    }

    /// 文字預設前景色
    pub fn default_fg(&self) -> (u8, u8, u8) {
        theme_by_name(&self.theme_name, self.is_dark)
            .settings
            .foreground
            .map(|c| (c.r, c.g, c.b))
            .unwrap_or(if self.is_dark { (216, 222, 233) } else { (34, 34, 34) })
    }

    /// 取得 [first, first + count) 行的突顯結果；語法或主題改變時自動重建
    ///
    /// 解析工作超過每幀預算時先回傳前景色暫替區段（[`Self::has_pending`] 為真，
    /// UI 應要求重繪續跑）；列印請改用 [`Self::highlight_full`]。
    pub fn highlight(
        &mut self,
        lines: &[String],
        first: usize,
        count: usize,
        syntax_name: &str,
        theme_name: &str,
        is_dark: bool,
    ) -> Vec<Rc<Vec<HighlightSpan>>> {
        self.highlight_inner(lines, first, count, syntax_name, theme_name, is_dark, self.parse_budget)
    }

    /// 無預算上限的完整突顯（列印用）：會先跑完上次分時剩下的進度
    pub fn highlight_full(
        &mut self,
        lines: &[String],
        first: usize,
        count: usize,
        syntax_name: &str,
        theme_name: &str,
        is_dark: bool,
    ) -> Vec<Rc<Vec<HighlightSpan>>> {
        if self.parse_frontier.is_some() {
            self.highlight_inner(lines, 0, lines.len(), syntax_name, theme_name, is_dark, None);
        }
        self.highlight_inner(lines, first, count, syntax_name, theme_name, is_dark, None)
    }

    /// 是否還有本輪未解析完的行（UI 應要求重繪以續跑）
    pub fn has_pending(&self) -> bool {
        self.parse_frontier.is_some()
    }

    fn highlight_inner(
        &mut self,
        lines: &[String],
        first: usize,
        count: usize,
        syntax_name: &str,
        theme_name: &str,
        is_dark: bool,
        budget: Option<Duration>,
    ) -> Vec<Rc<Vec<HighlightSpan>>> {
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
            // 純文字：以空白文字區段代表整行前景色，並快取起來避免每幀複製整行
            self.parse_frontier = None;
            let mut out = Vec::with_capacity(end - first);
            for l in first..end {
                let cached = self.lines.entry(l).or_insert_with(|| Rc::new(vec![HighlightSpan::plain(String::new(), fg)]));
                out.push(Rc::clone(cached));
            }
            return out;
        };
        let dirty = self.dirty_from;
        let pending = self.parse_frontier;
        if dirty.is_none()
            && pending.map_or(true, |f| f >= end)
            && (first..end).all(|l| self.lines.contains_key(&l))
        {
            return (first..end).map(|l| Rc::clone(&self.lines[&l])).collect();
        }

        // 逐行結果快取只需涵蓋最近看過的範圍，過大時清空避免記憶體無限成長
        if self.lines.len() > 5000 {
            self.lines.clear();
        }
        let theme = theme_by_name(theme_name, is_dark);
        let highlighter = Highlighter::new(theme);
        let ps = syntax_set();

        // 需要重建/建立突顯的起點：
        // - 有中斷點（frontier）時，其前的行都已完成，可視範圍若在其下直接沿用快取
        // - 有編輯失效（dirty）時，自 dirty（或可視範圍起點，取較小）起狀態都須重掃
        let work_from = match (dirty, self.parse_frontier) {
            (Some(d), Some(f)) => first.max(d.min(f)),
            (Some(d), None) => first.min(d),
            (None, Some(f)) => first.max(f),
            (None, None) => first,
        };
        // 起算點：進入 work_from 前最近的「有效」檢查點。行號 > dirty 的檢查點
        // 是編輯前留下的舊狀態，從它起算會與後面的舊檢查點形成假收斂，不可作為起點；
        // 沒有有效檢查點時由第 0 行或目標附近重新起算
        let cp_limit = work_from.min(dirty.unwrap_or(usize::MAX));
        let at = self.checkpoints.partition_point(|c| c.line <= cp_limit);
        let save_checkpoints = at > 0 || work_from <= MAX_CATCH_UP_LINES;
        let (mut parse, mut hl, mut line_no) = if at > 0 {
            let cp = &self.checkpoints[at - 1];
            (cp.parse.clone(), cp.hl.clone(), cp.line)
        } else {
            let start = if work_from <= MAX_CATCH_UP_LINES { 0 } else { work_from - work_from % CHECKPOINT_INTERVAL };
            (
                ParseState::new(syntax),
                HighlightState::new(&highlighter, ScopeStack::new()),
                start,
            )
        };
        if self.checkpoints.is_empty() && save_checkpoints {
            self.checkpoints.push(Checkpoint { line: 0, parse: parse.clone(), hl: hl.clone() });
        }

        let deadline = budget.map(|b| Instant::now() + b);
        let mut bytes_since = 0usize;
        let mut converged = None;
        let mut cut_at = None;
        while line_no < end {
            let text = &lines[line_no];
            // 超長行只解析前綴（在 UTF-8 字元邊界截斷），其餘字元沿用前景色
            let with_nl = if text.len() <= MAX_PARSE_LINE_BYTES {
                format!("{}\n", text)
            } else {
                let mut cut = MAX_PARSE_LINE_BYTES;
                while !text.is_char_boundary(cut) {
                    cut -= 1;
                }
                format!("{}\n", &text[..cut])
            };
            let ops = parse.parse_line(&with_nl, ps).unwrap_or_default();
            bytes_since += text.len() + 1;
            // 需要重建的行（≥ work_from 且「已失效或沒有快取」）：產生並覆蓋快取；
            // 其餘只推進突顯狀態
            let stale = dirty.is_some_and(|d| line_no >= d);
            if line_no >= work_from && (stale || !self.lines.contains_key(&line_no)) {
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
                    // 全行同色：空白文字代表「整行以前景色顯示」
                    spans.push(HighlightSpan::plain(String::new(), fg));
                }
                self.lines.insert(line_no, Rc::new(spans));
            } else {
                // 只推進突顯狀態，不產生區段
                for _ in HighlightIterator::new(&mut hl, &ops, &with_nl, &highlighter) {}
            }
            let next = line_no + 1;
            // 收斂判定：解析完 dirty 之後的行，若進入下一行的狀態與編輯前的
            // 檢查點相同，代表更後面的舊快取仍然正確，直接沿用並停止
            if dirty.is_none_or(|d| next > d) {
                let at = self.checkpoints.partition_point(|c| c.line < next);
                if let Some(c) = self.checkpoints.get(at).filter(|c| c.line == next) {
                    if c.parse == parse && c.hl == hl {
                        if (next..end).all(|l| self.lines.contains_key(&l)) {
                            converged = Some(next);
                            break;
                        }
                    } else {
                        // 狀態已分歧：以此行後的新狀態取代舊檢查點
                        self.checkpoints[at] = Checkpoint { line: next, parse: parse.clone(), hl: hl.clone() };
                    }
                }
            }
            // 分時預算：超過就中斷。存下目前狀態當檢查點，下幀自 next 續跑；
            // 沒有剩餘工作（其餘行都有快取且未失效）就直接收工
            if let Some(deadline) = deadline {
                if Instant::now() >= deadline {
                    let rest_work = (next..end).find(|&l| {
                        !self.lines.contains_key(&l) || dirty.is_some_and(|d| l >= d)
                    });
                    if rest_work.is_some() {
                        let at = self.checkpoints.partition_point(|c| c.line < next);
                        let exists = self.checkpoints.get(at).is_some_and(|c| c.line == next);
                        if !exists {
                            self.checkpoints.insert(at, Checkpoint { line: next, parse: parse.clone(), hl: hl.clone() });
                        }
                        cut_at = Some(next);
                    }
                    break;
                }
            }
            if save_checkpoints {
                let at = self.checkpoints.partition_point(|c| c.line < next);
                let exists = self.checkpoints.get(at).is_some_and(|c| c.line == next);
                if !exists && (next % CHECKPOINT_INTERVAL == 0 || bytes_since >= CHECKPOINT_MIN_BYTES) {
                    self.checkpoints.insert(at, Checkpoint { line: next, parse: parse.clone(), hl: hl.clone() });
                    bytes_since = 0;
                    self.trim_checkpoints();
                }
            }
            line_no = next;
        }
        match converged {
            Some(_) => {
                // 收斂：next 起的舊快取仍然正確
                self.dirty_from = None;
                self.parse_frontier = None;
            }
            None => match cut_at {
                Some(next) => {
                    // 預算中斷：[work_from..next) 已重建；dirty 尚未驗證到收斂，
                    // 未重建的失效區仍不可信
                    self.dirty_from = match dirty {
                        Some(d) => Some(d.max(next)),
                        None => None,
                    };
                    self.parse_frontier = Some(next);
                }
                None => {
                    // 已處理到 end
                    self.parse_frontier = None;
                    self.dirty_from = (dirty.is_some_and(|d| end < d) && end < lines.len()).then_some(end);
                }
            },
        }
        // out 依可視範圍補齊：快取優先，沒有的行以前景色暫替
        let missing = Rc::new(vec![HighlightSpan::plain(String::new(), fg)]);
        let mut out = Vec::with_capacity(end - first);
        for l in first..end {
            out.push(self.lines.get(&l).cloned().unwrap_or_else(|| Rc::clone(&missing)));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 反覆呼叫直到分時解析完成，回傳最後一次的完整結果
    fn drain(cache: &mut SyntaxCache, lines: &[String], first: usize, count: usize) -> Vec<Rc<Vec<HighlightSpan>>> {
        loop {
            let out = cache.highlight(lines, first, count, "Rust", "InspiredGitHub", false);
            if !cache.has_pending() {
                return out;
            }
        }
    }

    #[test]
    fn test_detect_syntax() {
        assert_eq!(detect_syntax(Path::new("main.rs"), ""), "Rust");
        assert_eq!(detect_syntax(Path::new("a.py"), ""), "Python");
        assert_eq!(detect_syntax(Path::new("script"), "#!/bin/bash"), "Bourne Again Shell (bash)");
        assert_eq!(detect_syntax(Path::new("notes.unknownext"), "hello"), PLAIN_TEXT);
    }

    #[test]
    fn test_long_line_only_parses_prefix() {
        // 超過 MAX_PARSE_LINE_BYTES 的行只解析前綴，不應 panic，且區段文字總長不超過行長
        let long = format!("let s = \"{}\";", "x".repeat(MAX_PARSE_LINE_BYTES * 2));
        let lines = vec!["fn main() {".to_string(), long, "}".to_string()];
        let mut cache = SyntaxCache::default();
        let out = cache.highlight(&lines, 0, 3, "Rust", "InspiredGitHub", false);
        assert_eq!(out.len(), 3);
        let text_chars: usize = out[1].iter().map(|s| s.text.chars().count()).sum();
        assert!(text_chars <= MAX_PARSE_LINE_BYTES, "超長行只突顯前綴");
        assert!(text_chars <= lines[1].chars().count());
    }

    #[test]
    fn test_plain_text_spans_are_cached_and_empty() {
        let lines = vec!["just text".to_string(), "更多文字".to_string()];
        let mut cache = SyntaxCache::default();
        let first = cache.highlight(&lines, 0, 2, PLAIN_TEXT, "InspiredGitHub", false);
        assert_eq!(first.len(), 2);
        for line in &first {
            assert_eq!(line.len(), 1);
            assert!(line[0].text.is_empty(), "純文字以空白區段代表整行前景色");
        }
        // 第二次呼叫走快取，結果一致
        let again = cache.highlight(&lines, 0, 2, PLAIN_TEXT, "InspiredGitHub", false);
        assert_eq!(first, again);
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
    fn test_edit_converges_and_reuses_following_cache() {
        // 編輯不影響後續行狀態時，重解析到下一個檢查點即收斂，後續行沿用原快取（Rc 同源）
        // 行長 ≥ CHECKPOINT_MIN_BYTES：每行後都有檢查點，收斂即時發生
        let pad = "x".repeat(4096);
        let mut lines: Vec<String> = (0..6).map(|i| format!("let v{i} = {i}; // {pad}")).collect();
        let mut cache = SyntaxCache::default();
        let before = drain(&mut cache, &lines, 0, 6);
        lines[1].push('x');
        cache.invalidate_from(1);
        let after = drain(&mut cache, &lines, 1, 5);
        assert_eq!(after.len(), 5);
        let text: String = after[0].iter().map(|s| s.text.as_str()).collect();
        assert_eq!(text, lines[1], "編輯的行必須重建");
        assert!(Rc::ptr_eq(&before[2], &after[1]), "收斂後第 2 行必須沿用原快取");
        assert!(Rc::ptr_eq(&before[5], &after[4]), "收斂後第 5 行必須沿用原快取");
        assert_eq!(cache.dirty_from, None);
    }

    #[test]
    fn test_diverging_edit_rehighlights_following_lines() {
        // 在第 1 行打開區塊註解：後續行狀態分歧，必須重解析且顏色改變
        let pad = "x".repeat(4096);
        let mut lines: Vec<String> = (0..6).map(|i| format!("let v{i} = {i}; // {pad}")).collect();
        let mut cache = SyntaxCache::default();
        let before = drain(&mut cache, &lines, 0, 6);
        lines[1] = format!("/* let v1 // {pad}");
        cache.invalidate_from(1);
        let after = drain(&mut cache, &lines, 1, 5);
        assert!(!Rc::ptr_eq(&before[2], &after[1]), "註解狀態分歧，第 2 行必須重解析");
        assert_ne!(before[2][0].color_rgb, after[1].last().unwrap().color_rgb, "第 2 行應變為註解色");
        assert!(!Rc::ptr_eq(&before[3], &after[2]), "註解持續到結尾前都應重解析");
        // 收斂後與從頭重算的結果一致
        let mut fresh = SyntaxCache::default();
        let all = drain(&mut fresh, &lines, 0, 6);
        for i in 0..5 {
            assert_eq!(after[i], all[i + 1]);
        }
    }

    #[test]
    fn test_time_sliced_highlight_fills_progressively() {
        // 預算歸零：每幀只解析一行，未完成的行先以前景色暫替，最終結果與一次算完一致
        let pad = "x".repeat(4096);
        let lines: Vec<String> = (0..6).map(|i| format!("let v{i} = {i}; // {pad}")).collect();
        let mut cache = SyntaxCache::default();
        cache.parse_budget = Some(Duration::ZERO);
        let mut frames = 0;
        let last = loop {
            assert!(frames < 20, "分時解析必須在有限幀內完成");
            let out = cache.highlight(&lines, 0, 6, "Rust", "InspiredGitHub", false);
            frames += 1;
            assert_eq!(out.len(), 6);
            if !cache.has_pending() {
                break out;
            }
        };
        let mut fresh = SyntaxCache::default();
        let all = drain(&mut fresh, &lines, 0, 6);
        for i in 0..6 {
            assert_eq!(last[i], all[i], "分時完成後應與一次算完一致");
        }
        assert!(frames > 1, "零預算必須分多幀");
    }

    #[test]
    fn test_diverging_edit_below_viewport_rebuilds_stale_lines() {
        // 編輯行在可視範圍之前且狀態分歧（未關閉的區塊註解）：可視範圍的行必須全部重解析
        let pad = "x".repeat(4096);
        let mut lines: Vec<String> = (0..8).map(|i| format!("let v{i} = {i}; // {pad}")).collect();
        let mut cache = SyntaxCache::default();
        let before = drain(&mut cache, &lines, 0, 8);
        lines[1] = format!("/* let v1 // {pad}");
        cache.invalidate_from(1);
        let after = drain(&mut cache, &lines, 4, 4);
        assert_eq!(after.len(), 4);
        for i in 0..4 {
            assert!(!Rc::ptr_eq(&before[4 + i], &after[i]), "註解分歧，第 {} 行必須重解析", 4 + i);
        }
        // 與從頭重算的結果一致
        let mut fresh = SyntaxCache::default();
        let all = drain(&mut fresh, &lines, 0, 8);
        for i in 0..4 {
            assert_eq!(after[i], all[4 + i]);
        }
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
