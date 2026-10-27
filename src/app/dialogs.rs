//! 對話框：選項、字型、排序、遞增數字、全形半形、在檔案中尋找、編碼轉換、字數統計等，以及確認視窗

use eframe::egui::{self, Key, RichText};

use super::commands::{self, C};
use super::i18n::{self, Lang};
use super::{App, Confirm};
use crate::core::document::EditKind;
use crate::core::encoding::{self, ALL_ENCODINGS};
use crate::core::model::EditMode;
use crate::core::settings::{OpenViewMode, PrintOptions, Settings, ThemeChoice, UiLanguage, WidthOptions};
use crate::core::spell;
use crate::core::syntax;
use crate::core::textops::{self, WordCount};

/// 對話框視窗：第一次出現時置於主視窗中央（之後可自由拖曳）
fn dialog_window(ctx: &egui::Context, title: impl Into<egui::WidgetText>) -> egui::Window<'static> {
    egui::Window::new(title).pivot(egui::Align2::CENTER_CENTER).default_pos(ctx.content_rect().center())
}

/// 選項對話框的狀態
#[derive(Default)]
pub struct OptionsState {
    tab: usize,
    /// 編輯中的設定副本（按「確定」或「套用」才寫回）
    draft: Option<Settings>,
    /// 正在擷取快捷鍵的命令
    pub capturing: Option<C>,
    selected: Option<C>,
    filter: String,
}

pub enum Dialog {
    Options(Box<OptionsState>),
    Font { filter: String, family: String, size: f32 },
    PageSetup,
    RecentFiles { filter: String },
    Width { full: bool },
    InsertNumbers,
    Sort,
    FindInFiles { query: String, replacement: String },
    TempMacro { script: String, error: Option<String> },
    Highlighting,
    SourceFormatter,
    PurgeHistories,
    ConvertEncoding { encoding: String, bom: bool },
    WordCount(WordCount, bool),
    WindowList,
    About,
}

/// 列印選項（頁面設定與選項對話框共用）
fn print_options_ui(ui: &mut egui::Ui, p: &mut PrintOptions, lang: Lang) {
    let t = |k: &str, zh: &'static str| i18n::translate(lang, k, zh);
    ui.checkbox(&mut p.syntax_colors, t("print.syntax", "列印語法色彩"));
    ui.checkbox(&mut p.line_numbers, t("print.lines", "列印行號"));
    ui.checkbox(&mut p.bookmarks, t("print.bookmarks", "列印書籤"));
    ui.checkbox(&mut p.end_of_line, t("print.eol", "列印行尾字元"));
    ui.checkbox(&mut p.tab_chars, t("print.tabs", "列印 Tab 字元"));
    ui.checkbox(&mut p.space_chars, t("print.spaces", "列印空白字元"));
    ui.separator();
    ui.checkbox(&mut p.header, t("print.header", "頁首"));
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut p.header_left).desired_width(120.0));
        ui.add(egui::TextEdit::singleline(&mut p.header_center).desired_width(120.0));
        ui.add(egui::TextEdit::singleline(&mut p.header_right).desired_width(120.0));
    });
    ui.checkbox(&mut p.footer, t("print.footer", "頁尾"));
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut p.footer_left).desired_width(120.0));
        ui.add(egui::TextEdit::singleline(&mut p.footer_center).desired_width(120.0));
        ui.add(egui::TextEdit::singleline(&mut p.footer_right).desired_width(120.0));
    });
    ui.label(RichText::new(t("print.vars", "變數：%f 檔名　%p 路徑　%n 頁碼　%s 總頁數　%d 日期　%t 時間")).small());
    ui.horizontal(|ui| {
        ui.label(t("print.hex_header", "十六進位模式位移標頭："));
        ui.radio_value(&mut p.hex_offset_header, 0, t("print.none", "無"));
        ui.radio_value(&mut p.hex_offset_header, 1, t("print.first", "第一頁"));
        ui.radio_value(&mut p.hex_offset_header, 2, t("print.every", "每頁"));
    });
}

fn encoding_combo(ui: &mut egui::Ui, id: &str, value: &mut String) {
    egui::ComboBox::from_id_salt(id).selected_text(value.as_str()).width(200.0).show_ui(ui, |ui| {
        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            for &e in ALL_ENCODINGS {
                ui.selectable_value(value, e.to_string(), e);
            }
        });
    });
}

fn width_options_ui(ui: &mut egui::Ui, w: &mut WidthOptions, lang: Lang) {
    let t = |k: &str, zh: &'static str| i18n::translate(lang, k, zh);
    ui.checkbox(&mut w.letters, t("width.letters", "英文字母"));
    ui.checkbox(&mut w.digits, t("width.digits", "數字"));
    ui.checkbox(&mut w.symbols, t("width.symbols", "符號"));
    ui.checkbox(&mut w.space, t("width.space", "空白"));
    ui.checkbox(&mut w.katakana, t("width.katakana", "日文片假名"));
}

