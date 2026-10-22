//! 純文字轉換工具（大小寫、全半形、Tab／空白、排序、註解、格式化等），與 UI 無關

use super::editor::{char_col_to_visual_col, is_word_char, tab_columns_at, TextPosition};
use super::settings::{NumberOptions, SortOptions, WidthOptions};

// ── 大小寫 ──────────────────────────────────────────────────────

pub fn to_upper(s: &str) -> String {
    s.to_uppercase()
}

pub fn to_lower(s: &str) -> String {
    s.to_lowercase()
}

/// 反轉大小寫
pub fn invert_case(s: &str) -> String {
    s.chars()
        .flat_map(|c| -> Box<dyn Iterator<Item = char>> {
            if c.is_uppercase() {
                Box::new(c.to_lowercase())
            } else if c.is_lowercase() {
                Box::new(c.to_uppercase())
            } else {
                Box::new(std::iter::once(c))
            }
        })
        .collect()
}

/// 每個單字首字母大寫，其餘小寫
pub fn capitalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut at_word_start = true;
    for c in s.chars() {
        if is_word_char(c) {
            if at_word_start {
                out.extend(c.to_uppercase());
            } else {
                out.extend(c.to_lowercase());
            }
            at_word_start = false;
        } else {
            out.push(c);
            at_word_start = true;
        }
    }
    out
}

// ── 全形／半形 ──────────────────────────────────────────────────

/// 半形片假名（U+FF61–U+FF9F）對應的全形字元
const HALF_KANA: &str = "｡｢｣､･ｦｧｨｩｪｫｬｭｮｯｰｱｲｳｴｵｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃﾄﾅﾆﾇﾈﾉﾊﾋﾌﾍﾎﾏﾐﾑﾒﾓﾔﾕﾖﾗﾘﾙﾚﾛﾜﾝﾞﾟ";
const FULL_KANA: &str = "。「」、・ヲァィゥェォャュョッーアイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワン゛゜";

fn kana_tables() -> (Vec<char>, Vec<char>) {
    (HALF_KANA.chars().collect(), FULL_KANA.chars().collect())
}

/// 可加濁點（゛）的全形片假名
fn voiced(c: char) -> Option<char> {
    match c {
        'カ'..='ト' | 'ハ'..='ホ' => {
            // カ キ ク ケ コ ... 的濁音碼位緊接在後（ガ = カ + 1），但只有部分字元有對應
            let v = char::from_u32(c as u32 + 1)?;
            matches!(v, 'ガ' | 'ギ' | 'グ' | 'ゲ' | 'ゴ' | 'ザ' | 'ジ' | 'ズ' | 'ゼ' | 'ゾ' | 'ダ' | 'ヂ' | 'ヅ' | 'デ' | 'ド' | 'バ' | 'ビ' | 'ブ' | 'ベ' | 'ボ')
                .then_some(v)
        }
        'ウ' => Some('ヴ'),
        _ => None,
    }
}

/// 可加半濁點（゜）的全形片假名
fn semi_voiced(c: char) -> Option<char> {
    matches!(c, 'ハ' | 'ヒ' | 'フ' | 'ヘ' | 'ホ').then(|| char::from_u32(c as u32 + 2)).flatten()
}

/// 轉為全形
pub fn to_fullwidth(s: &str, opt: &WidthOptions) -> String {
    let (half, full) = kana_tables();
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() * 2);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let mapped = match c {
            ' ' if opt.space => Some('\u{3000}'),
            'a'..='z' | 'A'..='Z' if opt.letters => char::from_u32(c as u32 + 0xFEE0),
            '0'..='9' if opt.digits => char::from_u32(c as u32 + 0xFEE0),
            '!'..='~' if opt.symbols && !c.is_ascii_alphanumeric() => char::from_u32(c as u32 + 0xFEE0),
            _ if opt.katakana => half.iter().position(|&h| h == c).map(|p| full[p]),
            _ => None,
        };
        match mapped {
            Some(m) if opt.katakana && half.contains(&c) => {
                // 半形濁點／半濁點與前一字合併
                let next = chars.get(i + 1).copied();
                if next == Some('ﾞ') && voiced(m).is_some() {
                    out.push(voiced(m).unwrap());
                    i += 1;
                } else if next == Some('ﾟ') && semi_voiced(m).is_some() {
                    out.push(semi_voiced(m).unwrap());
                    i += 1;
                } else {
                    out.push(m);
                }
            }
            Some(m) => out.push(m),
            None => out.push(c),
        }
        i += 1;
    }
    out
}

