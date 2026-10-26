//! 搜尋／取代列與跳行列：文字模式以行為單位比對（Regex），十六進位模式比對位元組

use eframe::egui::{self, Key};
use regex::Regex;

use super::{App, Confirm};
use crate::core::document::EditKind;
use crate::core::editor::{EditorState, TextPosition};
use crate::core::encoding;
use crate::core::model::EditMode;
use crate::core::search::{self, SearchOptions};

#[derive(Default)]
pub struct FindBar {
    pub visible: bool,
    pub replace: bool,
    pub query: String,
    pub replacement: String,
    /// 以十六進位位元組搜尋
    pub hex: bool,
    pub focus: bool,
    pub info: Option<String>,
}

#[derive(Default)]
pub struct GotoBar {
    pub visible: bool,
    /// 依位置（位元組位移）跳轉，而非行號
    pub by_position: bool,
    pub input: String,
    pub focus: bool,
    pub error: Option<String>,
}

/// 在行陣列中尋找下一個（或上一個）符合的位置
///
/// `range` 限制只在指定範圍內搜尋（「在選取範圍內搜尋」）。
pub fn find_in_lines(
    ed: &EditorState,
    re: &Regex,
    from: TextPosition,
    backward: bool,
    wrap: bool,
    range: Option<(TextPosition, TextPosition)>,
) -> Option<(TextPosition, TextPosition)> {
    let (lo, hi) = range.unwrap_or((TextPosition::new(0, 0), TextPosition::new(ed.lines.len() - 1, ed.line_char_count(ed.lines.len() - 1))));
    let in_range = |s: TextPosition, e: TextPosition| s >= lo && e <= hi;
    let line_matches = |li: usize| -> Vec<(TextPosition, TextPosition)> {
        let line = &ed.lines[li];
        re.find_iter(line)
            .filter(|m| m.start() != m.end())
            .map(|m| {
                let s = line[..m.start()].chars().count();
                let e = s + line[m.start()..m.end()].chars().count();
                (TextPosition::new(li, s), TextPosition::new(li, e))
            })
            .filter(|(s, e)| in_range(*s, *e))
            .collect()
    };
    let n = ed.lines.len();
    if !backward {
        for li in from.line..n {
            if let Some(m) = line_matches(li).into_iter().find(|(s, _)| *s >= from) {
                return Some(m);
            }
        }
        if wrap {
            for li in 0..=from.line.min(n - 1) {
                if let Some(m) = line_matches(li).into_iter().next() {
                    return Some(m);
                }
            }
        }
    } else {
        for li in (0..=from.line.min(n - 1)).rev() {
            if let Some(m) = line_matches(li).into_iter().rev().find(|(_, e)| *e <= from) {
                return Some(m);
            }
        }
        if wrap {
            for li in (from.line..n).rev() {
                if let Some(m) = line_matches(li).into_iter().last() {
                    return Some(m);
                }
            }
        }
    }
    None
}

/// 全部取代（以行為單位），並維護書籤（取代文字含換行時，書籤隨行號位移）
pub fn replace_all_lines(ed: &mut EditorState, re: &Regex, rep: &str, use_regex: bool, lines: std::ops::Range<usize>) -> usize {
    let mut count = 0;
    let mut new_lines: Vec<String> = Vec::with_capacity(ed.lines.len());
    let mut line_map: Vec<usize> = Vec::with_capacity(ed.lines.len());
    for (li, line) in ed.lines.iter().enumerate() {
        line_map.push(new_lines.len());
        if !lines.contains(&li) {
            new_lines.push(line.clone());
            continue;
        }
        let mut n = 0;
        let replaced = re.replace_all(line, |caps: &regex::Captures| {
            n += 1;
            if use_regex {
                let mut dst = String::new();
                caps.expand(rep, &mut dst);
                dst
            } else {
                rep.to_string()
            }
        });
        count += n;
        if n > 0 && replaced.contains(['\n', '\r']) {
            let norm = replaced.replace("\r\n", "\n").replace('\r', "\n");
            new_lines.extend(norm.split('\n').map(str::to_string));
        } else {
            new_lines.push(replaced.into_owned());
        }
    }
    if count > 0 {
        // P1-11：書籤依新的行號重新對應
        ed.bookmarks = ed.bookmarks.iter().filter_map(|&b| line_map.get(b).copied()).collect();
        ed.lines = new_lines;
        ed.clamp_caret();
    }
    count
}

