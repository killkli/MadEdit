//! 在多個檔案中尋找／取代：背景執行緒掃描資料夾，結果顯示於下方面板

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

use eframe::egui;
use regex::Regex;

use super::App;
use crate::core::document::EditKind;
use crate::core::editor::TextPosition;
use crate::core::encoding;

/// 一筆搜尋結果
#[derive(Clone, Debug)]
pub struct Hit {
    pub path: PathBuf,
    pub line: usize,
    pub col: usize,
    pub len: usize,
    pub text: String,
}

enum Msg {
    Hit(Hit),
    Replaced { path: PathBuf, count: usize },
    Error(String),
    Done { files: usize },
}

#[derive(Default)]
pub struct FindInFiles {
    pub show_results: bool,
    pub results: Vec<Hit>,
    pub summary: String,
    pub collapsed: bool,
    rx: Option<Receiver<Msg>>,
    pub running: bool,
}

/// 簡單的萬用字元比對（* 與 ?，不分大小寫），多個樣式以 ; 或空白分隔
pub fn wildcard_match(patterns: &str, name: &str) -> bool {
    let pats: Vec<&str> = patterns.split([';', ' ', ',']).filter(|p| !p.is_empty()).collect();
    if pats.is_empty() {
        return true;
    }
    let name = name.to_lowercase();
    pats.iter().any(|p| {
        let re = format!("^{}$", regex::escape(&p.to_lowercase()).replace(r"\*", ".*").replace(r"\?", "."));
        Regex::new(&re).is_ok_and(|r| r.is_match(&name))
    })
}

fn collect_files(dir: &Path, include: &str, exclude: &str, recursive: bool, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            if recursive && !(!exclude.is_empty() && wildcard_match(exclude, &name)) {
                collect_files(&p, include, exclude, recursive, out);
            }
        } else if wildcard_match(include, &name) && !(!exclude.is_empty() && wildcard_match(exclude, &name)) {
            out.push(p);
        }
    }
}

impl App {
    /// 開始搜尋（`replace` 為 Some 時同時取代）
    pub fn start_find_in_files(&mut self, query: String, replace: Option<String>) {
        let re = match crate::core::search::build_regex(&query, &self.search_options()) {
            Ok(r) => r,
            Err(e) => {
                self.message("搜尋條件錯誤", e);
                return;
            }
        };
        let use_regex = self.settings.search_regex;
        let opts = self.settings.find_in_files.clone();
        self.fif.results.clear();
        self.fif.show_results = true;
        self.fif.collapsed = false;
        let max_chars = self.settings.max_search_result_chars.max(20);

        // 已開啟的文件直接在記憶體中處理（取代可復原）
        let mut opened: Vec<PathBuf> = Vec::new();
        let mut replaced_total = 0;
        for t in &mut self.tabs {
            // 未命名文件沒有路徑，結果無法定位，略過
            let Some(p) = t.doc.path.clone() else { continue };
            let in_scope = opts.opened_files_only
                || (p.starts_with(&opts.directory) && wildcard_match(&opts.file_filter, &p.file_name().unwrap_or_default().to_string_lossy()));
            if !in_scope {
                continue;
            }
            opened.push(p.clone());
            if let Some(rep) = &replace {
                if !t.doc.read_only {
                    let hint = t.doc.whole_hint();
                    let all = 0..t.doc.editor.lines.len();
                    let n = t.doc.edit(EditKind::Other, hint, |ed| super::find::replace_all_lines(ed, &re, rep, use_regex, all));
                    replaced_total += n;
                }
            } else {
                for (li, line) in t.doc.editor.lines.iter().enumerate() {
                    for m in re.find_iter(line).filter(|m| m.start() != m.end()) {
                        self.fif.results.push(Hit {
                            path: p.clone(),
                            line: li,
                            col: line[..m.start()].chars().count(),
                            len: line[m.start()..m.end()].chars().count(),
                            text: line.chars().take(max_chars).collect(),
                        });
                    }
                }
            }
        }
        if opts.opened_files_only {
            self.fif.summary = match &replace {
                Some(_) => format!("已在開啟的檔案中取代 {replaced_total} 處"),
                None => format!("在開啟的檔案中找到 {} 筆", self.fif.results.len()),
            };
            return;
        }

        let dir = PathBuf::from(&opts.directory);
        if !dir.is_dir() {
            self.message("在檔案中尋找", format!("資料夾不存在：{}", opts.directory));
            return;
        }
        crate::core::settings::Settings::push_history(&mut self.settings.recent_dirs, opts.directory.clone());
        let default_enc = encoding::encoding_for_name(&self.settings.default_encoding).unwrap_or(encoding_rs::BIG5);
        let (tx, rx) = channel();
        self.fif.rx = Some(rx);
        self.fif.running = true;
        self.fif.summary = if replace.is_some() { format!("取代中…（開啟中的檔案已取代 {replaced_total} 處）") } else { "搜尋中…".into() };
        std::thread::spawn(move || {
            let mut files = Vec::new();
            collect_files(&dir, &opts.file_filter, &opts.exclude_filter, opts.subdirectories, &mut files);
            let total = files.len();
            for f in files {
                if opened.contains(&f) {
                    continue;
                }
                let Ok(bytes) = std::fs::read(&f) else { continue };
                if bytes.len() > 64 * 1024 * 1024 || encoding::looks_binary(&bytes) {
                    continue;
                }
                let d = encoding::detect_and_decode(&bytes, default_enc, false);
                match &replace {
                    None => {
                        for (li, line) in d.text.lines().enumerate() {
                            for m in re.find_iter(line).filter(|m| m.start() != m.end()) {
                                let _ = tx.send(Msg::Hit(Hit {
                                    path: f.clone(),
                                    line: li,
                                    col: line[..m.start()].chars().count(),
                                    len: line[m.start()..m.end()].chars().count(),
                                    text: line.chars().take(max_chars).collect(),
                                }));
                            }
                        }
                    }
                    Some(rep) => {
                        if d.had_errors {
                            let _ = tx.send(Msg::Error(format!("略過（無法正確解碼）：{}", f.display())));
                            continue;
                        }
                        let mut count = 0;
                        let new_text = re.replace_all(&d.text, |caps: &regex::Captures| {
                            count += 1;
                            if use_regex {
                                let mut s = String::new();
                                caps.expand(rep, &mut s);
                                s
                            } else {
                                rep.clone()
                            }
                        });
                        if count == 0 {
                            continue;
                        }
                        match encoding::encode_text(&new_text, d.encoding, d.has_bom) {
                            Ok(out) => {
                                if let Err(e) = std::fs::write(&f, out) {
                                    let _ = tx.send(Msg::Error(format!("{}：{e}", f.display())));
                                } else {
                                    let _ = tx.send(Msg::Replaced { path: f.clone(), count });
                                }
                            }
                            Err(_) => {
                                let _ = tx.send(Msg::Error(format!("略過（取代後的文字無法以原編碼儲存）：{}", f.display())));
                            }
                        }
                    }
                }
            }
            let _ = tx.send(Msg::Done { files: total });
        });
    }

