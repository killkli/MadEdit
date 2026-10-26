//! 選單列與工具列（結構對應原版 MadEdit 的 CommandTable）

use eframe::egui::{self, Button};

use super::commands::C;
use super::App;
use crate::core::encoding::{self, ALL_ENCODINGS, COMMON_ENCODINGS};
use crate::core::model::{EditMode, NewLineFormat};
use crate::core::settings::{UiLanguage, WrapMode};
use crate::core::syntax;

/// 選單項目的勾選標記
fn check_label(checked: bool, text: &str) -> String {
    format!("{} {}", if checked { "✔" } else { "　" }, text)
}

impl App {
    /// 一般命令項目
    fn item(&mut self, ui: &mut egui::Ui, c: C) {
        self.item_enabled(ui, c, true);
    }

    fn item_enabled(&mut self, ui: &mut egui::Ui, c: C, enabled: bool) {
        let sc = self.shortcut_text(c, ui.ctx());
        if ui.add_enabled(enabled, Button::new(check_label(false, &self.cmd_text(c))).shortcut_text(sc)).clicked() {
            self.pending.push(c);
            ui.close();
        }
    }

    /// 可勾選的命令項目
    fn check(&mut self, ui: &mut egui::Ui, c: C, checked: bool) {
        let sc = self.shortcut_text(c, ui.ctx());
        if ui.add(Button::new(check_label(checked, &self.cmd_text(c))).shortcut_text(sc)).clicked() {
            self.pending.push(c);
            ui.close();
        }
    }