impl App {
    pub fn search_options(&self) -> SearchOptions {
        SearchOptions::new(self.settings.search_match_case, self.settings.search_whole_word, self.settings.search_regex)
    }

    /// 建立文字搜尋用的 Regex（十六進位查詢先依文件編碼解碼成文字）
    fn text_regex(&self) -> Result<Regex, String> {
        let q = &self.find.query;
        if self.find.hex {
            let bytes = search::parse_hex_query(q)?;
            let d = encoding::decode_with(&bytes, self.doc().encoding);
            let opts = SearchOptions::new(true, false, false);
            return search::build_regex(&d.text, &opts);
        }
        search::build_regex(q, &self.search_options())
    }

    /// 找下一個或上一個；回傳是否找到
    pub fn find_next(&mut self, backward: bool) -> bool {
        if self.find.query.is_empty() {
            self.find.visible = true;
            self.find.focus = true;
            return false;
        }
        crate::core::settings::Settings::push_history(&mut self.settings.recent_finds, self.find.query.clone());
        if self.macros.recording {
            let step = super::macros::MacroStep::Find {
                query: self.find.query.clone(),
                hex: self.find.hex,
                match_case: self.settings.search_match_case,
                whole_word: self.settings.search_whole_word,
                regex: self.settings.search_regex,
                backward,
            };
            self.macros.record(step);
        }
        let wrap = self.settings.search_wrap_around;
        if self.doc().mode == EditMode::Hex {
            return self.find_bytes(backward, wrap);
        }
        let re = match self.text_regex() {
            Ok(r) => r,
            Err(e) => {
                self.find.info = Some(e);
                return false;
            }
        };
        let doc = self.doc_mut();
        let sel = doc.editor.selection;
        let range = None;
        let from = if backward { sel.start() } else { sel.end() };
        let found = find_in_lines(&doc.editor, &re, from, backward, wrap, range);
        match found {
            Some((s, e)) => {
                let from_pos = sel.head;
                doc.record_jump(from_pos);
                if backward {
                    doc.editor.select_range(e, s);
                } else {
                    doc.editor.select_range(s, e);
                }
                self.tab_mut().view.ensure_visible = true;
                let wrapped = if backward { s > from } else { s < from };
                self.find.info = Some(if wrapped { self.t("find.wrapped", "已從另一端繼續搜尋").to_string() } else { String::new() });
                true
            }
            None => {
                self.find.info = Some(self.t("find.not_found", "找不到符合的文字").to_string());
                let q = self.find.query.clone();
                self.set_status(self.tf("find.not_found_q", "找不到「{q}」", None, Some(&q)));
                false
            }
        }
    }