impl App {
    pub fn show_dialogs(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.dialog.take() else { return };
        let mut open = true;
        let mut close = false;
        match &mut dlg {
            Dialog::Options(st) => self.options_dialog(ctx, st, &mut open),
            Dialog::Font { filter, family, size } => {
                dialog_window(ctx, self.t("dlg.font", "字型")).open(&mut open).default_size([420.0, 460.0]).show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(self.t("font.filter", "篩選："));
                        ui.text_edit_singleline(filter);
                    });
                    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                        if ui.selectable_label(family.is_empty(), self.t("font.auto", "（自動選擇等寬字型）")).clicked() {
                            family.clear();
                        }
                        let f = filter.to_lowercase();
                        for name in &self.fonts.families {
                            if (f.is_empty() || name.to_lowercase().contains(&f)) && ui.selectable_label(name == family, name).clicked() {
                                *family = name.clone();
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(self.t("font.size", "大小："));
                        ui.add(egui::DragValue::new(size).range(6.0..=72.0).speed(0.5));
                    });
                    ui.label(self.t("font.cjk", "中文、日文等字元若字型中沒有，會自動以系統的 CJK 字型補字。比例字型請搭配關閉「等寬模式」。"));
                    ui.horizontal(|ui| {
                        if ui.button(self.t("btn.ok", "確定")).clicked() {
                            self.settings.font_family = family.clone();
                            self.settings.font_size = *size;
                            close = true;
                        }
                        if ui.button(self.t("btn.cancel", "取消")).clicked() {
                            close = true;
                        }
                    });
                });
            }
            Dialog::PageSetup => {
                dialog_window(ctx, self.t("dlg.page", "頁面設定")).open(&mut open).show(ctx, |ui| {
                    let lang = self.ui_lang();
                    print_options_ui(ui, &mut self.settings.print, lang);
                    ui.label(self.t("print.browser", "列印時會以瀏覽器開啟排版後的頁面，紙張大小與邊界請在瀏覽器的列印對話框中設定。"));
                    if ui.button(self.t("btn.close", "關閉")).clicked() {
                        close = true;
                    }
                });
            }
            Dialog::RecentFiles { filter } => {
                let mut chosen = None;
                dialog_window(ctx, self.t("dlg.recent", "最近使用的檔案")).open(&mut open).default_size([520.0, 360.0]).show(ctx, |ui| {
                    let resp = ui.add(egui::TextEdit::singleline(filter).hint_text(self.t("opt.filter", "篩選：")));
                    resp.request_focus();
                    let f = filter.to_lowercase();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for p in &self.settings.recent_files {
                            let s = p.display().to_string();
                            if (f.is_empty() || s.to_lowercase().contains(&f)) && ui.selectable_label(false, &s).clicked() {
                                chosen = Some(p.clone());
                            }
                        }
                    });
                    if ui.input(|i| i.key_pressed(Key::Enter)) {
                        chosen = self.settings.recent_files.iter().find(|p| f.is_empty() || p.display().to_string().to_lowercase().contains(&f)).cloned();
                    }
                });
                if let Some(p) = chosen {
                    self.open_path(&p);
                    close = true;
                }
            }
            Dialog::Width { full } => {
                let full = *full;
                let title = if full { self.t("dlg.fullwidth", "依選項轉為全形") } else { self.t("dlg.halfwidth", "依選項轉為半形") };
                let lang = self.ui_lang();
                dialog_window(ctx, title).open(&mut open).show(ctx, |ui| {
                    width_options_ui(ui, &mut self.settings.width, lang);
                    ui.horizontal(|ui| {
                        if ui.button(self.t("btn.ok", "確定")).clicked() {
                            let o = self.settings.width.clone();
                            let text = self.doc().editor.selected_text();
                            let all = text.is_empty();
                            let src = if all { self.doc().editor.lines.join("\n") } else { text };
                            let out = if full { textops::to_fullwidth(&src, &o) } else { textops::to_halfwidth(&src, &o) };
                            let doc = self.doc_mut();
                            if all {
                                doc.replace_all_text(&out);
                            } else {
                                let hint = doc.selection_hint();
                                doc.edit(EditKind::Other, hint, |ed| ed.replace_selection(&out));
                            }
                            close = true;
                        }
                        if ui.button(self.t("btn.cancel", "取消")).clicked() {
                            close = true;
                        }
                    });
                });
            }
            Dialog::InsertNumbers => {
                let title = self.t("dlg.numbers", "插入遞增數字");
                let l_initial = self.t("num.initial", "起始值：");
                let l_step = self.t("num.step", "遞增值：");
                let l_count = self.t("num.count", "數量（無列塊選取時）：");
                let l_format = self.t("num.format", "格式：");
                let l_dec = self.t("num.dec", "十進位");
                let l_hex = self.t("num.hex", "十六進位");
                let l_oct = self.t("num.oct", "八進位");
                let l_bin = self.t("num.bin", "二進位");
                let l_affix = self.t("num.affix", "前綴／後綴：");
                let l_upper = self.t("num.upper", "十六進位使用大寫");
                let l_right = self.t("num.right", "靠右對齊");
                let l_left = self.t("num.left", "靠左對齊");
                let l_pad = self.t("num.pad", "前面補零");
                let l_hint = self.t("num.hint", "列塊模式下會在矩形選取的每一行插入一個數字；否則自游標處起每行插入一個。");
                let l_ok = self.t("btn.ok", "確定");
                let l_cancel = self.t("btn.cancel", "取消");
                dialog_window(ctx, title).open(&mut open).show(ctx, |ui| {
                    let n = &mut self.settings.numbers;
                    egui::Grid::new("numbers").num_columns(2).show(ui, |ui| {
                        ui.label(l_initial);
                        ui.add(egui::DragValue::new(&mut n.initial));
                        ui.end_row();
                        ui.label(l_step);
                        ui.add(egui::DragValue::new(&mut n.step));
                        ui.end_row();
                        ui.label(l_count);
                        ui.add(egui::DragValue::new(&mut n.total).range(1..=1_000_000));
                        ui.end_row();
                        ui.label(l_format);
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut n.format, 0, l_dec);
                            ui.radio_value(&mut n.format, 1, l_hex);
                            ui.radio_value(&mut n.format, 2, l_oct);
                            ui.radio_value(&mut n.format, 3, l_bin);
                        });
                        ui.end_row();
                        ui.label(l_affix);
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut n.prefix).desired_width(60.0));
                            ui.add(egui::TextEdit::singleline(&mut n.postfix).desired_width(60.0));
                        });
                        ui.end_row();
                    });
                    ui.checkbox(&mut n.uppercase_hex, l_upper);
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut n.align_left, false, l_right);
                        ui.radio_value(&mut n.align_left, true, l_left);
                    });
                    ui.checkbox(&mut n.zero_pad, l_pad);
                    ui.label(l_hint);
                    ui.horizontal(|ui| {
                        if ui.button(l_ok).clicked() {
                            self.insert_numbers();
                            close = true;
                        }
                        if ui.button(l_cancel).clicked() {
                            close = true;
                        }
                    });
                });
            }
            Dialog::Sort => {
                let title = self.t("dlg.sort", "排序選項");
                let l_asc = self.t("sort.asc", "遞增");
                let l_desc = self.t("sort.desc", "遞減");
                let l_case = self.t("sort.case", "區分大小寫");
                let l_num = self.t("sort.numeric", "依數值排序");
                let l_dup = self.t("sort.unique", "移除重複行");
                let l_go = self.t("sort.go", "排序");
                let l_close = self.t("btn.close", "關閉");
                dialog_window(ctx, title).open(&mut open).show(ctx, |ui| {
                    let s = &mut self.settings.sort;
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut s.descending, false, l_asc);
                        ui.radio_value(&mut s.descending, true, l_desc);
                    });
                    ui.checkbox(&mut s.case_sensitive, l_case);
                    ui.checkbox(&mut s.numeric, l_num);
                    ui.checkbox(&mut s.remove_duplicates, l_dup);
                    ui.horizontal(|ui| {
                        if ui.button(l_go).clicked() {
                            self.pending.push(C::SortByOptions);
                            close = true;
                        }
                        if ui.button(l_close).clicked() {
                            close = true;
                        }
                    });
                });
            }
            Dialog::FindInFiles { query, replacement } => {
                let mut start: Option<Option<String>> = None;
                let title = self.t("dlg.find_files", "在檔案中尋找／取代");
                let l_find = self.t("fif.find", "尋找：");
                let l_rep = self.t("fif.replace", "取代為：");
                let l_dir = self.t("fif.folder", "資料夾：");
                let l_types = self.t("fif.types", "檔案類型：");
                let l_types_hint = self.t("fif.types_hint", "例如 *.rs;*.toml");
                let l_ex = self.t("fif.exclude", "排除：");
                let l_ex_hint = self.t("fif.exclude_hint", "例如 target;*.min.js");
                let l_sub = self.t("fif.subdirs", "包含子資料夾");
                let l_opened = self.t("fif.opened", "只在已開啟的檔案中");
                let l_case = self.t("find.case", "大小寫");
                let l_word = self.t("find.word", "全字");
                let l_regex = self.t("find.regex", "Regex");
                let l_warn = self.t("fif.warn", "取代未開啟的檔案會直接寫入磁碟且無法復原；已開啟的檔案在編輯器中取代，可復原。");
                let l_find_all = self.t("fif.find_all", "全部尋找");
                let l_rep_all = self.t("find.replace_all", "全部取代");
                let l_close = self.t("btn.close", "關閉");
                dialog_window(ctx, title).open(&mut open).default_width(520.0).show(ctx, |ui| {
                    {
                        let o = &mut self.settings.find_in_files;
                        egui::Grid::new("fif").num_columns(2).show(ui, |ui| {
                            ui.label(l_find);
                            ui.add(egui::TextEdit::singleline(query).desired_width(360.0));
                            ui.end_row();
                            ui.label(l_rep);
                            ui.add(egui::TextEdit::singleline(replacement).desired_width(360.0));
                            ui.end_row();
                            ui.label(l_dir);
                            ui.horizontal(|ui| {
                                ui.add(egui::TextEdit::singleline(&mut o.directory).desired_width(300.0));
                                if ui.button("…").clicked()
                                    && let Some(d) = rfd::FileDialog::new().pick_folder()
                                {
                                    o.directory = d.display().to_string();
                                }
                            });
                            ui.end_row();
                            ui.label(l_types);
                            ui.add(egui::TextEdit::singleline(&mut o.file_filter).desired_width(360.0).hint_text(l_types_hint));
                            ui.end_row();
                            ui.label(l_ex);
                            ui.add(egui::TextEdit::singleline(&mut o.exclude_filter).desired_width(360.0).hint_text(l_ex_hint));
                            ui.end_row();
                        });
                        ui.checkbox(&mut o.subdirectories, l_sub);
                        ui.checkbox(&mut o.opened_files_only, l_opened);
                    }
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.settings.search_match_case, l_case);
                        ui.checkbox(&mut self.settings.search_whole_word, l_word);
                        ui.checkbox(&mut self.settings.search_regex, l_regex);
                    });
                    ui.label(RichText::new(l_warn).small());
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!self.fif.running && !query.is_empty(), egui::Button::new(l_find_all)).clicked() {
                            start = Some(None);
                        }
                        if ui.add_enabled(!self.fif.running && !query.is_empty(), egui::Button::new(l_rep_all)).clicked() {
                            start = Some(Some(replacement.clone()));
                        }
                        if ui.button(l_close).clicked() {
                            close = true;
                        }
                    });
                });
                if let Some(rep) = start {
                    self.find.query = query.clone();
                    self.start_find_in_files(query.clone(), rep);
                }
            }
            Dialog::TempMacro { script, error } => {
                dialog_window(ctx, self.t("dlg.macro", "執行暫存巨集")).open(&mut open).default_size([520.0, 420.0]).show(ctx, |ui| {
                    ui.label(self.t("macro.hint", "巨集為 JSON 格式的步驟清單（可由「開始錄製」產生後在此修改）："));
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        ui.add(egui::TextEdit::multiline(script).code_editor().desired_width(f32::INFINITY));
                    });
                    if let Some(e) = error {
                        ui.colored_label(egui::Color32::RED, e.as_str());
                    }
                    ui.horizontal(|ui| {
                        if ui.button(self.t("macro.run", "執行")).clicked() {
                            match super::macros::from_json(script) {
                                Ok(steps) => {
                                    self.play_macro(steps);
                                    close = true;
                                }
                                Err(e) => *error = Some(e),
                            }
                        }
                        if ui.button(self.t("btn.close", "關閉")).clicked() {
                            close = true;
                        }
                    });
                });
            }
            Dialog::Highlighting => {
                let l_light = self.t("hl.light", "淺色主題：");
                let l_dark = self.t("hl.dark", "深色主題：");
                let l_ui = self.t("opt.theme", "介面主題：");
                let l_sys = self.t("opt.theme_system", "跟隨系統");
                let l_theme_light = self.t("opt.theme_light", "淺色");
                let l_theme_dark = self.t("opt.theme_dark", "深色");
                let l_close = self.t("btn.close", "關閉");
                dialog_window(ctx, self.t("dlg.syntax", "語法突顯設定")).open(&mut open).show(ctx, |ui| {
                    let themes = syntax::theme_names();
                    ui.horizontal(|ui| {
                        ui.label(l_light);
                        egui::ComboBox::from_id_salt("light-theme").selected_text(self.settings.syntax_theme_light.as_str()).show_ui(ui, |ui| {
                            for t in &themes {
                                ui.selectable_value(&mut self.settings.syntax_theme_light, t.clone(), t);
                            }
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label(l_dark);
                        egui::ComboBox::from_id_salt("dark-theme").selected_text(self.settings.syntax_theme_dark.as_str()).show_ui(ui, |ui| {
                            for t in &themes {
                                ui.selectable_value(&mut self.settings.syntax_theme_dark, t.clone(), t);
                            }
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label(l_ui);
                        let mut changed = false;
                        changed |= ui.radio_value(&mut self.settings.theme, ThemeChoice::System, l_sys).changed();
                        changed |= ui.radio_value(&mut self.settings.theme, ThemeChoice::Light, l_theme_light).changed();
                        changed |= ui.radio_value(&mut self.settings.theme, ThemeChoice::Dark, l_theme_dark).changed();
                        if changed {
                            self.apply_theme(ctx);
                        }
                    });
                    if ui.button(l_close).clicked() {
                        close = true;
                    }
                });
            }
            Dialog::SourceFormatter => {
                let l_opt = self.t("fmt.options", "AStyle 命令列選項：");
                let l_hint = self.t("fmt.hint", "需要系統已安裝 astyle 執行檔。選項說明請見 https://astyle.sourceforge.net/astyle.html");
                let l_close = self.t("btn.close", "關閉");
                dialog_window(ctx, self.t("dlg.formatter", "原始碼格式化設定（AStyle）")).open(&mut open).show(ctx, |ui| {
                    ui.label(l_opt);
                    ui.add(egui::TextEdit::singleline(&mut self.settings.astyle_options).desired_width(380.0));
                    ui.label(RichText::new(l_hint).small());
                    if ui.button(l_close).clicked() {
                        close = true;
                    }
                });
            }
            Dialog::PurgeHistories => {
                let l_ask = self.t("opt.history_ask", "要清除最近使用的檔案、搜尋與取代紀錄、資料夾紀錄與游標位置紀錄嗎？");
                let l_clear = self.t("opt.clear", "清除");
                let l_done = self.t("msg.history_cleared", "已清除歷史紀錄").to_string();
                let l_cancel = self.t("btn.cancel", "取消");
                dialog_window(ctx, self.t("dlg.history", "清除歷史紀錄")).open(&mut open).show(ctx, |ui| {
                    ui.label(l_ask);
                    ui.horizontal(|ui| {
                        if ui.button(l_clear).clicked() {
                            self.settings.purge_history();
                            self.set_status(l_done.clone());
                            close = true;
                        }
                        if ui.button(l_cancel).clicked() {
                            close = true;
                        }
                    });
                });
            }
            Dialog::ConvertEncoding { encoding, bom } => {
                let current = self.doc().encoding.name().to_string();
                let l_current = self.tf("enc.current", "目前編碼：{q}", None, Some(&current));
                let l_to = self.t("enc.to", "轉換為：");
                let l_bom = self.t("opt.add_bom", "加入 BOM");
                let l_hint = self.t("enc.hint", "轉換編碼會保留文字內容，存檔時改用新編碼寫出。若只是開檔時編碼判斷錯誤，請改用「檢視 → 編碼」重新解碼。");
                let l_convert = self.t("enc.convert", "轉換");
                let l_need = self.t("enc.need_text", "請先切換到文字模式").to_string();
                let l_fail = self.t("enc.fail", "無法轉換編碼").to_string();
                let l_cancel = self.t("btn.cancel", "取消");
                dialog_window(ctx, self.t("dlg.encoding", "轉換檔案編碼")).open(&mut open).show(ctx, |ui| {
                    ui.label(&l_current);
                    ui.horizontal(|ui| {
                        ui.label(l_to);
                        encoding_combo(ui, "conv-enc", encoding);
                    });
                    if encoding::encoding_for_name(encoding).is_some_and(encoding::supports_bom) {
                        ui.checkbox(bom, l_bom);
                    }
                    ui.label(RichText::new(l_hint).small());
                    ui.horizontal(|ui| {
                        if ui.button(l_convert).clicked() {
                            if let Some(enc) = encoding::encoding_for_name(encoding) {
                                let r = if self.doc().mode == EditMode::Hex { Err(l_need.clone()) } else { self.doc_mut().convert_encoding(enc, *bom) };
                                match r {
                                    Ok(()) => {
                                        let name = enc.name().to_string();
                                        self.set_status(self.tf("enc.done", "已轉換為 {q}，存檔時生效", None, Some(&name)));
                                    }
                                    Err(e) => self.message(&l_fail, e),
                                }
                            }
                            close = true;
                        }
                        if ui.button(l_cancel).clicked() {
                            close = true;
                        }
                    });
                });
            }
            Dialog::WordCount(wc, sel) => {
                let l_sel = self.t("wc.selection", "（選取範圍）");
                let l_doc = self.t("wc.document", "（整份文件）");
                let rows = [
                    (self.t("wc.words", "字數"), wc.words),
                    (self.t("wc.chars", "字元數（不含換行）"), wc.chars),
                    (self.t("wc.spaces", "空白字元"), wc.spaces),
                    (self.t("wc.half", "半形字元"), wc.halfwidth),
                    (self.t("wc.full", "全形字元"), wc.fullwidth),
                    (self.t("wc.cjk", "中日韓文字"), wc.cjk),
                    (self.t("wc.lines", "行數"), wc.lines),
                ];
                let l_close = self.t("btn.close", "關閉");
                dialog_window(ctx, self.t("dlg.words", "字數統計")).open(&mut open).show(ctx, |ui| {
                    ui.label(if *sel { l_sel } else { l_doc });
                    egui::Grid::new("wc").num_columns(2).show(ui, |ui| {
                        for (k, v) in rows {
                            ui.label(k);
                            ui.monospace(v.to_string());
                            ui.end_row();
                        }
                    });
                    if ui.button(l_close).clicked() {
                        close = true;
                    }
                });
            }
            Dialog::WindowList => {
                let mut go = None;
                let mut close_ids = Vec::new();
                let l_close_one = self.t("btn.close", "關閉");
                let l_close_list = self.t("btn.close_list", "關閉清單");
                dialog_window(ctx, self.t("dlg.windows", "視窗清單")).open(&mut open).default_size([480.0, 320.0]).show(ctx, |ui| {
                    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                        for (i, t) in self.tabs.iter().enumerate() {
                            ui.horizontal(|ui| {
                                let name = format!("{}{}", t.doc.display_name(), if t.doc.is_modified() { " *" } else { "" });
                                if ui.selectable_label(i == self.active, name).clicked() {
                                    go = Some(i);
                                }
                                ui.label(RichText::new(t.doc.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default()).small().weak());
                                if ui.small_button(l_close_one).clicked() {
                                    close_ids.push(t.doc.id);
                                }
                            });
                        }
                    });
                    if ui.button(l_close_list).clicked() {
                        close = true;
                    }
                });
                if let Some(i) = go {
                    self.activate(i);
                }
                if !close_ids.is_empty() {
                    self.request_close(close_ids, false);
                }
            }
            Dialog::About => {
                let l_tag = self.t("about.tagline", "以 Rust 與 egui 重製的文字／十六進位編輯器。");
                let l_credit = self.t("about.credit", "原版 MadEdit 由 Alston Chen 開發，MadEdit-Mod 由 Minggang Li 維護（GPL）。");
                let l_assets = self.t("about.assets", "工具列圖示、簡繁對照表與拼字字典取自原版。");
                let l_ok = self.t("btn.ok", "確定");
                dialog_window(ctx, self.t("about.title", "關於 MadEdit")).open(&mut open).collapsible(false).show(ctx, |ui| {
                    ui.heading(format!("MadEdit {}", env!("CARGO_PKG_VERSION")));
                    ui.label(l_tag);
                    ui.label(l_credit);
                    ui.label(l_assets);
                    if ui.button(l_ok).clicked() {
                        close = true;
                    }
                });
            }
        }
        if open && !close {
            self.dialog = Some(dlg);
        }
    }

    /// 插入遞增數字：列塊選取時每行一個，否則自游標處起逐行插入
    fn insert_numbers(&mut self) {
        let opts = self.settings.numbers.clone();
        let doc = self.doc_mut();
        let sel = doc.editor.selection;
        if sel.is_block() {
            let b = sel.visual_bounds(&doc.editor);
            let nums = textops::incremental_numbers(b.max_line - b.min_line + 1, &opts);
            let hint = doc.selection_hint();
            doc.edit(EditKind::Other, hint, |ed| {
                ed.delete_selection();
                for (k, l) in (b.min_line..=b.max_line).enumerate() {
                    let line_v = crate::core::editor::char_col_to_visual_col(&ed.lines[l], ed.line_char_count(l));
                    if line_v < b.min_vcol {
                        ed.lines[l].push_str(&" ".repeat(b.min_vcol - line_v));
                    }
                    let col = crate::core::editor::visual_col_to_char_col(&ed.lines[l], b.min_vcol);
                    let byte = crate::core::editor::char_to_byte(&ed.lines[l], col);
                    ed.lines[l].insert_str(byte, &nums[k]);
                }
                ed.clamp_caret();
            });
        } else {
            let nums = textops::incremental_numbers(opts.total.max(1), &opts);
            let head = sel.head;
            let hint = head.line..doc.editor.lines.len();
            doc.edit(EditKind::Other, hint, |ed| {
                let vcol = crate::core::editor::char_col_to_visual_col(&ed.lines[head.line], head.col);
                for (k, n) in nums.iter().enumerate() {
                    let l = head.line + k;
                    if l >= ed.lines.len() {
                        ed.lines.push(String::new());
                    }
                    let line_v = crate::core::editor::char_col_to_visual_col(&ed.lines[l], ed.line_char_count(l));
                    if line_v < vcol {
                        ed.lines[l].push_str(&" ".repeat(vcol - line_v));
                    }
                    let col = crate::core::editor::visual_col_to_char_col(&ed.lines[l], vcol);
                    let byte = crate::core::editor::char_to_byte(&ed.lines[l], col);
                    ed.lines[l].insert_str(byte, n);
                }
                ed.clamp_caret();
            });
        }
    }

    fn options_dialog(&mut self, ctx: &egui::Context, st: &mut OptionsState, open: &mut bool) {
        let draft = st.draft.get_or_insert_with(|| self.settings.clone());
        let mut apply = false;
        let mut ok = false;
        let mut cancel = false;
        // 擷取快捷鍵
        if let Some(c) = st.capturing {
            let pressed = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Key { key, pressed: true, modifiers, .. } if !matches!(key, Key::Escape) => Some(egui::KeyboardShortcut::new(*modifiers, *key)),
                    _ => None,
                })
            });
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                st.capturing = None;
            } else if let Some(mut sc) = pressed.filter(commands::is_bindable) {
                // 只保留邏輯修飾鍵（macOS 的 ⌘ 視為 Ctrl）
                sc.modifiers = egui::Modifiers { mac_cmd: false, ..sc.modifiers };
                let text = commands::shortcut_to_string(&sc);
                let list = draft.key_bindings.entry(c.id().to_string()).or_insert_with(|| c.default_keys().split('|').filter(|k| !k.is_empty()).map(str::to_string).collect());
                if !list.contains(&text) {
                    list.push(text);
                }
                st.capturing = None;
            }
        }
        let lang = self.ui_lang();
        let tabs = [
            self.t("opt.general", "一般"),
            self.t("opt.edit", "編輯"),
            self.t("opt.new", "新文件"),
            self.t("opt.print", "列印"),
            self.t("opt.keys", "按鍵"),
            self.t("opt.spell", "拼字檢查"),
        ];
        let l_lang = self.t("opt.ui_language", "介面語言：");
        let l_def_enc = self.t("opt.default_enc", "預設編碼：");
        let l_fallback_enc = self.t("opt.fallback_enc", "備援編碼（非 UTF-8 時）：");
        let l_enforce = self.t("opt.enforce_enc", "一律使用預設編碼開檔（不自動偵測）");
        let l_max = self.t("opt.max_mb", "文字模式檔案大小上限（MB）：");
        let l_open = self.t("opt.open_mode", "開檔模式：");
        let l_auto = self.t("opt.open_auto", "自動（二進位檔用十六進位）");
        let l_text = self.t("opt.open_text", "文字模式");
        let l_column = self.t("opt.open_column", "列塊模式");
        let l_hex = self.t("opt.open_hex", "十六進位模式");
        let l_search_chars = self.t("opt.search_chars", "搜尋結果每行最多字元：");
        let l_theme = self.t("opt.theme", "介面主題：");
        let l_theme_sys = self.t("opt.theme_system", "跟隨系統");
        let l_theme_light = self.t("opt.theme_light", "淺色");
        let l_theme_dark = self.t("opt.theme_dark", "深色");
        let l_font = self.t("opt.editor_font", "編輯區字型：");
        let font_auto = self.tf("opt.font_auto", "自動（{q}）", None, Some(&self.fonts.editor_family));
        let l_reload = self.t("opt.reload_startup", "啟動時重新開啟上次的檔案");
        let l_caret = self.t("opt.restore_caret", "開檔時還原上次的游標位置");
        let l_autosave = self.t("opt.autosave", "自動儲存，每");
        let l_minutes = self.t("opt.minutes", "分鐘");
        let l_backup = self.t("opt.backup", "存檔時保留備份（檔名.bak）");
        let l_purge = self.t("opt.purge_exit", "結束時清除歷史紀錄");
        dialog_window(ctx, self.t("dlg.options", "選項")).open(open).default_size([620.0, 480.0]).show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (i, name) in tabs.iter().enumerate() {
                    ui.selectable_value(&mut st.tab, i, *name);
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| match st.tab {
                0 => {
                    egui::Grid::new("opt-general").num_columns(2).show(ui, |ui| {
                        ui.label(l_lang);
                        let current = i18n::language_choice_label(lang, draft.ui_language);
                        egui::ComboBox::from_id_salt("ui-lang").selected_text(current).show_ui(ui, |ui| {
                            for choice in UiLanguage::ALL {
                                ui.selectable_value(&mut draft.ui_language, choice, i18n::language_choice_label(lang, choice));
                            }
                        });
                        ui.end_row();
                        ui.label(l_def_enc);
                        encoding_combo(ui, "def-enc", &mut draft.default_encoding);
                        ui.end_row();
                        ui.label(l_fallback_enc);
                        encoding_combo(ui, "fallback-enc", &mut draft.fallback_encoding);
                        ui.end_row();
                        ui.label("");
                        ui.checkbox(&mut draft.enforce_default_encoding, l_enforce);
                        ui.end_row();
                        ui.label(l_max);
                        ui.add(egui::DragValue::new(&mut draft.max_text_file_mb).range(1..=4096));
                        ui.end_row();
                        ui.label(l_open);
                        let open_label = match draft.open_view_mode {
                            OpenViewMode::Auto => l_auto,
                            OpenViewMode::Text => l_text,
                            OpenViewMode::Column => l_column,
                            OpenViewMode::Hex => l_hex,
                        };
                        egui::ComboBox::from_id_salt("open-mode").selected_text(open_label).show_ui(ui, |ui| {
                            ui.selectable_value(&mut draft.open_view_mode, OpenViewMode::Auto, l_auto);
                            ui.selectable_value(&mut draft.open_view_mode, OpenViewMode::Text, l_text);
                            ui.selectable_value(&mut draft.open_view_mode, OpenViewMode::Column, l_column);
                            ui.selectable_value(&mut draft.open_view_mode, OpenViewMode::Hex, l_hex);
                        });
                        ui.end_row();
                        ui.label(l_search_chars);
                        ui.add(egui::DragValue::new(&mut draft.max_search_result_chars).range(20..=1000));
                        ui.end_row();
                        ui.label(l_theme);
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut draft.theme, ThemeChoice::System, l_theme_sys);
                            ui.radio_value(&mut draft.theme, ThemeChoice::Light, l_theme_light);
                            ui.radio_value(&mut draft.theme, ThemeChoice::Dark, l_theme_dark);
                        });
                        ui.end_row();
                        ui.label(l_font);
                        ui.label(if draft.font_family.is_empty() { font_auto.clone() } else { draft.font_family.clone() });
                        ui.end_row();
                    });
                    ui.checkbox(&mut draft.reload_files_on_startup, l_reload);
                    ui.checkbox(&mut draft.restore_caret_position, l_caret);
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut draft.auto_save, l_autosave);
                        ui.add(egui::DragValue::new(&mut draft.auto_save_minutes).range(1..=120));
                        ui.label(l_minutes);
                    });
                    ui.checkbox(&mut draft.auto_backup, l_backup);
                    ui.checkbox(&mut draft.purge_history_on_exit, l_purge);
                }
                1 => {
                    egui::Grid::new("opt-edit").num_columns(2).show(ui, |ui| {
                        ui.label(self.t("opt.wrap_col", "依欄數換行的欄數："));
                        ui.add(egui::DragValue::new(&mut draft.wrap_column).range(10..=1000));
                        ui.end_row();
                        ui.label(self.t("opt.tab_size", "Tab 欄數："));
                        ui.add(egui::DragValue::new(&mut draft.tab_size).range(1..=16));
                        ui.end_row();
                        ui.label(self.t("opt.indent", "縮排欄數："));
                        ui.add(egui::DragValue::new(&mut draft.indent_size).range(1..=16));
                        ui.end_row();
                        ui.label(self.t("opt.guide", "欄位提示線位置："));
                        ui.add(egui::DragValue::new(&mut draft.col_hint_position).range(1..=1000));
                        ui.end_row();
                        ui.label(self.t("opt.datetime", "日期時間格式："));
                        ui.text_edit_singleline(&mut draft.date_time_format);
                        ui.end_row();
                        ui.label(self.t("opt.spacing", "行距（%）："));
                        ui.add(egui::DragValue::new(&mut draft.line_spacing).range(100..=250));
                        ui.end_row();
                    });
                    let preview = chrono::Local::now().format(&draft.date_time_format).to_string();
                    ui.label(RichText::new(self.tf("opt.date_preview", "日期預覽：{q}", None, Some(&preview))).small());
                    ui.checkbox(&mut draft.insert_spaces_for_tab, self.t("opt.spaces_for_tab", "以空白取代 Tab"));
                    ui.checkbox(&mut draft.auto_indent, self.t("opt.auto_indent", "自動縮排"));
                    ui.checkbox(&mut draft.auto_complete_pair, self.t("opt.auto_pair", "自動補全成對字元（括號、引號）"));
                    ui.checkbox(&mut draft.wrap_selection_with_pair, self.t("opt.wrap_pair", "有選取時以成對字元包住選取內容"));
                    ui.checkbox(&mut draft.auto_copy_mouse_selection, self.t("opt.auto_copy", "滑鼠選取後自動複製"));
                    ui.checkbox(&mut draft.middle_click_paste, self.t("opt.middle_paste", "滑鼠中鍵貼上"));
                    ui.checkbox(&mut draft.auto_fill_column_paste, self.t("opt.fill_paste", "列塊貼上時以空白補齊短行"));
                    ui.checkbox(&mut draft.double_click_highlight, self.t("opt.double_click", "雙擊時標示相同字詞"));
                    ui.checkbox(&mut draft.typewriter_mode, self.t("opt.typewriter", "打字機模式（游標保持在畫面中央）"));
                    ui.checkbox(&mut draft.fixed_width, self.t("opt.fixed", "等寬模式"));
                }
                2 => {
                    egui::Grid::new("opt-new").num_columns(2).show(ui, |ui| {
                        ui.label(self.t("opt.encoding", "編碼："));
                        encoding_combo(ui, "new-enc", &mut draft.new_doc_encoding);
                        ui.end_row();
                        ui.label("");
                        ui.checkbox(&mut draft.new_doc_bom, self.t("opt.add_bom", "加入 BOM"));
                        ui.end_row();
                        ui.label(self.t("opt.newline", "換行字元："));
                        ui.horizontal(|ui| {
                            for n in ["CRLF", "LF", "CR"] {
                                ui.radio_value(&mut draft.new_doc_newline, n.to_string(), n);
                            }
                        });
                        ui.end_row();
                        ui.label(self.t("opt.syntax", "語法："));
                        egui::ComboBox::from_id_salt("new-syntax").selected_text(draft.new_doc_syntax.as_str()).show_ui(ui, |ui| {
                            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                                for s in syntax::syntax_names() {
                                    ui.selectable_value(&mut draft.new_doc_syntax, s.clone(), s);
                                }
                            });
                        });
                        ui.end_row();
                    });
                }
                3 => print_options_ui(ui, &mut draft.print, lang),
                4 => {
                    ui.horizontal(|ui| {
                        ui.label(self.t("opt.filter", "篩選："));
                        ui.text_edit_singleline(&mut st.filter);
                    });
                    ui.columns(2, |cols| {
                        egui::ScrollArea::vertical().id_salt("cmd-list").max_height(300.0).show(&mut cols[0], |ui| {
                            let f = st.filter.to_lowercase();
                            for &c in commands::ALL {
                                let label = self.cmd_text(c);
                                if f.is_empty() || label.to_lowercase().contains(&f) || c.id().to_lowercase().contains(&f) {
                                    let keys = draft.key_bindings.get(c.id()).map(|v| v.join(", ")).unwrap_or_else(|| c.default_keys().replace('|', ", "));
                                    let text = if keys.is_empty() { label } else { format!("{label}　[{keys}]") };
                                    if ui.selectable_label(st.selected == Some(c), text).clicked() {
                                        st.selected = Some(c);
                                    }
                                }
                            }
                        });
                        let ui = &mut cols[1];
                        if let Some(c) = st.selected {
                            ui.strong(self.cmd_text(c));
                            ui.label(RichText::new(c.id()).small().weak());
                            let list: Vec<String> = draft
                                .key_bindings
                                .get(c.id())
                                .cloned()
                                .unwrap_or_else(|| c.default_keys().split('|').filter(|k| !k.is_empty()).map(str::to_string).collect());
                            let mut remove = None;
                            for (i, k) in list.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.monospace(k);
                                    if ui.small_button(self.t("btn.remove", "移除")).clicked() {
                                        remove = Some(i);
                                    }
                                });
                            }
                            if let Some(i) = remove {
                                let mut l = list.clone();
                                l.remove(i);
                                draft.key_bindings.insert(c.id().to_string(), l);
                            }
                            if st.capturing == Some(c) {
                                ui.colored_label(egui::Color32::from_rgb(220, 120, 0), self.t("opt.press_key", "請按下新的快捷鍵（需含 Ctrl／⌘／Alt，F 鍵除外；Esc 取消）…"));
                            } else if ui.button(self.t("btn.add_shortcut", "新增快捷鍵…")).clicked() {
                                st.capturing = Some(c);
                            }
                            if ui.button(self.t("btn.restore", "還原預設")).clicked() {
                                draft.key_bindings.remove(c.id());
                            }
                            // 衝突提示。{n} 是另一個命令的名稱，不是數字
                            for k in &list {
                                for &other in commands::ALL {
                                    if other == c {
                                        continue;
                                    }
                                    let okeys: Vec<String> = draft.key_bindings.get(other.id()).cloned().unwrap_or_else(|| other.default_keys().split('|').map(str::to_string).collect());
                                    if okeys.iter().any(|o| o == k) {
                                        let name = self.cmd_text(other);
                                        let msg = self.t("opt.conflict", "{q} 也指派給「{n}」").replace("{q}", k).replace("{n}", &name);
                                        ui.colored_label(egui::Color32::RED, msg);
                                    }
                                }
                            }
                        } else {
                            ui.label(self.t("opt.pick_command", "請在左側選擇命令"));
                        }
                    });
                }
                _ => {
                    let langs = spell::available_languages();
                    ui.horizontal(|ui| {
                        ui.label(self.t("opt.spell_lang", "字典語言："));
                        egui::ComboBox::from_id_salt("spell-lang").selected_text(draft.spell_language.as_str()).show_ui(ui, |ui| {
                            for l in langs {
                                ui.selectable_value(&mut draft.spell_language, l.clone(), l);
                            }
                        });
                    });
                    if let Some(e) = &self.spell_error {
                        ui.colored_label(egui::Color32::RED, e);
                    }
                    if let Some(dir) = spell::dictionary_dirs().first() {
                        let path = dir.display().to_string();
                        ui.label(RichText::new(self.tf("opt.dict_dir", "可將其他語言的 Hunspell 字典（.aff 與 .dic）放到：{q}", None, Some(&path))).small());
                    }
                    ui.separator();
                    ui.label(self.t("opt.personal", "個人字典："));
                    let mut remove = None;
                    egui::ScrollArea::vertical().id_salt("personal").max_height(200.0).show(ui, |ui| {
                        for (i, w) in draft.personal_dictionary.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(w);
                                if ui.small_button(self.t("btn.remove", "移除")).clicked() {
                                    remove = Some(i);
                                }
                            });
                        }
                    });
                    if let Some(i) = remove {
                        draft.personal_dictionary.remove(i);
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(self.t("btn.ok", "確定")).clicked() {
                    ok = true;
                }
                if ui.button(self.t("btn.apply", "套用")).clicked() {
                    apply = true;
                }
                if ui.button(self.t("btn.cancel", "取消")).clicked() {
                    cancel = true;
                }
            });
        });
        if ok || apply {
            let new = st.draft.clone().unwrap();
            let spell_changed = new.spell_language != self.settings.spell_language || new.personal_dictionary != self.settings.personal_dictionary;
            let theme_changed = new.theme != self.settings.theme;
            self.settings = new;
            self.rebuild_bindings();
            if theme_changed {
                self.apply_theme(ctx);
            }
            if spell_changed && self.settings.spell_check {
                self.load_spell();
            }
            let _ = self.settings.save();
        }
        if ok || cancel {
            *open = false;
        }
    }

    pub fn show_confirm(&mut self, ctx: &egui::Context) {
        let Some(confirm) = self.confirm.take() else { return };
        let mut keep = true;
        let modal = egui::Modal::new(egui::Id::unique("confirm"));
        match &confirm {
            Confirm::Message { title, text } => {
                modal.show(ctx, |ui| {
                    ui.heading(title);
                    ui.label(text);
                    if ui.button(self.t("btn.ok", "確定")).clicked() || ui.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Escape)) {
                        keep = false;
                    }
                });
            }
            Confirm::Close { ids, exit } => {
                let id = ids[0];
                let Some(idx) = self.index_of(id) else {
                    let rest: Vec<u64> = ids[1..].to_vec();
                    self.confirm = if rest.is_empty() { None } else { Some(Confirm::Close { ids: rest, exit: *exit }) };
                    if rest_empty_and_exit(&self.confirm, *exit) {
                        self.allow_close = true;
                    }
                    return;
                };
                let name = self.tabs[idx].doc.display_name();
                let mut choice = None;
                let ask = self.tf("confirm.save_ask", "「{q}」已修改，要儲存嗎？", None, Some(&name));
                modal.show(ctx, |ui| {
                    ui.heading(self.t("confirm.unsaved", "尚未儲存"));
                    ui.label(ask);
                    ui.horizontal(|ui| {
                        if ui.button(self.t("btn.save", "儲存")).clicked() {
                            choice = Some(0);
                        }
                        if ui.button(self.t("btn.dont_save", "不儲存")).clicked() {
                            choice = Some(1);
                        }
                        if ui.button(self.t("btn.cancel", "取消")).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                            choice = Some(2);
                        }
                    });
                });
                match choice {
                    Some(0) => {
                        self.activate(idx);
                        self.active = idx;
                        if self.save_tab(idx, false) {
                            if !*exit { self.close_now(id); }
                            self.continue_close(ids, *exit);
                        } else if self.confirm.is_some() {
                            // 存檔需要再確認：記下關閉流程，確認後存檔成功再接續關閉
                            self.pending_close = Some((ids.clone(), *exit));
                        }
                        // 否則為使用者取消另存對話框：中止關閉
                        return;
                    }
                    Some(1) => {
                        if !*exit { self.close_now(id); }
                        self.continue_close(ids, *exit);
                        return;
                    }
                    Some(_) => keep = false,
                    None => {}
                }
            }
            Confirm::SaveLossy { id, path } => {
                let mut choice = None;
                modal.show(ctx, |ui| {
                    ui.heading(self.t("confirm.lossy_title", "存檔可能遺失資料"));
                    ui.label(self.t("confirm.lossy_body", "這份文件含有無法以目前編碼解碼的位元組（顯示為 �）。直接存檔會以 U+FFFD 取代，原始位元組將永久遺失。"));
                    ui.label(self.t("confirm.lossy_hint", "建議先以「檢視 → 編碼」選擇正確的編碼重新解碼。"));
                    ui.horizontal(|ui| {
                        if ui.button(self.t("btn.save_anyway", "仍要存檔")).clicked() {
                            choice = Some(true);
                        }
                        if ui.button(self.t("btn.cancel", "取消")).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                            choice = Some(false);
                        }
                    });
                });
                if let Some(c) = choice {
                    keep = false;
                    if c {
                        let (id, path) = (*id, path.clone());
                        self.save_and_continue_close(id, path, true);
                        return;
                    }
                }
            }
            Confirm::SaveUnmappable { id, path, message } => {
                let mut choice = None;
                modal.show(ctx, |ui| {
                    ui.heading(self.t("confirm.unmappable_title", "無法以目前編碼存檔"));
                    ui.label(message);
                    ui.label(self.t("confirm.unmappable_hint", "可改用 UTF-8 存檔，或取消後修改內容。"));
                    ui.horizontal(|ui| {
                        if ui.button(self.t("btn.save_utf8", "改用 UTF-8 存檔")).clicked() {
                            choice = Some(true);
                        }
                        if ui.button(self.t("btn.cancel", "取消")).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                            choice = Some(false);
                        }
                    });
                });
                if let Some(c) = choice {
                    keep = false;
                    if c && let Some(i) = self.index_of(*id) {
                        let bom = self.tabs[i].doc.has_bom;
                        if self.tabs[i].doc.convert_encoding(encoding_rs::UTF_8, bom).is_ok() {
                            let (id, path) = (*id, path.clone());
                            self.save_and_continue_close(id, path, false);
                            return;
                        }
                    }
                }
            }
            Confirm::OverwriteExternal { id, path } => {
                let mut choice = None;
                modal.show(ctx, |ui| {
                    ui.heading(self.t("confirm.external_title", "檔案已被其他程式修改"));
                    ui.label(self.t("confirm.external_body", "磁碟上的檔案在開啟後已被修改。覆寫會遺失其他程式所做的變更。"));
                    ui.horizontal(|ui| {
                        if ui.button(self.t("btn.overwrite", "覆寫")).clicked() {
                            choice = Some(0);
                        }
                        if ui.button(self.t("btn.reload_discard", "重新載入（放棄我的修改）")).clicked() {
                            choice = Some(1);
                        }
                        if ui.button(self.t("btn.cancel", "取消")).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                            choice = Some(2);
                        }
                    });
                });
                if let Some(c) = choice {
                    keep = false;
                    let (id, path) = (*id, path.clone());
                    match c {
                        0 => {
                            self.save_and_continue_close(id, path, false);
                            return;
                        }
                        1 => self.reload_tab(id),
                        _ => {}
                    }
                }
            }
            Confirm::ReloadModified { id } => {
                let mut choice = None;
                modal.show(ctx, |ui| {
                    ui.heading(self.t("confirm.reload_title", "重新載入"));
                    ui.label(self.t("confirm.reload_body", "重新載入會放棄目前未存檔的修改，確定嗎？"));
                    ui.horizontal(|ui| {
                        if ui.button(self.t("btn.reload", "重新載入")).clicked() {
                            choice = Some(true);
                        }
                        if ui.button(self.t("btn.cancel", "取消")).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                            choice = Some(false);
                        }
                    });
                });
                if let Some(c) = choice {
                    keep = false;
                    if c {
                        self.reload_tab(*id);
                    }
                }
            }
            Confirm::CreateNew { path } => {
                let mut choice = None;
                let name = path.display().to_string();
                let body = self.tf("confirm.create_body", "「{q}」不存在，是否要建立新的檔案？", None, Some(&name));
                modal.show(ctx, |ui| {
                    ui.heading(self.t("confirm.create_title", "檔案不存在"));
                    ui.label(body);
                    ui.horizontal(|ui| {
                        if ui.button(self.t("btn.create", "建立")).clicked() {
                            choice = Some(true);
                        }
                        if ui.button(self.t("btn.cancel", "取消")).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                            choice = Some(false);
                        }
                    });
                });
                if let Some(c) = choice {
                    keep = false;
                    if c {
                        self.create_new_file(path.clone());
                    }
                    // 已詢問的路徑移出佇列，其餘的稍後繼續詢問
                    if let Some(pos) = self.pending_create.iter().position(|p| p == path) {
                        self.pending_create.remove(pos);
                    }
                }
            }
        }
        if keep && self.confirm.is_none() {
            self.confirm = Some(confirm);
        } else if !keep && self.confirm.is_none() {
            // 確認視窗已關閉且沒有後續確認（取消、重新載入或存檔失敗）：中止暫存的關閉流程
            self.pending_close = None;
            // 還有其他等待詢問是否建立的檔案時，繼續問下一個
            if let Some(next) = self.pending_create.first() {
                self.confirm = Some(Confirm::CreateNew { path: next.clone() });
            }
        }
    }

    /// 存檔；若關閉流程正等待這份文件存檔，成功後接續關閉
    fn save_and_continue_close(&mut self, id: u64, path: Option<std::path::PathBuf>, allow_lossy: bool) {
        if !self.save_with(id, path, allow_lossy) {
            return;
        }
        if let Some((ids, exit)) = self.pending_close.take_if(|(ids, _)| ids[0] == id) {
            if !exit { self.close_now(id); }
            self.continue_close(&ids, exit);
        }
    }

    /// 處理完一份文件後，繼續詢問下一份；全部處理完且為結束程式時允許關閉
    fn continue_close(&mut self, ids: &[u64], exit: bool) {
        let rest: Vec<u64> = ids[1..].to_vec();
        if rest.is_empty() {
            if exit {
                self.allow_close = true;
            }
        } else {
            self.confirm = Some(Confirm::Close { ids: rest, exit });
        }
    }
}

fn rest_empty_and_exit(next: &Option<Confirm>, exit: bool) -> bool {
    next.is_none() && exit
}