/// 轉為半形
pub fn to_halfwidth(s: &str, opt: &WidthOptions) -> String {
    let (half, full) = kana_tables();
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let u = c as u32;
        let ascii = (0xFF01..=0xFF5E).contains(&u).then(|| char::from_u32(u - 0xFEE0)).flatten();
        match ascii {
            _ if c == '\u{3000}' && opt.space => out.push(' '),
            Some(a) if a.is_ascii_alphabetic() && opt.letters => out.push(a),
            Some(a) if a.is_ascii_digit() && opt.digits => out.push(a),
            Some(a) if !a.is_ascii_alphanumeric() && opt.symbols => out.push(a),
            _ if opt.katakana => {
                if let Some(p) = full.iter().position(|&f| f == c) {
                    out.push(half[p]);
                } else if let Some(base) = full.iter().find(|&&f| voiced(f) == Some(c)) {
                    out.push(half[full.iter().position(|f| f == base).unwrap()]);
                    out.push('ﾞ');
                } else if let Some(base) = full.iter().find(|&&f| semi_voiced(f) == Some(c)) {
                    out.push(half[full.iter().position(|f| f == base).unwrap()]);
                    out.push('ﾟ');
                } else {
                    out.push(c);
                }
            }
            _ => out.push(c),
        }
    }
    out
}

// ── Tab 與空白 ──────────────────────────────────────────────────

/// 將 Tab 展開為空白（依 Tab 停駐點對齊）
pub fn tabs_to_spaces(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut vcol = 0;
    for c in line.chars() {
        if c == '\t' {
            let n = tab_columns_at(vcol);
            out.extend(std::iter::repeat_n(' ', n));
            vcol += n;
        } else {
            vcol += super::editor::char_visual_columns(c);
            out.push(c);
        }
    }
    out
}

/// 將對齊到 Tab 停駐點的連續空白轉為 Tab
pub fn spaces_to_tabs(line: &str) -> String {
    let expanded = tabs_to_spaces(line);
    let ts = super::editor::tab_size();
    let mut out = String::with_capacity(expanded.len());
    let mut vcol = 0;
    let mut pending_spaces = 0;
    for c in expanded.chars() {
        if c == ' ' {
            pending_spaces += 1;
            vcol += 1;
            if vcol % ts == 0 {
                out.push(if pending_spaces > 1 { '\t' } else { ' ' });
                pending_spaces = 0;
            }
        } else {
            out.extend(std::iter::repeat_n(' ', pending_spaces));
            pending_spaces = 0;
            vcol += super::editor::char_visual_columns(c);
            out.push(c);
        }
    }
    out.extend(std::iter::repeat_n(' ', pending_spaces));
    out
}

// ── 行操作 ──────────────────────────────────────────────────────

pub fn trim_trailing(line: &str) -> String {
    line.trim_end().to_string()
}

pub fn trim_leading(line: &str) -> String {
    line.trim_start().to_string()
}

/// 刪除空行（with_spaces 為真時也刪除只含空白的行）
pub fn delete_empty_lines(lines: &[String], with_spaces: bool) -> Vec<String> {
    lines
        .iter()
        .filter(|l| if with_spaces { !l.trim().is_empty() } else { !l.is_empty() })
        .cloned()
        .collect()
}