    pub fn find_results_panel(&mut self, ui: &mut egui::Ui) {
        let mut open: Option<Hit> = None;
        ui.horizontal(|ui| {
            ui.strong("搜尋結果");
            ui.label(&self.fif.summary);
            if ui.button("清除").clicked() {
                self.fif.results.clear();
            }
            if ui.button("✕").clicked() {
                self.fif.show_results = false;
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let mut current: Option<&Path> = None;
            for h in &self.fif.results {
                if current != Some(h.path.as_path()) {
                    current = Some(h.path.as_path());
                    ui.strong(h.path.display().to_string());
                }
                if self.fif.collapsed {
                    continue;
                }
                let label = format!("    {:>6}: {}", h.line + 1, h.text.trim_end());
                if ui.add(egui::Label::new(egui::RichText::new(label).monospace()).sense(egui::Sense::click())).clicked() {
                    open = Some(h.clone());
                }
            }
        });
        if let Some(h) = open
            && self.open_path(&h.path) {
                let doc = self.doc_mut();
                let from = doc.editor.selection.head;
                doc.record_jump(from);
                doc.editor.select_range(TextPosition::new(h.line, h.col), TextPosition::new(h.line, h.col + h.len));
                self.tab_mut().view.ensure_visible = true;
                self.focus_editor = true;
            }
    }
}

impl FindInFiles {
    /// 接收背景執行緒的結果
    pub fn poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.rx else { return };
        let mut replaced = 0;
        let mut errors = Vec::new();
        loop {
            match rx.try_recv() {
                Ok(Msg::Hit(h)) => self.results.push(h),
                Ok(Msg::Replaced { path, count }) => {
                    replaced += count;
                    self.results.push(Hit { path, line: 0, col: 0, len: 0, text: format!("已取代 {count} 處") });
                }
                Ok(Msg::Error(e)) => errors.push(e),
                Ok(Msg::Done { files }) => {
                    self.running = false;
                    self.summary = format!("共掃描 {files} 個檔案，{} 筆結果", self.results.len());
                    self.rx = None;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                    break;
                }
                Err(_) => {
                    self.running = false;
                    self.rx = None;
                    break;
                }
            }
        }
        if replaced > 0 {
            self.summary = format!("{}（已取代 {replaced} 處）", self.summary);
        }
        for e in errors {
            self.results.push(Hit { path: PathBuf::from("（錯誤）"), line: 0, col: 0, len: 0, text: e });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wildcard_match() {
        assert!(wildcard_match("*.rs;*.toml", "main.rs"));
        assert!(wildcard_match("*.RS", "main.rs"));
        assert!(!wildcard_match("*.rs", "main.rsx"));
        assert!(wildcard_match("", "x"));
        assert!(wildcard_match("a?c", "abc"));
    }
}
