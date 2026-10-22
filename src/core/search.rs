#![allow(dead_code)]

use regex::{Regex, RegexBuilder};
use super::editor::TextPosition;

/// 搜尋匹配選項
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SearchOptions {
    pub match_case: bool,
    pub whole_word: bool,
    pub use_regex: bool,
}

impl SearchOptions {
    pub fn new(match_case: bool, whole_word: bool, use_regex: bool) -> Self {
        Self {
            match_case,
            whole_word,
            use_regex,
        }
    }
}

/// 單一搜尋匹配結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub start: TextPosition,
    pub end: TextPosition,
    pub byte_offset: usize,
    pub byte_len: usize,
    pub matched_text: String,
}

impl SearchMatch {
    #[allow(dead_code)]
    pub fn new(
        start: TextPosition,
        end: TextPosition,
        byte_offset: usize,
        byte_len: usize,
        matched_text: String,
    ) -> Self {
        Self {
            start,
            end,
            byte_offset,
            byte_len,
            matched_text,
        }
    }
}

/// 解析十六進位搜尋字串（支援如 "48 65 6C 6C 6F" 或 "48656C6C6F" 或 "0x48 0x65"）
pub fn parse_hex_query(query: &str) -> Result<Vec<u8>, String> {
    let clean = query
        .replace("0x", "")
        .replace("0X", "")
        .replace([' ', '\t', '\r', '\n', ',', '-'], "");

    if clean.is_empty() {
        return Err("搜尋內容不可為空".to_string());
    }
    // 先確認全為 ASCII 十六進位數字：之後以位元組切片取兩字一組才安全（中文等多位元組字元會切在字元中間而 panic）
    if let Some(c) = clean.chars().find(|c| !c.is_ascii_hexdigit()) {
        return Err(format!("無效的十六進位字元: {}", c));
    }
    if !clean.len().is_multiple_of(2) {
        return Err("十六進位序列長度必須為偶數".to_string());
    }

    let mut bytes = Vec::with_capacity(clean.len() / 2);
    for i in (0..clean.len()).step_by(2) {
        let byte_str = &clean[i..i + 2];
        let byte = u8::from_str_radix(byte_str, 16)
            .map_err(|_| format!("無效的十六進位字元: {}", byte_str))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

/// 在位元組串流中搜尋 Hex Pattern，回傳所有匹配的位元組位移（Offsets）
pub fn find_hex_matches(data: &[u8], pattern: &[u8]) -> Vec<usize> {
    if pattern.is_empty() || data.len() < pattern.len() {
        return Vec::new();
    }
    let mut matches = Vec::new();
    for i in 0..=(data.len() - pattern.len()) {
        if &data[i..i + pattern.len()] == pattern {
            matches.push(i);
        }
    }
    matches
}

/// 建構比對用之正規表達式物件
pub fn build_regex(query: &str, options: &SearchOptions) -> Result<Regex, String> {
    if query.is_empty() {
        return Err("搜尋關鍵字不可為空".to_string());
    }

    let pattern_str = if options.use_regex {
        if options.whole_word {
            format!(r"\b(?:{})\b", query)
        } else {
            query.to_string()
        }
    } else {
        let escaped = regex::escape(query);
        if options.whole_word {
            format!(r"\b{}\b", escaped)
        } else {
            escaped
        }
    };

    RegexBuilder::new(&pattern_str)
        .case_insensitive(!options.match_case)
        .multi_line(true)
        .build()
        .map_err(|e| format!("無效的正規表達式: {}", e))
}

/// 在文字行列表中搜尋所有匹配項目
pub fn find_all_matches(
    lines: &[String],
    query: &str,
    options: &SearchOptions,
) -> Result<Vec<SearchMatch>, String> {
    let re = build_regex(query, options)?;
    let mut matches = Vec::new();
    let mut line_start_byte_offset = 0;

    for (line_idx, line) in lines.iter().enumerate() {
        for m in re.find_iter(line) {
            let start_byte = m.start();
            let end_byte = m.end();
            let start_col = line[..start_byte].chars().count();
            let end_col = start_col + line[start_byte..end_byte].chars().count();

            matches.push(SearchMatch {
                start: TextPosition::new(line_idx, start_col),
                end: TextPosition::new(line_idx, end_col),
                byte_offset: line_start_byte_offset + start_byte,
                byte_len: end_byte - start_byte,
                matched_text: m.as_str().to_string(),
            });
        }
        line_start_byte_offset += line.len() + 1; // 預估包含換行字元
    }

    Ok(matches)
}

/// 計算匹配總次數
#[allow(dead_code)]
pub fn count_matches(
    lines: &[String],
    query: &str,
    options: &SearchOptions,
) -> Result<usize, String> {
    let matches = find_all_matches(lines, query, options)?;
    Ok(matches.len())
}

/// 尋找游標之後的下一個匹配項目（環狀循環）
pub fn find_next_match(
    matches: &[SearchMatch],
    current_pos: TextPosition,
) -> Option<(usize, SearchMatch)> {
    if matches.is_empty() {
        return None;
    }
    // 尋找第一個起點在 current_pos 之後的項目
    for (idx, m) in matches.iter().enumerate() {
        if m.start >= current_pos {
            return Some((idx, m.clone()));
        }
    }
    // 環狀回起點
    Some((0, matches[0].clone()))
}

/// 尋找游標之前的上一個匹配項目（環狀循環）
pub fn find_prev_match(
    matches: &[SearchMatch],
    current_pos: TextPosition,
) -> Option<(usize, SearchMatch)> {
    if matches.is_empty() {
        return None;
    }
    // 從後往前找第一個起點在 current_pos 之前的項目
    for (idx, m) in matches.iter().enumerate().rev() {
        if m.start < current_pos {
            return Some((idx, m.clone()));
        }
    }
    // 環狀回末尾
    let last_idx = matches.len() - 1;
    Some((last_idx, matches[last_idx].clone()))
}

/// 全部取代：將所有符合條件之字串替換為目標字串，回傳更新後之行列表與取代總次數
pub fn replace_all_matches(
    lines: &[String],
    query: &str,
    replace_with: &str,
    options: &SearchOptions,
) -> Result<(Vec<String>, usize), String> {
    let re = build_regex(query, options)?;
    let mut new_lines = Vec::with_capacity(lines.len());
    let mut total_replacements = 0;

    for line in lines {
        let mut count = 0;
        let replaced = re.replace_all(line, |caps: &regex::Captures| {
            count += 1;
            if options.use_regex {
                let mut dst = String::new();
                caps.expand(replace_with, &mut dst);
                dst
            } else {
                replace_with.to_string()
            }
        });
        total_replacements += count;
        new_lines.push(replaced.into_owned());
    }

    Ok((new_lines, total_replacements))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_case_sensitive_and_whole_word() {
        let lines = vec![
            "Apple apple Application".to_string(),
            "an apple a day".to_string(),
        ];

        // 1. 忽略大小寫
        let opt_ci = SearchOptions::new(false, false, false);
        let m1 = find_all_matches(&lines, "apple", &opt_ci).unwrap();
        assert_eq!(m1.len(), 3); // "Apple", "apple", "apple"

        // 2. 區分大小寫
        let opt_cs = SearchOptions::new(true, false, false);
        let m2 = find_all_matches(&lines, "apple", &opt_cs).unwrap();
        assert_eq!(m2.len(), 2); // 只有小寫 "apple"

        // 3. 全字匹配 + 區分大小寫
        let opt_ww = SearchOptions::new(true, true, false);
        let m3 = find_all_matches(&lines, "apple", &opt_ww).unwrap();
        assert_eq!(m3.len(), 2);
        assert_eq!(m3[0].start, TextPosition::new(0, 6)); // 第 0 行的 "apple"
        assert_eq!(m3[1].start, TextPosition::new(1, 3)); // 第 1 行的 "apple"
    }

    #[test]
    fn test_search_regex() {
        let lines = vec![
            "Error: code 404 not found".to_string(),
            "Warning: code 500 error".to_string(),
        ];

        let opt_regex = SearchOptions::new(false, false, true);
        let m = find_all_matches(&lines, r"code \d+", &opt_regex).unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].matched_text, "code 404");
        assert_eq!(m[1].matched_text, "code 500");
    }

    #[test]
    fn test_hex_query_parse_and_find() {
        let query = "48 65 6C 6C 6F";
        let parsed = parse_hex_query(query).unwrap();
        assert_eq!(parsed, b"Hello");

        let data = b"Header...Hello World...Hello!";
        let matches = find_hex_matches(data, &parsed);
        assert_eq!(matches, vec![9, 23]);
    }

    #[test]
    fn test_hex_query_rejects_non_hex_without_panic() {
        // 位元組長度為偶數的多位元組字元曾因位元組切片落在字元中間而 panic
        assert!(parse_hex_query("中文").is_err());
        assert!(parse_hex_query("A中").is_err());
        // from_str_radix 會接受正號，必須明確拒絕
        assert!(parse_hex_query("+1").is_err());
    }

    #[test]
    fn test_find_next_and_prev_navigation() {
        let lines = vec!["A B A C A".to_string()];
        let opt = SearchOptions::default();
        let matches = find_all_matches(&lines, "A", &opt).unwrap();
        assert_eq!(matches.len(), 3);

        // 游標在 (0, 0)，找下一個就是第 0 個
        let (idx, _) = find_next_match(&matches, TextPosition::new(0, 0)).unwrap();
        assert_eq!(idx, 0);

        // 游標在 (0, 1)，找下一個是第 1 個 (offset 4)
        let (idx, m) = find_next_match(&matches, TextPosition::new(0, 1)).unwrap();
        assert_eq!(idx, 1);
        assert_eq!(m.start, TextPosition::new(0, 4));

        // 游標在 (0, 9)，找下一個環狀回第 0 個
        let (idx, _) = find_next_match(&matches, TextPosition::new(0, 9)).unwrap();
        assert_eq!(idx, 0);

        // 游標在 (0, 9)，找上一個是第 2 個 (offset 8)
        let (idx, m) = find_prev_match(&matches, TextPosition::new(0, 9)).unwrap();
        assert_eq!(idx, 2);
        assert_eq!(m.start, TextPosition::new(0, 8));

        // 游標在 (0, 0)，找上一個環狀回最後一個 (idx 2)
        let (idx, _) = find_prev_match(&matches, TextPosition::new(0, 0)).unwrap();
        assert_eq!(idx, 2);
    }

    #[test]
    fn test_replace_all_matches() {
        let lines = vec![
            "foo bar foo".to_string(),
            "foo baz".to_string(),
        ];
        let opt = SearchOptions::default();
        let (replaced, count) = replace_all_matches(&lines, "foo", "qux", &opt).unwrap();
        assert_eq!(count, 3);
        assert_eq!(replaced, vec!["qux bar qux", "qux baz"]);
    }
}