    /// 十六進位模式：在位元組中尋找
    fn find_bytes(&mut self, backward: bool, wrap: bool) -> bool {
        let pattern = if self.find.hex {
            match search::parse_hex_query(&self.find.query) {
                Ok(p) => p,
                Err(e) => {
                    self.find.info = Some(e);
                    return false;
                }
            }
        } else {
            match encoding::encode_text(&self.find.query, self.doc().encoding, false) {
                Ok(b) => b,
                Err(_) => {
                    self.find.info = Some(self.t("find.bad_encoding", "目前編碼無法表示搜尋文字").to_string());
                    return false;
                }
            }
        };
        let Some(hb) = self.doc_mut().hex.as_mut() else { return false };
        let len = hb.len();
        let caret = hb.caret;
        let found = if backward {
            let before = hb.selection().map(|(s, _)| s).unwrap_or(caret);
            hb.data.rfind(&pattern, before).or_else(|| if wrap { hb.data.rfind(&pattern, len) } else { None })
        } else {
            let from = hb.selection().map(|(_, e)| e).unwrap_or(caret);
            hb.data.find(&pattern, from, len).or_else(|| if wrap { hb.data.find(&pattern, 0, len) } else { None })
        };
        match found {
            Some(off) => {
                hb.move_to(off, false);
                hb.move_to(off + pattern.len() - 1, true);
                let rows = 8;
                hb.top_row = (off / crate::core::hexbuf::BYTES_PER_ROW).saturating_sub(rows);
                self.find.info = Some(self.tf("find.offset", "位移 {q}", None, Some(&format!("{off:X}h"))));
                true
            }
            None => {
                self.find.info = Some(self.t("find.no_bytes", "找不到符合的資料").to_string());
                false
            }
        }
    }

    /// 計算符合數量
    fn count_matches(&mut self) {
        match self.text_regex() {
            Ok(re) => {
                let n: usize = self.doc().editor.lines.iter().map(|l| re.find_iter(l).filter(|m| m.start() != m.end()).count()).sum();
                self.find.info = Some(self.tf("find.count_n", "共 {n} 個符合", Some(n), None));
            }
            Err(e) => self.find.info = Some(e),
        }
    }

    /// 取代目前選取的符合文字，並找下一個
    pub fn replace_once(&mut self) {
        if self.doc().read_only || self.doc().mode == EditMode::Hex {
            return;
        }
        let re = match self.text_regex() {
            Ok(r) => r,
            Err(e) => {
                self.find.info = Some(e);
                return;
            }
        };
        let use_regex = self.settings.search_regex && !self.find.hex;
        let rep = self.find.replacement.clone();
        crate::core::settings::Settings::push_history(&mut self.settings.recent_replaces, rep.clone());
        let doc = self.doc_mut();
        let selected = doc.editor.selected_text();
        let whole = re.find(&selected).is_some_and(|m| m.start() == 0 && m.end() == selected.len());
        if whole && !selected.is_empty() {
            let new_text = if use_regex { re.replace(&selected, rep.as_str()).into_owned() } else { rep };
            let hint = doc.selection_hint();
            doc.edit(EditKind::Other, hint, |ed| ed.insert_text(&new_text));
        }
        self.find_next(false);
    }

    /// 全部取代（在選取範圍內或整份文件）
    pub fn replace_all(&mut self) {
        if self.doc().read_only || self.doc().mode == EditMode::Hex {
            return;
        }
        let re = match self.text_regex() {
            Ok(r) => r,
            Err(e) => {
                self.find.info = Some(e);
                return;
            }
        };
        let use_regex = self.settings.search_regex && !self.find.hex;
        let rep = self.find.replacement.clone();
        crate::core::settings::Settings::push_history(&mut self.settings.recent_replaces, rep.clone());
        if self.macros.recording {
            self.macros.record(super::macros::MacroStep::ReplaceAll {
                query: self.find.query.clone(),
                replacement: rep.clone(),
                match_case: self.settings.search_match_case,
                whole_word: self.settings.search_whole_word,
                regex: self.settings.search_regex,
            });
        }
        let in_sel = self.settings.search_in_selection;
        let doc = self.doc_mut();
        let sel = doc.editor.selection;
        let count = if in_sel && !sel.is_empty() && !sel.is_block() {
            // 只取代選取範圍：對選取文字整段處理，以 insert_text 維護書籤
            let text = doc.editor.selected_text();
            let mut n = 0;
            let new_text = re
                .replace_all(&text, |caps: &regex::Captures| {
                    n += 1;
                    if use_regex {
                        let mut d = String::new();
                        caps.expand(&rep, &mut d);
                        d
                    } else {
                        rep.clone()
                    }
                })
                .into_owned();
            if n > 0 {
                let hint = doc.selection_hint();
                doc.edit(EditKind::Other, hint, |ed| ed.replace_selection(&new_text));
            }
            n
        } else {
            let hint = doc.whole_hint();
            let all = 0..doc.editor.lines.len();
            doc.edit(EditKind::Other, hint, |ed| replace_all_lines(ed, &re, &rep, use_regex, all))
        };
        let msg = self.tf("find.replaced", "已取代 {n} 處", Some(count), None);
        self.find.info = Some(msg.clone());
        self.set_status(msg);
    }