/// 排序行
pub fn sort_lines(lines: &mut Vec<String>, opt: &SortOptions) {
    let key = |s: &String| if opt.case_sensitive { s.clone() } else { s.to_lowercase() };
    if opt.numeric {
        let num = |s: &String| -> f64 {
            let t = s.trim_start();
            let end = t
                .char_indices()
                .take_while(|&(i, c)| c.is_ascii_digit() || c == '.' || (i == 0 && (c == '-' || c == '+')))
                .map(|(i, c)| i + c.len_utf8())
                .last()
                .unwrap_or(0);
            t[..end].parse().unwrap_or(f64::MAX)
        };
        lines.sort_by(|a, b| num(a).partial_cmp(&num(b)).unwrap_or(std::cmp::Ordering::Equal).then_with(|| key(a).cmp(&key(b))));
    } else {
        lines.sort_by_key(key);
    }
    if opt.descending {
        lines.reverse();
    }
    if opt.remove_duplicates {
        if opt.case_sensitive {
            lines.dedup();
        } else {
            lines.dedup_by(|a, b| a.to_lowercase() == b.to_lowercase());
        }
    }
}

/// 產生遞增數字清單
pub fn incremental_numbers(count: usize, opt: &NumberOptions) -> Vec<String> {
    let raw: Vec<String> = (0..count)
        .map(|i| {
            let v = opt.initial + opt.step * i as i64;
            let neg = v < 0;
            let a = v.unsigned_abs();
            let body = match opt.format {
                1 if opt.uppercase_hex => format!("{a:X}"),
                1 => format!("{a:x}"),
                2 => format!("{a:o}"),
                3 => format!("{a:b}"),
                _ => a.to_string(),
            };
            if neg { format!("-{body}") } else { body }
        })
        .collect();
    let width = raw.iter().map(|s| s.len()).max().unwrap_or(0);
    raw.into_iter()
        .map(|s| {
            let padded = if opt.align_left {
                format!("{s:<width$}")
            } else if opt.zero_pad {
                match s.strip_prefix('-') {
                    Some(rest) => format!("-{:0>w$}", rest, w = width - 1),
                    None => format!("{s:0>width$}"),
                }
            } else {
                format!("{s:>width$}")
            };
            format!("{}{}{}", opt.prefix, padded, opt.postfix)
        })
        .collect()
}

/// 列塊內容靠左或靠右對齊（以行內最長者為寬度，依視覺欄寬補空白）
pub fn align_column(texts: &[String], right: bool) -> Vec<String> {
    let trimmed: Vec<&str> = texts.iter().map(|t| t.trim()).collect();
    let width_of = |s: &str| char_col_to_visual_col(s, s.chars().count());
    let max = texts.iter().map(|t| width_of(t)).max().unwrap_or(0);
    trimmed
        .iter()
        .map(|t| {
            let pad = " ".repeat(max.saturating_sub(width_of(t)));
            if right { format!("{pad}{t}") } else { format!("{t}{pad}") }
        })
        .collect()
}

/// 行註解：所有行都已註解則移除，否則加入（對齊最小縮排）
pub fn toggle_line_comment(lines: &[String], prefix: &str, force: Option<bool>) -> Vec<String> {
    let non_empty: Vec<&String> = lines.iter().filter(|l| !l.trim().is_empty()).collect();
    let all_commented = !non_empty.is_empty() && non_empty.iter().all(|l| l.trim_start().starts_with(prefix.trim_end()));
    let add = force.unwrap_or(!all_commented);
    if add {
        let indent = non_empty
            .iter()
            .map(|l| l.chars().take_while(|c| c.is_whitespace()).count())
            .min()
            .unwrap_or(0);
        lines
            .iter()
            .map(|l| {
                if l.trim().is_empty() {
                    l.clone()
                } else {
                    let byte = super::editor::char_to_byte(l, indent);
                    format!("{}{}{}", &l[..byte], prefix, &l[byte..])
                }
            })
            .collect()
    } else {
        let p = prefix.trim_end();
        lines
            .iter()
            .map(|l| {
                let lead = l.len() - l.trim_start().len();
                let rest = &l[lead..];
                match rest.strip_prefix(prefix).or_else(|| rest.strip_prefix(p)) {
                    Some(r) => format!("{}{}", &l[..lead], r),
                    None => l.clone(),
                }
            })
            .collect()
    }
}