    pub fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(self.t("menu.file", "檔案(F)"), |ui| self.file_menu(ui));
            ui.menu_button(self.t("menu.edit", "編輯(E)"), |ui| self.edit_menu(ui));
            ui.menu_button(self.t("menu.search", "搜尋(S)"), |ui| self.search_menu(ui));
            ui.menu_button(self.t("menu.view", "檢視(V)"), |ui| self.view_menu(ui));
            ui.menu_button(self.t("menu.macro", "巨集(M)"), |ui| self.macro_menu(ui));
            ui.menu_button(self.t("menu.tools", "工具(T)"), |ui| self.tools_menu(ui));
            ui.menu_button(self.t("menu.window", "視窗(W)"), |ui| self.window_menu(ui));
            ui.menu_button(self.t("menu.help", "說明(H)"), |ui| self.item(ui, C::About));
        });
    }

    fn file_menu(&mut self, ui: &mut egui::Ui) {
        let has_path = self.doc().path.is_some();
        self.item(ui, C::New);
        self.item(ui, C::Open);
        ui.separator();
        self.item(ui, C::Save);
        self.item(ui, C::SaveAs);
        self.item(ui, C::SaveACopy);
        self.item(ui, C::SaveAll);
        ui.separator();
        self.item_enabled(ui, C::Reload, has_path);
        ui.separator();
        self.item(ui, C::Close);
        self.item(ui, C::CloseAll);
        self.item(ui, C::CloseOthers);
        self.item(ui, C::CloseLeft);
        self.item(ui, C::CloseRight);
        ui.separator();
        let ro = self.doc().read_only;
        self.check(ui, C::ToggleReadOnly, ro);
        ui.separator();
        self.item(ui, C::PageSetup);
        self.item(ui, C::PrintPreview);
        self.item(ui, C::Print);
        ui.separator();
        ui.menu_button(check_label(false, self.t("menu.recent", "最近使用的檔案")), |ui| {
            let recent = self.settings.recent_files.clone();
            if recent.is_empty() {
                ui.label(self.t("menu.none", "（無）"));
            }
            for (i, p) in recent.iter().enumerate() {
                if ui.button(format!("{} {}", i + 1, p.display())).clicked() {
                    self.open_path(p);
                    ui.close();
                }
            }
        });
        self.item(ui, C::RecentFilesList);
        ui.separator();
        self.item(ui, C::Exit);
    }

    fn edit_menu(&mut self, ui: &mut egui::Ui) {
        let can_undo = self.doc().can_undo();
        let can_redo = self.doc().can_redo();
        self.item_enabled(ui, C::Undo, can_undo);
        self.item_enabled(ui, C::Redo, can_redo);
        ui.separator();
        self.item(ui, C::Cut);
        self.item(ui, C::Copy);
        self.item(ui, C::Paste);
        self.item(ui, C::Delete);
        ui.separator();
        self.item(ui, C::CutLine);
        self.item(ui, C::DeleteLine);
        self.item(ui, C::DuplicateLine);
        self.item(ui, C::MoveLinesUp);
        self.item(ui, C::MoveLinesDown);
        self.item(ui, C::JoinLines);
        self.item(ui, C::DeleteEmptyLines);
        self.item(ui, C::DeleteEmptyLinesWithSpaces);
        self.item(ui, C::TrimTrailingSpaces);
        self.item(ui, C::TrimLeadingSpaces);
        ui.separator();
        self.item(ui, C::SelectAll);
        let sel_mode = self.doc().selecting_mode;
        self.check(ui, C::StartEndSelecting, sel_mode);
        ui.separator();
        self.item(ui, C::InsertTabChar);
        self.item(ui, C::InsertDateTime);
        ui.separator();
        ui.menu_button(check_label(false, self.t("menu.bookmarks", "書籤")), |ui| {
            for c in [C::BookmarkCopy, C::BookmarkCut, C::BookmarkDel, C::BookmarkCopyUnmarked, C::BookmarkCutUnmarked, C::BookmarkDelUnmarked, C::BookmarkReplace] {
                self.item(ui, c);
            }
        });
        ui.separator();
        ui.menu_button(check_label(false, self.t("menu.advanced", "進階")), |ui| {
            self.item(ui, C::CopyAsHexString);
            self.item(ui, C::CopyAsHexStringWithSpace);
            self.item(ui, C::CopyRevertHex);
            ui.separator();
            self.item(ui, C::IncreaseIndent);
            self.item(ui, C::DecreaseIndent);
            ui.separator();
            self.item(ui, C::Comment);
            self.item(ui, C::Uncomment);
            ui.separator();
            self.item(ui, C::WordWrapToNewLine);
            self.item(ui, C::NewLineToWordWrap);
            ui.separator();
            self.item(ui, C::ToUpperCase);
            self.item(ui, C::ToLowerCase);
            self.item(ui, C::InvertCase);
            self.item(ui, C::Capitalize);
            ui.separator();
            self.item(ui, C::ToHalfWidth);
            self.item(ui, C::ToHalfWidthByOptions);
            self.item(ui, C::ToFullWidth);
            self.item(ui, C::ToFullWidthByOptions);
            ui.separator();
            self.item(ui, C::TabToSpace);
            self.item(ui, C::SpaceToTab);
            ui.separator();
            self.item(ui, C::InsertNumbers);
            self.item(ui, C::ColumnAlignLeft);
            self.item(ui, C::ColumnAlignRight);
        });
        ui.separator();
        ui.menu_button(check_label(false, self.t("menu.sort", "排序")), |ui| {
            self.item(ui, C::SortAscending);
            self.item(ui, C::SortDescending);
            ui.separator();
            self.item(ui, C::SortAscendingCase);
            self.item(ui, C::SortDescendingCase);
            ui.separator();
            self.item(ui, C::SortByOptions);
            self.item(ui, C::SortOptions);
        });
    }

    fn search_menu(&mut self, ui: &mut egui::Ui) {
        self.item(ui, C::Find);
        self.item(ui, C::FindNext);
        self.item(ui, C::FindPrevious);
        self.item(ui, C::QuickFind);
        ui.separator();
        self.item(ui, C::Replace);
        ui.separator();
        self.item(ui, C::FindInFiles);
        let shown = self.fif.show_results;
        self.check(ui, C::ShowFindInFilesResults, shown);
        let collapsed = self.fif.collapsed;
        self.check(ui, C::CollapseAllResults, collapsed);
        ui.separator();
        self.item(ui, C::GoToLine);
        self.item(ui, C::GoToPosition);
        ui.separator();
        self.item(ui, C::GoBack);
        self.item(ui, C::GoForward);
        ui.separator();
        self.item(ui, C::LeftBrace);
        self.item(ui, C::RightBrace);
        ui.separator();
        self.item(ui, C::ToggleBookmark);
        self.item(ui, C::NextBookmark);
        self.item(ui, C::PreviousBookmark);
        self.item(ui, C::ClearAllBookmarks);
    }

    /// 檢視選單最上方：立刻切換介面語言並寫入設定
    fn language_menu(&mut self, ui: &mut egui::Ui) {
        let current = self.settings.ui_language;
        let lang = self.ui_lang();
        ui.menu_button(check_label(false, self.t("menu.language", "介面語言")), |ui| {
            for choice in UiLanguage::ALL {
                let label = super::i18n::language_choice_label(lang, choice);
                if ui.button(check_label(current == choice, label)).clicked() {
                    self.settings.ui_language = choice;
                    let _ = self.settings.save();
                    ui.close();
                }
            }
        });
    }

    fn view_menu(&mut self, ui: &mut egui::Ui) {
        self.language_menu(ui);
        ui.separator();
        let s = self.settings.clone();
        self.check(ui, C::AlwaysOnTop, s.always_on_top);
        self.check(ui, C::FullScreen, self.fullscreen);
        self.check(ui, C::PostIt, self.post_it);
        ui.separator();
        let enc_name = self.doc().encoding.name().to_string();
        ui.menu_button(check_label(false, &self.tf("menu.encoding", "編碼：{q}", None, Some(&enc_name))), |ui| {
            let mut chosen = None;
            for &e in COMMON_ENCODINGS {
                if ui.button(check_label(e == enc_name, e)).clicked() {
                    chosen = Some(e);
                }
            }
            ui.separator();
            ui.menu_button(check_label(false, self.t("menu.all_encodings", "所有編碼")), |ui| {
                egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                    for &e in ALL_ENCODINGS {
                        if ui.button(check_label(e == enc_name, e)).clicked() {
                            chosen = Some(e);
                        }
                    }
                });
            });
            if let Some(e) = chosen {
                ui.close();
                self.reinterpret(e);
            }
        });
        ui.separator();
        let syn = self.doc().syntax.clone();
        ui.menu_button(check_label(false, &self.tf("menu.syntax", "語法類型：{q}", None, Some(&syn))), |ui| {
            egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                for name in syntax::syntax_names() {
                    if ui.button(check_label(name == syn, &name)).clicked() {
                        let doc = self.doc_mut();
                        doc.syntax = name.clone();
                        doc.syntax_cache.clear();
                        ui.close();
                    }
                }
            });
        });
        ui.separator();
        let fam = if s.font_family.is_empty() { self.fonts.editor_family.clone() } else { s.font_family.clone() };
        ui.menu_button(check_label(false, &self.tf("menu.font_name", "字型名稱：{q}", None, Some(&fam))), |ui| {
            type Group = (&'static str, fn(char) -> bool);
            let groups: [Group; 7] = [
                ("[@]", |c| c == '@'),
                ("[ABCDE]", |c| ('A'..='E').contains(&c)),
                ("[FGHIJ]", |c| ('F'..='J').contains(&c)),
                ("[KLMNO]", |c| ('K'..='O').contains(&c)),
                ("[PQRST]", |c| ('P'..='T').contains(&c)),
                ("[UVWXYZ]", |c| ('U'..='Z').contains(&c)),
                (self.t("menu.other", "[其他]"), |c| !c.is_ascii_alphabetic() && c != '@'),
            ];
            for (label, pred) in groups {
                ui.menu_button(label, |ui| {
                    egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                        for f in self.fonts.families.clone() {
                            let first = f.chars().next().unwrap_or(' ').to_ascii_uppercase();
                            if pred(first) && ui.button(check_label(f == fam, &f)).clicked() {
                                self.settings.font_family = f.clone();
                                ui.close();
                            }
                        }
                    });
                });
            }
        });
        let size_label = self.tf("menu.font_size", "字型大小：{q}", None, Some(&s.font_size.to_string()));
        ui.menu_button(check_label(false, &size_label), |ui| {
            egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                for n in 1..=72 {
                    if ui.button(check_label(s.font_size as i32 == n, &n.to_string())).clicked() {
                        self.settings.font_size = n as f32;
                        ui.close();
                    }
                }
            });
        });
        self.item(ui, C::SetFont);
        self.item(ui, C::ZoomIn);
        self.item(ui, C::ZoomOut);
        self.check(ui, C::FixedWidthMode, s.fixed_width);
        ui.separator();
        let tab_label = self.tf("menu.tab_width", "Tab 欄數：{q}", None, Some(&s.tab_size.to_string()));
        ui.menu_button(check_label(false, &tab_label), |ui| {
            for n in 1..=16 {
                if ui.button(check_label(s.tab_size == n, &n.to_string())).clicked() {
                    self.settings.tab_size = n;
                    ui.close();
                }
            }
        });
        let spacing_label = self.tf("menu.line_spacing", "行距：{q}%", None, Some(&s.line_spacing.to_string()));
        ui.menu_button(check_label(false, &spacing_label), |ui| {
            egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                for n in (100..=250).step_by(5) {
                    if ui.button(check_label(s.line_spacing == n, &format!("{n}%"))).clicked() {
                        self.settings.line_spacing = n;
                        ui.close();
                    }
                }
            });
        });
        ui.separator();
        self.check(ui, C::NoWrap, s.wrap_mode == WrapMode::NoWrap);
        self.check(ui, C::WrapByWindow, s.wrap_mode == WrapMode::ByWindow);
        self.check(ui, C::WrapByColumn, s.wrap_mode == WrapMode::ByColumn);
        ui.separator();
        self.check(ui, C::DisplayLineNumber, s.show_line_numbers);
        self.check(ui, C::DisplayBookmark, s.show_bookmarks);
        self.check(ui, C::Display80ColHint, s.show_col_hint);
        self.check(ui, C::ShowRuler, s.show_ruler);
        self.check(ui, C::ShowEndOfLine, s.show_eol);
        self.check(ui, C::ShowTabChar, s.show_tab);
        self.check(ui, C::ShowSpaceChar, s.show_space);
        self.check(ui, C::ShowAllChar, s.show_eol && s.show_tab && s.show_space);
        self.check(ui, C::RightToLeft, s.right_to_left);
        self.check(ui, C::MarkActiveLine, s.mark_active_line);
        self.check(ui, C::MarkBracePair, s.mark_brace_pair);
        self.check(ui, C::SpellCheck, s.spell_check);
        self.check(ui, C::TypewriterMode, s.typewriter_mode);
        ui.separator();
        ui.menu_button(check_label(false, self.t("menu.toolbars", "工具列")), |ui| {
            self.check(ui, C::ToggleToolbar, s.show_toolbar);
            self.check(ui, C::ToggleMenubar, s.show_menubar);
            self.check(ui, C::ToggleStatusbar, s.show_statusbar);
        });
        self.item(ui, C::ToggleTheme);
        ui.separator();
        let mode = self.doc().mode;
        let hex_only = self.doc().hex_only;
        let sc = self.shortcut_text(C::TextMode, ui.ctx());
        if ui.add_enabled(!hex_only, Button::new(check_label(mode == EditMode::Text, &self.cmd_text(C::TextMode))).shortcut_text(sc)).clicked() {
            self.pending.push(C::TextMode);
            ui.close();
        }
        let sc = self.shortcut_text(C::ColumnMode, ui.ctx());
        if ui.add_enabled(!hex_only, Button::new(check_label(mode == EditMode::Column, &self.cmd_text(C::ColumnMode))).shortcut_text(sc)).clicked() {
            self.pending.push(C::ColumnMode);
            ui.close();
        }
        self.check(ui, C::HexMode, mode == EditMode::Hex);
        ui.separator();
        ui.menu_button(check_label(false, self.t("menu.preview_as", "預覽為")), |ui| {
            let is_md = self.doc().is_markdown();
            self.item_enabled(ui, C::PreviewMarkdown, is_md);
            self.item(ui, C::PreviewHtml);
        });
    }

    fn macro_menu(&mut self, ui: &mut egui::Ui) {
        let rec = self.macros.recording;
        let has = !self.macros.steps.is_empty();
        self.item(ui, C::RunTempMacro);
        self.item(ui, C::LoadMacro);
        ui.separator();
        self.item_enabled(ui, C::StartRecMacro, !rec);
        self.item_enabled(ui, C::StopRecMacro, rec);
        self.item_enabled(ui, C::PlayRecMacro, !rec && has);
        self.item_enabled(ui, C::SaveRecMacro, !rec && has);
        ui.separator();
        ui.menu_button(check_label(false, self.t("menu.local_macros", "本機巨集清單")), |ui| {
            self.item(ui, C::EditMacroFile);
            ui.separator();
            let list = super::macros::list_macros();
            if list.is_empty() {
                ui.label(self.t("menu.no_macros", "（尚無已儲存的巨集）"));
            }
            for p in list {
                let name = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                if ui.button(name).clicked() {
                    self.load_macro_file(&p);
                    ui.close();
                }
            }
        });
    }

    fn tools_menu(&mut self, ui: &mut egui::Ui) {
        self.item(ui, C::Options);
        self.item(ui, C::Highlighting);
        self.item(ui, C::FileAssociation);
        self.item(ui, C::SourceFormatter);
        self.item(ui, C::PurgeHistories);
        ui.separator();
        let bom = self.doc().has_bom;
        let can_bom = encoding::supports_bom(self.doc().encoding);
        let bom_state = if bom { self.t("menu.bom_yes", "有") } else { self.t("menu.bom_no", "無") };
        ui.menu_button(check_label(false, &self.tf("menu.bom", "Unicode BOM：{q}", None, Some(bom_state))), |ui| {
            self.item_enabled(ui, C::ToggleBOM, can_bom);
        });
        ui.separator();
        let nl = self.doc().newline;
        let nl_name = match nl {
            NewLineFormat::Dos => "CRLF",
            NewLineFormat::Unix => "LF",
            NewLineFormat::Mac => "CR",
        };
        ui.menu_button(check_label(false, &self.tf("menu.newline", "換行字元（檔案格式）：{q}", None, Some(nl_name))), |ui| {
            self.check(ui, C::ConvertToDOS, nl == NewLineFormat::Dos);
            self.check(ui, C::ConvertToMac, nl == NewLineFormat::Mac);
            self.check(ui, C::ConvertToUNIX, nl == NewLineFormat::Unix);
        });
        ui.menu_button(check_label(false, &self.tf("menu.newline_enter", "按 Enter 插入的換行字元：{q}", None, Some(nl_name))), |ui| {
            ui.label(self.t("menu.newline_same", "本程式整份文件使用同一種換行字元，此設定與上方的檔案格式相同。"));
            self.check(ui, C::InsertDOS, nl == NewLineFormat::Dos);
            self.check(ui, C::InsertMac, nl == NewLineFormat::Mac);
            self.check(ui, C::InsertUNIX, nl == NewLineFormat::Unix);
        });
        ui.separator();
        self.item(ui, C::ConvertEncoding);
        ui.separator();
        ui.menu_button(check_label(false, self.t("menu.chinese", "簡繁轉換")), |ui| {
            let safe = self.chinese_safe;
            self.check(ui, C::ChineseSafeMode, safe);
            ui.separator();
            for c in [C::Simp2TradChinese, C::Trad2SimpChinese, C::Kanji2TradChinese, C::Kanji2SimpChinese, C::Chinese2Kanji] {
                self.item(ui, c);
            }
            ui.separator();
            for c in [C::Simp2TradClipboard, C::Trad2SimpClipboard, C::Kanji2TradClipboard, C::Kanji2SimpClipboard, C::Chinese2KanjiClipboard] {
                self.item(ui, c);
            }
        });
        ui.separator();
        ui.menu_button(check_label(false, self.t("menu.convert", "文字轉換／格式化")), |ui| {
            self.item(ui, C::Markdown2Html);
            self.item(ui, C::Html2PlainText);
            ui.separator();
            self.item(ui, C::AstyleFormat);
            self.item(ui, C::XmlFormat);
            self.item(ui, C::JsonFormat);
        });
        ui.separator();
        self.item(ui, C::WordCount);
    }

    fn window_menu(&mut self, ui: &mut egui::Ui) {
        self.item(ui, C::ToggleWindow);
        self.item(ui, C::PreviousWindow);
        self.item(ui, C::NextWindow);
        self.item(ui, C::WindowList);
        ui.separator();
        let mut go = None;
        for (i, t) in self.tabs.iter().enumerate() {
            let name = format!("{} {}{}", i + 1, t.doc.display_name(), if t.doc.is_modified() { " *" } else { "" });
            if ui.button(check_label(i == self.active, &name)).clicked() {
                go = Some(i);
                ui.close();
            }
        }
        if let Some(i) = go {
            self.activate(i);
        }
    }

    /// 以指定編碼重新解碼目前文件
    pub fn reinterpret(&mut self, name: &str) {
        let Some(enc) = encoding::encoding_for_name(name) else { return };
        if self.doc().hex_only {
            self.doc_mut().encoding = enc;
            return;
        }
        if self.doc().is_modified() && self.doc().has_decode_error {
            self.message("無法重新解碼", "文件已修改且原本解碼有誤，無法取得原始位元組；請先重新載入檔案。");
            return;
        }
        let was_hex = self.doc().mode == EditMode::Hex;
        if was_hex {
            self.doc_mut().encoding = enc;
            return;
        }
        match self.doc_mut().reinterpret(enc) {
            Ok(()) => {
                let msg = if self.doc().has_decode_error { format!("以 {name} 解碼時有無法辨識的位元組") } else { format!("已改用 {name} 解碼") };
                self.set_status(msg);
            }
            Err(e) => self.message("無法重新解碼", e),
        }
    }

    pub fn toolbar(&mut self, ui: &mut egui::Ui) {
        let resp = ui
            .horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let mode = self.doc().mode;
                let wrap = self.settings.wrap_mode;
                let groups: &[&[(&str, C, Option<bool>)]] = &[
                    &[("new", C::New, None), ("fileopen", C::Open, None), ("filesave", C::Save, None), ("saveall", C::SaveAll, None), ("fileclose", C::Close, None), ("closeall", C::CloseAll, None)],
                    &[("print", C::Print, None)],
                    &[("undo", C::Undo, None), ("redo", C::Redo, None)],
                    &[("cut", C::Cut, None), ("copy", C::Copy, None), ("paste", C::Paste, None)],
                    &[("indent", C::IncreaseIndent, None), ("unindent", C::DecreaseIndent, None), ("comment", C::Comment, None), ("uncomment", C::Uncomment, None)],
                    &[("find", C::Find, None), ("findnext", C::FindNext, None), ("findprev", C::FindPrevious, None), ("replace", C::Replace, None), ("goposition", C::GoToLine, None)],
                    &[
                        ("nowrap", C::NoWrap, Some(wrap == WrapMode::NoWrap)),
                        ("wrapbywin", C::WrapByWindow, Some(wrap == WrapMode::ByWindow)),
                        ("wrapbycol", C::WrapByColumn, Some(wrap == WrapMode::ByColumn)),
                    ],
                    &[
                        ("textmode", C::TextMode, Some(mode == EditMode::Text)),
                        ("columnmode", C::ColumnMode, Some(mode == EditMode::Column)),
                        ("hexmode", C::HexMode, Some(mode == EditMode::Hex)),
                    ],
                    &[("bookmark_toggle", C::ToggleBookmark, None), ("bookmark_next", C::NextBookmark, None), ("bookmark_prev", C::PreviousBookmark, None), ("bookmark_clear", C::ClearAllBookmarks, None)],
                    &[("preview", C::PreviewMarkdown, None)],
                ];
                let can_undo = self.doc().can_undo();
                let can_redo = self.doc().can_redo();
                let is_md = self.doc().is_markdown();
                for group in groups {
                    for &(icon, c, selected) in *group {
                        let Some(tex) = self.icons.get(icon) else { continue };
                        let img = egui::Image::new((tex.id(), egui::vec2(16.0, 16.0)));
                        let mut btn = Button::image(img).frame_when_inactive(false);
                        if let Some(sel) = selected {
                            btn = btn.selected(sel);
                        }
                        let enabled = match c {
                            C::Undo => can_undo,
                            C::Redo => can_redo,
                            C::PreviewMarkdown => is_md,
                            _ => true,
                        };
                        let sc = self.shortcut_text(c, ui.ctx());
                        let tip = if sc.is_empty() { self.cmd_text(c) } else { format!("{} ({sc})", self.cmd_text(c)) };
                        if ui.add_enabled(enabled, btn).on_hover_text(tip).clicked() {
                            self.pending.push(c);
                        }
                    }
                    ui.separator();
                }
                // 字級與主題
                if ui.button("A−").on_hover_text(self.t("zoom.out", "縮小字型")).clicked() {
                    self.pending.push(C::ZoomOut);
                }
                ui.label(format!("{}", self.settings.font_size));
                if ui.button("A+").on_hover_text(self.t("zoom.in", "放大字型")).clicked() {
                    self.pending.push(C::ZoomIn);
                }
                let dark = App::is_dark(ui.ctx());
                if ui.button(if dark { "☀" } else { "🌙" }).on_hover_text(self.t("zoom.theme", "切換深色/淺色主題")).clicked() {
                    self.pending.push(C::ToggleTheme);
                }
                ui.separator();
                // 快速搜尋框
                let quick = self.t("find.quick", "快速搜尋");
                let resp = ui.add(egui::TextEdit::singleline(&mut self.find.query).desired_width(140.0).hint_text(quick));
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let back = ui.input(|i| i.modifiers.shift);
                    self.find_next(back);
                    resp.request_focus();
                }
            })
            .response;
        resp.context_menu(|ui| {
            let s = self.settings.clone();
            self.check(ui, C::ToggleToolbar, s.show_toolbar);
            self.check(ui, C::ToggleMenubar, s.show_menubar);
            self.check(ui, C::ToggleStatusbar, s.show_statusbar);
        });
    }
}