    pub fn find_bar(&mut self, ui: &mut egui::Ui) {
        if !self.find.visible {
            return;
        }
        let mut close = false;
        let mut do_next = None;
        let mut do_replace = false;
        let mut do_replace_all = false;
        let mut do_count = false;
        // 先取出譯文，避免與下方對欄位的可變借用重疊
        let lbl_find = self.t("find.find", "尋找：");
        let lbl_hex_hint = self.t("find.hex_hint", "十六進位，例如 4D 61 64").to_string();
        let lbl_prev = self.t("find.prev", "◀ 上一個");
        let lbl_next = self.t("find.next", "下一個 ▶");
        let lbl_count = self.t("find.count", "計數");
        let lbl_case = self.t("find.case", "大小寫");
        let lbl_word = self.t("find.word", "全字");
        let lbl_regex = self.t("find.regex", "Regex");
        let lbl_hex = self.t("find.hex", "Hex");
        let lbl_wrap = self.t("find.wrap", "循環");
        let lbl_toggle = self.t("find.replace_toggle", "取代…");
        let lbl_close = self.t("find.close", "關閉 (Esc)");
        let lbl_replace = self.t("find.replace", "取代：");
        let lbl_replace_btn = self.t("find.replace_btn", "取代");
        let lbl_replace_all = self.t("find.replace_all", "全部取代");
        let lbl_in_sel = self.t("find.in_selection", "只在選取範圍內");
        egui::Frame::new().inner_margin(egui::Margin::symmetric(6, 4)).fill(ui.visuals().faint_bg_color).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(lbl_find);
                let resp = ui.add(egui::TextEdit::singleline(&mut self.find.query).desired_width(260.0).hint_text(if self.find.hex { lbl_hex_hint.clone() } else { String::new() }));
                if std::mem::take(&mut self.find.focus) {
                    resp.request_focus();
                    // 全選以便直接輸入新的搜尋文字
                    if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), resp.id) {
                        let all = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(self.find.query.chars().count()));
                        state.cursor.set_char_range(Some(all));
                        state.store(ui.ctx(), resp.id);
                    }
                }
                if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    do_next = Some(ui.input(|i| i.modifiers.shift));
                    resp.request_focus();
                }
                egui::ComboBox::from_id_salt("find-history").selected_text("▾").width(24.0).show_ui(ui, |ui| {
                    for h in self.settings.recent_finds.clone() {
                        if ui.selectable_label(false, &h).clicked() {
                            self.find.query = h;
                        }
                    }
                });
                if ui.button(lbl_prev).clicked() {
                    do_next = Some(true);
                }
                if ui.button(lbl_next).clicked() {
                    do_next = Some(false);
                }
                if ui.button(lbl_count).clicked() {
                    do_count = true;
                }
                ui.checkbox(&mut self.settings.search_match_case, lbl_case);
                ui.checkbox(&mut self.settings.search_whole_word, lbl_word);
                ui.checkbox(&mut self.settings.search_regex, lbl_regex);
                ui.checkbox(&mut self.find.hex, lbl_hex);
                ui.checkbox(&mut self.settings.search_wrap_around, lbl_wrap);
                if ui.selectable_label(self.find.replace, lbl_toggle).clicked() {
                    self.find.replace = !self.find.replace;
                }
                if let Some(info) = &self.find.info {
                    ui.label(info);
                }
                if ui.button("✕").on_hover_text(lbl_close).clicked() {
                    close = true;
                }
            });
            if self.find.replace {
                ui.horizontal(|ui| {
                    ui.label(lbl_replace);
                    let resp = ui.add(egui::TextEdit::singleline(&mut self.find.replacement).desired_width(260.0));
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        do_replace = true;
                        resp.request_focus();
                    }
                    egui::ComboBox::from_id_salt("replace-history").selected_text("▾").width(24.0).show_ui(ui, |ui| {
                        for h in self.settings.recent_replaces.clone() {
                            if ui.selectable_label(false, &h).clicked() {
                                self.find.replacement = h;
                            }
                        }
                    });
                    if ui.button(lbl_replace_btn).clicked() {
                        do_replace = true;
                    }
                    if ui.button(lbl_replace_all).clicked() {
                        do_replace_all = true;
                    }
                    ui.checkbox(&mut self.settings.search_in_selection, lbl_in_sel);
                });
            }
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                close = true;
            }
        });
        if let Some(back) = do_next {
            self.find_next(back);
        }
        if do_count {
            self.count_matches();
        }
        if do_replace {
            self.replace_once();
        }
        if do_replace_all {
            self.replace_all();
        }
        if close {
            self.find.visible = false;
            self.focus_editor = true;
        }
    }

    pub fn goto_bar(&mut self, ui: &mut egui::Ui) {
        if !self.goto.visible {
            return;
        }
        let mut go = false;
        let mut close = false;
        let hex_mode = self.doc().mode == EditMode::Hex;
        let lbl_goto_pos = self.t("find.goto_pos", "移至位置（位元組，可用 0x 十六進位）：");
        let lbl_goto_line = self.t("find.goto_line", "移至行（行號 或 行:欄）：");
        let lbl_go = self.t("find.go", "跳轉");
        let lbl_by_bytes = self.t("find.by_bytes", "依位元組位置");
        egui::Frame::new().inner_margin(egui::Margin::symmetric(6, 4)).fill(ui.visuals().faint_bg_color).show(ui, |ui| {
            ui.horizontal(|ui| {
                let by_pos = self.goto.by_position || hex_mode;
                ui.label(if by_pos { lbl_goto_pos } else { lbl_goto_line });
                let resp = ui.add(egui::TextEdit::singleline(&mut self.goto.input).desired_width(160.0));
                if std::mem::take(&mut self.goto.focus) {
                    resp.request_focus();
                }
                if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    go = true;
                }
                if ui.button(lbl_go).clicked() {
                    go = true;
                }
                if !hex_mode {
                    ui.checkbox(&mut self.goto.by_position, lbl_by_bytes);
                }
                if let Some(e) = &self.goto.error {
                    ui.colored_label(egui::Color32::RED, e);
                }
                if ui.button("✕").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    close = true;
                }
            });
        });
        if go {
            match self.goto_target() {
                Ok(()) => {
                    self.goto.visible = false;
                    self.goto.error = None;
                    self.focus_editor = true;
                }
                Err(e) => self.goto.error = Some(e),
            }
        }
        if close {
            self.goto.visible = false;
            self.focus_editor = true;
        }
    }

    fn goto_target(&mut self) -> Result<(), String> {
        let input = self.goto.input.trim().to_string();
        // 不借用 self，錯誤字串稍後再翻譯
        let parse_num = |s: &str| -> Result<usize, String> {
            let s = s.trim();
            let r = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                usize::from_str_radix(h, 16)
            } else if let Some(h) = s.strip_suffix(['h', 'H']) {
                usize::from_str_radix(h, 16)
            } else {
                s.parse()
            };
            r.map_err(|_| s.to_string())
        };
        let hex_mode = self.doc().mode == EditMode::Hex;
        if self.goto.by_position || hex_mode {
            let off = parse_num(&input).map_err(|tok| self.tf("find.bad_number", "無效的數字：{q}", None, Some(&tok)))?;
            let doc = self.doc_mut();
            if hex_mode {
                let hb = doc.hex.as_mut().unwrap();
                let off = off.min(hb.len());
                hb.move_to(off, false);
                hb.top_row = (off / crate::core::hexbuf::BYTES_PER_ROW).saturating_sub(5);
            } else {
                // 依編碼換算位元組位移對應的文字位置
                let target = off;
                let mut pos = TextPosition::new(0, 0);
                let nl = encoding::encoded_len(doc.newline.newline_str(), doc.encoding);
                let mut acc = if doc.has_bom { encoding::bom_bytes(doc.encoding).len() } else { 0 };
                'outer: for (li, line) in doc.editor.lines.iter().enumerate() {
                    let line_len = encoding::encoded_len(line, doc.encoding);
                    if acc + line_len >= target {
                        let mut col = 0;
                        let mut b = acc;
                        for (ci, c) in line.chars().enumerate() {
                            let cl = encoding::encoded_len(c.encode_utf8(&mut [0; 4]), doc.encoding);
                            if b + cl > target {
                                col = ci;
                                pos = TextPosition::new(li, col);
                                break 'outer;
                            }
                            b += cl;
                            col = ci + 1;
                        }
                        pos = TextPosition::new(li, col);
                        break;
                    }
                    acc += line_len + nl;
                    pos = TextPosition::new(li, line.chars().count());
                }
                let from = doc.editor.selection.head;
                doc.record_jump(from);
                doc.editor.set_caret(pos.line, pos.col);
            }
        } else {
            let mut parts = input.splitn(2, [':', ',']);
            let line = parse_num(parts.next().unwrap_or("")).map_err(|tok| self.tf("find.bad_number", "無效的數字：{q}", None, Some(&tok)))?.max(1);
            let col = match parts.next() {
                Some(s) => parse_num(s).map_err(|tok| self.tf("find.bad_number", "無效的數字：{q}", None, Some(&tok)))?.max(1),
                None => 1,
            };
            let doc = self.doc_mut();
            let from = doc.editor.selection.head;
            doc.record_jump(from);
            doc.editor.set_caret(line - 1, col - 1);
        }
        self.tab_mut().view.ensure_visible = true;
        Ok(())
    }

    /// 顯示訊息對話框（供其他模組使用）
    pub fn message(&mut self, title: &str, text: impl Into<String>) {
        self.confirm = Some(Confirm::Message { title: title.into(), text: text.into() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_in_lines_wraps_and_respects_direction() {
        let ed = EditorState::from_text("foo bar\nbar foo\nfoo");
        let re = Regex::new("foo").unwrap();
        let m = find_in_lines(&ed, &re, TextPosition::new(0, 1), false, true, None).unwrap();
        assert_eq!(m.0, TextPosition::new(1, 4));
        let m = find_in_lines(&ed, &re, TextPosition::new(2, 3), false, true, None).unwrap();
        assert_eq!(m.0, TextPosition::new(0, 0));
        assert!(find_in_lines(&ed, &re, TextPosition::new(2, 3), false, false, None).is_none());
        let m = find_in_lines(&ed, &re, TextPosition::new(1, 4), true, true, None).unwrap();
        assert_eq!(m.0, TextPosition::new(0, 0));
    }

    #[test]
    fn test_replace_all_keeps_bookmarks() {
        let mut ed = EditorState::from_text("a\nb\na\nc");
        ed.bookmarks.insert(2);
        ed.bookmarks.insert(3);
        let re = Regex::new("a").unwrap();
        // 取代文字含換行：第 0 行變兩行，之後的書籤往後移一行
        let n = replace_all_lines(&mut ed, &re, "x\ny", false, 0..1);
        assert_eq!(n, 1);
        assert_eq!(ed.lines, vec!["x", "y", "b", "a", "c"]);
        assert_eq!(ed.bookmarks.iter().copied().collect::<Vec<_>>(), vec![3, 4]);
    }
}