/// 區塊註解包住文字，或移除既有的區塊註解
pub fn toggle_block_comment(text: &str, open: &str, close: &str, force: Option<bool>) -> String {
    let t = text.trim();
    let is_commented = t.starts_with(open) && t.ends_with(close) && t.len() >= open.len() + close.len();
    if force.unwrap_or(!is_commented) {
        format!("{open}{text}{close}")
    } else if is_commented {
        let start = text.find(open).unwrap();
        let end = text.rfind(close).unwrap();
        format!("{}{}{}", &text[..start], &text[start + open.len()..end], &text[end + close.len()..])
    } else {
        text.to_string()
    }
}

/// 增加縮排
pub fn indent_line(line: &str, unit: &str) -> String {
    if line.is_empty() { String::new() } else { format!("{unit}{line}") }
}

/// 減少縮排（移除一個 Tab 或最多 `size` 個空白）
pub fn unindent_line(line: &str, size: usize) -> String {
    if let Some(rest) = line.strip_prefix('\t') {
        return rest.to_string();
    }
    let n = line.chars().take(size).take_while(|&c| c == ' ').count();
    line[n..].to_string()
}

// ── 十六進位字串 ────────────────────────────────────────────────

/// 位元組轉為十六進位字串（如 "48656C6C6F" 或 "48 65 6C 6C 6F"）
pub fn to_hex_string(bytes: &[u8], spaced: bool) -> String {
    let parts: Vec<String> = bytes.iter().map(|b| format!("{b:02X}")).collect();
    parts.join(if spaced { " " } else { "" })
}

/// 由十六進位字串還原位元組（忽略空白、逗號與 0x 前綴）
pub fn from_hex_string(s: &str) -> Option<Vec<u8>> {
    super::search::parse_hex_query(s).ok()
}

// ── 字數統計 ────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WordCount {
    pub words: usize,
    pub chars: usize,
    pub spaces: usize,
    pub halfwidth: usize,
    pub fullwidth: usize,
    pub cjk: usize,
    pub lines: usize,
}

/// 統計字數：拉丁文以空白分隔計字，CJK 字元每字各算一字
pub fn word_count(text: &str) -> WordCount {
    let mut wc = WordCount {
        lines: if text.is_empty() { 0 } else { text.split('\n').count() },
        ..Default::default()
    };
    let mut in_word = false;
    for c in text.chars() {
        if c == '\n' || c == '\r' {
            in_word = false;
            continue;
        }
        wc.chars += 1;
        let is_cjk = matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F);
        if c.is_whitespace() {
            wc.spaces += 1;
            in_word = false;
        } else if is_cjk {
            wc.cjk += 1;
            wc.words += 1;
            in_word = false;
        } else if !in_word {
            wc.words += 1;
            in_word = true;
        }
        if super::editor::char_visual_columns(c) >= 2 && c != '\t' {
            wc.fullwidth += 1;
        } else {
            wc.halfwidth += 1;
        }
    }
    wc
}

// ── 格式化與轉換 ────────────────────────────────────────────────

/// JSON 美化（保留鍵的原始順序）
pub fn format_json(s: &str, indent: usize) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(s).map_err(|e| format!("JSON 格式錯誤：{e}"))?;
    let indent_str = " ".repeat(indent);
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(indent_str.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    serde::Serialize::serialize(&value, &mut ser).map_err(|e| e.to_string())?;
    String::from_utf8(buf).map_err(|e| e.to_string())
}

/// XML 美化：每個標籤一行並依層級縮排（保留註解、CDATA 與處理指令）
pub fn format_xml(s: &str, indent: usize) -> Result<String, String> {
    let mut out = String::new();
    let mut depth: usize = 0;
    let mut rest = s.trim();
    let pad = |d: usize| " ".repeat(d * indent);
    while !rest.is_empty() {
        if rest.starts_with('<') {
            let end_pat = if rest.starts_with("<!--") {
                "-->"
            } else if rest.starts_with("<![CDATA[") {
                "]]>"
            } else if rest.starts_with("<?") {
                "?>"
            } else {
                ">"
            };
            let end = rest.find(end_pat).ok_or("XML 標籤未正確結束")? + end_pat.len();
            let tag = &rest[..end];
            rest = rest[end..].trim_start();
            if tag.starts_with("</") {
                depth = depth.checked_sub(1).ok_or("XML 結束標籤多於開始標籤")?;
                out.push_str(&pad(depth));
                out.push_str(tag);
                out.push('\n');
            } else if tag.starts_with("<!") || tag.starts_with("<?") || tag.ends_with("/>") {
                out.push_str(&pad(depth));
                out.push_str(tag);
                out.push('\n');
            } else {
                // 開始標籤：若緊接文字與結束標籤，整段放同一行
                let text_end = rest.find('<').unwrap_or(rest.len());
                let text = rest[..text_end].trim();
                let after = &rest[text_end..];
                if !text.is_empty() && after.starts_with("</") {
                    let close_end = after.find('>').ok_or("XML 標籤未正確結束")? + 1;
                    out.push_str(&pad(depth));
                    out.push_str(tag);
                    out.push_str(text);
                    out.push_str(&after[..close_end]);
                    out.push('\n');
                    rest = after[close_end..].trim_start();
                } else {
                    out.push_str(&pad(depth));
                    out.push_str(tag);
                    out.push('\n');
                    depth += 1;
                }
            }
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            let text = rest[..end].trim();
            if !text.is_empty() {
                out.push_str(&pad(depth));
                out.push_str(text);
                out.push('\n');
            }
            rest = &rest[end..];
        }
    }
    if depth != 0 {
        return Err("XML 有未關閉的標籤".into());
    }
    Ok(out.trim_end().to_string())
}

/// Markdown 轉 HTML
pub fn markdown_to_html(s: &str) -> String {
    let mut opts = pulldown_cmark::Options::empty();
    opts.insert(pulldown_cmark::Options::ENABLE_TABLES);
    opts.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    opts.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);
    let parser = pulldown_cmark::Parser::new_ext(s, opts);
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);
    html
}

/// HTML 轉純文字：移除標籤、腳本與樣式，區塊元素換行，解碼常見實體
pub fn html_to_text(s: &str) -> String {
    // regex 套件不支援反向參照，因此三種區塊各自比對
    let mut s = s.to_string();
    for tag in ["script", "style", "head"] {
        let re_drop = regex::Regex::new(&format!(r"(?is)<{tag}\b.*?</{tag}\s*>")).unwrap();
        s = re_drop.replace_all(&s, "").into_owned();
    }
    let re_break = regex::Regex::new(r"(?i)<\s*(br|/p|/div|/li|/tr|/h[1-6]|/blockquote|/pre|/table)\b[^>]*>").unwrap();
    let s = re_break.replace_all(&s, "\n");
    let re_li = regex::Regex::new(r"(?i)<\s*li\b[^>]*>").unwrap();
    let s = re_li.replace_all(&s, "• ");
    let re_tag = regex::Regex::new(r"(?s)<[^>]*>").unwrap();
    let s = re_tag.replace_all(&s, "");
    let s = decode_html_entities(&s);
    // 收斂多餘空行
    let re_blank = regex::Regex::new(r"\n[ \t]*\n(?:[ \t]*\n)+").unwrap();
    re_blank.replace_all(s.trim(), "\n\n").into_owned()
}

fn decode_html_entities(s: &str) -> String {
    let re = regex::Regex::new(r"&(#x[0-9a-fA-F]+|#[0-9]+|[a-zA-Z]+);").unwrap();
    re.replace_all(s, |caps: &regex::Captures| {
        let e = &caps[1];
        let decoded = if let Some(hex) = e.strip_prefix("#x").or_else(|| e.strip_prefix("#X")) {
            u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
        } else if let Some(dec) = e.strip_prefix('#') {
            dec.parse().ok().and_then(char::from_u32)
        } else {
            match e {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                "copy" => Some('©'),
                "reg" => Some('®'),
                "hellip" => Some('…'),
                "mdash" => Some('—'),
                "ndash" => Some('–'),
                _ => None,
            }
        };
        decoded.map(String::from).unwrap_or_else(|| caps[0].to_string())
    })
    .into_owned()
}

// ── 括號配對 ────────────────────────────────────────────────────

const BRACE_PAIRS: [(char, char); 4] = [('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')];

/// 游標左右兩側的括號及其對應括號位置
pub fn find_brace_pair(lines: &[String], pos: TextPosition) -> Option<(TextPosition, TextPosition)> {
    let line: Vec<char> = lines.get(pos.line)?.chars().collect();
    let candidates = [pos.col, pos.col.wrapping_sub(1)];
    for col in candidates {
        let Some(&c) = line.get(col) else { continue };
        for (open, close) in BRACE_PAIRS {
            if c == open {
                return find_match(lines, TextPosition::new(pos.line, col), open, close, true)
                    .map(|m| (TextPosition::new(pos.line, col), m));
            }
            if c == close {
                return find_match(lines, TextPosition::new(pos.line, col), open, close, false)
                    .map(|m| (TextPosition::new(pos.line, col), m));
            }
        }
    }
    None
}

/// 向左或向右找出包住游標的最近括號（供「跳到左／右括號」）
pub fn enclosing_brace(lines: &[String], pos: TextPosition, left: bool) -> Option<TextPosition> {
    let mut depth = [0i32; 4];
    let mut line = pos.line;
    let mut chars: Vec<char> = lines.get(line)?.chars().collect();
    let mut col = pos.col as isize;
    let mut scanned = 0usize;
    loop {
        if left {
            col -= 1;
            while col < 0 {
                if line == 0 {
                    return None;
                }
                line -= 1;
                chars = lines[line].chars().collect();
                col = chars.len() as isize - 1;
            }
        } else {
            while col as usize >= chars.len() {
                line += 1;
                if line >= lines.len() {
                    return None;
                }
                chars = lines[line].chars().collect();
                col = 0;
            }
        }
        let c = chars[col as usize];
        for (i, (open, close)) in BRACE_PAIRS.iter().enumerate().take(3) {
            let (inc, dec) = if left { (*close, *open) } else { (*open, *close) };
            if c == inc {
                depth[i] += 1;
            } else if c == dec {
                if depth[i] == 0 {
                    return Some(TextPosition::new(line, col as usize));
                }
                depth[i] -= 1;
            }
        }
        if !left {
            col += 1;
        }
        scanned += 1;
        if scanned > 2_000_000 {
            return None;
        }
    }
}

fn find_match(lines: &[String], start: TextPosition, open: char, close: char, forward: bool) -> Option<TextPosition> {
    let mut depth = 0i32;
    let mut scanned = 0usize;
    if forward {
        for (li, line) in lines.iter().enumerate().skip(start.line) {
            let skip = if li == start.line { start.col } else { 0 };
            for (ci, c) in line.chars().enumerate().skip(skip) {
                scanned += 1;
                if c == open {
                    depth += 1;
                } else if c == close {
                    depth -= 1;
                    if depth == 0 {
                        return Some(TextPosition::new(li, ci));
                    }
                }
            }
            if scanned > 2_000_000 {
                return None;
            }
        }
    } else {
        for li in (0..=start.line).rev() {
            let chars: Vec<char> = lines[li].chars().collect();
            let end = if li == start.line { start.col + 1 } else { chars.len() };
            for ci in (0..end.min(chars.len())).rev() {
                scanned += 1;
                let c = chars[ci];
                if c == close {
                    depth += 1;
                } else if c == open {
                    depth -= 1;
                    if depth == 0 {
                        return Some(TextPosition::new(li, ci));
                    }
                }
            }
            if scanned > 2_000_000 {
                return None;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_case_conversions() {
        assert_eq!(invert_case("aBc 中"), "AbC 中");
        assert_eq!(capitalize("hello wORLD foo_bar"), "Hello World Foo_bar");
    }

    #[test]
    fn test_width_conversions() {
        let o = WidthOptions::default();
        assert_eq!(to_fullwidth("Ab1! ", &o), "Ａｂ１！\u{3000}");
        assert_eq!(to_halfwidth("Ａｂ１！\u{3000}", &o), "Ab1! ");
        assert_eq!(to_fullwidth("ｶﾞｷﾞﾊﾟｱ", &o), "ガギパア");
        assert_eq!(to_halfwidth("ガパア", &o), "ｶﾞﾊﾟｱ");
        let only_digits = WidthOptions { letters: false, symbols: false, space: false, katakana: false, digits: true };
        assert_eq!(to_fullwidth("a1", &only_digits), "a１");
    }

    #[test]
    fn test_tabs_and_spaces() {
        assert_eq!(tabs_to_spaces("a\tb"), "a   b");
        assert_eq!(spaces_to_tabs("        x"), "\t\tx");
        assert_eq!(spaces_to_tabs("a   b"), "a\tb");
        assert_eq!(spaces_to_tabs("a b"), "a b");
    }

    #[test]
    fn test_sort_and_dedup() {
        let mut v: Vec<String> = ["b", "A", "a", "10", "9"].iter().map(|s| s.to_string()).collect();
        sort_lines(&mut v, &SortOptions { case_sensitive: false, remove_duplicates: true, ..Default::default() });
        assert_eq!(v, vec!["10", "9", "A", "b"]);
        let mut n: Vec<String> = ["10", "9", "-1"].iter().map(|s| s.to_string()).collect();
        sort_lines(&mut n, &SortOptions { numeric: true, ..Default::default() });
        assert_eq!(n, vec!["-1", "9", "10"]);
    }

    #[test]
    fn test_incremental_numbers() {
        let o = NumberOptions { initial: 8, step: 1, total: 3, zero_pad: true, ..Default::default() };
        assert_eq!(incremental_numbers(3, &o), vec!["08", "09", "10"]);
        let hex = NumberOptions { initial: 255, step: 1, format: 1, zero_pad: true, prefix: "0x".into(), ..Default::default() };
        assert_eq!(incremental_numbers(2, &hex), vec!["0x0FF", "0x100"]);
    }

    #[test]
    fn test_comments_and_indent() {
        let lines: Vec<String> = ["  a", "", "  b"].iter().map(|s| s.to_string()).collect();
        let c = toggle_line_comment(&lines, "// ", None);
        assert_eq!(c, vec!["  // a", "", "  // b"]);
        assert_eq!(toggle_line_comment(&c, "// ", None), lines);
        assert_eq!(toggle_block_comment("x", "/*", "*/", None), "/*x*/");
        assert_eq!(toggle_block_comment("/*x*/", "/*", "*/", None), "x");
        assert_eq!(unindent_line("\tx", 4), "x");
        assert_eq!(unindent_line("      x", 4), "  x");
    }

    #[test]
    fn test_hex_string_and_word_count() {
        assert_eq!(to_hex_string(b"Hi", true), "48 69");
        assert_eq!(from_hex_string("48 69").unwrap(), b"Hi");
        let wc = word_count("Hello world 中文\nok");
        assert_eq!(wc.words, 5);
        assert_eq!(wc.cjk, 2);
        assert_eq!(wc.lines, 2);
    }

    #[test]
    fn test_formatters() {
        let j = format_json(r#"{"b":1,"a":[1,2]}"#, 2).unwrap();
        assert!(j.starts_with("{\n  \"b\": 1"));
        let x = format_xml("<a><b>t</b><c/></a>", 2).unwrap();
        assert_eq!(x, "<a>\n  <b>t</b>\n  <c/>\n</a>");
        assert!(format_xml("<a><b></a>", 2).is_err() || format_xml("<a>", 2).is_err());
        assert!(markdown_to_html("# T\n\n*x*").contains("<h1>T</h1>"));
        assert_eq!(html_to_text("<p>A &amp; B</p><script>x</script><br>C"), "A & B\n\nC");
    }

    #[test]
    fn test_brace_matching() {
        let lines: Vec<String> = ["fn x() {", "  (a[1])", "}"].iter().map(|s| s.to_string()).collect();
        let (a, b) = find_brace_pair(&lines, TextPosition::new(0, 7)).unwrap();
        assert_eq!((a, b), (TextPosition::new(0, 7), TextPosition::new(2, 0)));
        let (_, m) = find_brace_pair(&lines, TextPosition::new(1, 8)).unwrap();
        assert_eq!(m, TextPosition::new(1, 2));
        assert_eq!(enclosing_brace(&lines, TextPosition::new(1, 5), true), Some(TextPosition::new(1, 4)));
        assert_eq!(enclosing_brace(&lines, TextPosition::new(1, 1), false), Some(TextPosition::new(2, 0)));
    }
}
