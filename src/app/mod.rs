//! egui 主程式：多文件分頁、選單、工具列、狀態列、對話框與快捷鍵派送

mod actions;
pub mod commands;
mod dialogs;
mod find;
mod findfiles;
pub mod fonts;
pub mod hexview;
mod macros;
mod menus;
mod print;
pub mod view;
#[cfg(test)]
mod visual_tests;
#[cfg(test)]
mod ui_tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, Event, KeyboardShortcut, TextureHandle};

use crate::core::document::{Document, OpenOptions, SaveError};
use crate::core::editor;
use crate::core::encoding;
use crate::core::model::{EditMode, NewLineFormat};
use crate::core::settings::{SessionFile, Settings, ThemeChoice};
use crate::core::spell::SpellChecker;

use commands::C;
use dialogs::Dialog;
use find::{FindBar, GotoBar};
use findfiles::FindInFiles;
use fonts::FontManager;
use hexview::HexViewState;
use macros::MacroState;
use view::{CharWidths, ViewEnv, ViewOutput, ViewState};

/// 一個分頁：文件與其檢視狀態
pub struct Tab {
    pub doc: Document,
    pub view: ViewState,
    pub hex: HexViewState,
    /// 磁碟上的檔案已被其他程式修改（只有使用者選擇重新載入或忽略時才清除）
    pub disk_changed: bool,
}

impl Tab {
    fn new(doc: Document) -> Self {
        Self { doc, view: ViewState::default(), hex: HexViewState::default(), disk_changed: false }
    }
}

/// 需要使用者確認的動作
pub enum Confirm {
    /// 關閉有未存檔修改的文件：依序詢問 `ids` 中每份文件；`exit` 為真時全部處理完後結束程式
    Close { ids: Vec<u64>, exit: bool },
    /// 解碼有錯的文件仍要存檔
    SaveLossy { id: u64, path: Option<PathBuf> },
    /// 目前編碼無法表示部分字元：可改以 UTF-8 存檔
    SaveUnmappable { id: u64, path: Option<PathBuf>, message: String },
    /// 磁碟上的檔案在開啟後被其他程式修改，仍要覆寫
    OverwriteExternal { id: u64, path: Option<PathBuf> },
    /// 重新載入會遺失未存檔修改
    ReloadModified { id: u64 },
    /// 一般訊息
    Message { title: String, text: String },
}

pub struct App {
    pub settings: Settings,
    pub tabs: Vec<Tab>,
    pub active: usize,
    prev_active: usize,
    fonts: FontManager,
    font_applied: Option<String>,
    widths: CharWidths,
    icons: HashMap<&'static str, TextureHandle>,
    status: Option<(String, f64)>,
    find: FindBar,
    goto: GotoBar,
    dialog: Option<Dialog>,
    confirm: Option<Confirm>,
    macros: MacroState,
    spell: Option<SpellChecker>,
    spell_error: Option<String>,
    fif: FindInFiles,
    bindings: Vec<(KeyboardShortcut, C)>,
    post_it: bool,
    fullscreen: bool,
    last_autosave: f64,
    last_disk_check: f64,
    allow_close: bool,
    clipboard: Option<arboard::Clipboard>,
    /// 測試用：啟動後截圖存檔並結束
    screenshot: Option<(PathBuf, u32)>,
    pending: Vec<C>,
    /// 下一幀將焦點交給編輯區
    focus_editor: bool,
    /// 簡繁轉換的安全模式（不轉換易誤轉的字）
    chinese_safe: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext, files: Vec<PathBuf>, screenshot: Option<PathBuf>) -> Self {
        let settings = Settings::load();
        let mut app = Self {
            settings,
            tabs: Vec::new(),
            active: 0,
            prev_active: 0,
            fonts: FontManager::new(),
            font_applied: None,
            widths: CharWidths::default(),
            icons: HashMap::new(),
            status: None,
            find: FindBar::default(),
            goto: GotoBar::default(),
            dialog: None,
            confirm: None,
            macros: MacroState::default(),
            spell: None,
            spell_error: None,
            fif: FindInFiles::default(),
            bindings: Vec::new(),
            post_it: false,
            fullscreen: false,
            last_autosave: 0.0,
            last_disk_check: 0.0,
            allow_close: false,
            // 測試時不連接系統剪貼簿，避免結果受使用者剪貼簿內容影響或覆寫它
            clipboard: if cfg!(test) { None } else { arboard::Clipboard::new().ok() },
            screenshot: screenshot.map(|p| (p, 0)),
            pending: Vec::new(),
            focus_editor: true,
            chinese_safe: true,
        };
        app.load_icons(&cc.egui_ctx);
        app.rebuild_bindings();
        app.apply_theme(&cc.egui_ctx);
        egui_extras_style(&cc.egui_ctx);
        // 第一幀繪製前就註冊編輯區字型家族（set_fonts 於下一幀才生效，延後註冊會 panic）
        let fam = app.settings.font_family.clone();
        app.fonts.apply(&cc.egui_ctx, &fam);
        app.font_applied = Some(fam);

        if files.is_empty() && app.settings.reload_files_on_startup {
            let session = app.settings.session_files.clone();
            for f in session {
                if f.path.exists() && app.open_path(&f.path) {
                    let doc = &mut app.tabs[app.active].doc;
                    doc.editor.set_caret(f.line, f.col);
                }
            }
            if !app.tabs.is_empty() {
                app.active = app.settings.session_active.min(app.tabs.len() - 1);
            }
        }
        for f in files {
            app.open_path(&f);
        }
        if app.tabs.is_empty() {
            app.new_document();
        }
        if app.settings.spell_check {
            app.load_spell();
        }
        app
    }

    // ── 文件與分頁 ──────────────────────────────────────────────

    pub fn doc(&self) -> &Document {
        &self.tabs[self.active].doc
    }

    pub fn doc_mut(&mut self) -> &mut Document {
        &mut self.tabs[self.active].doc
    }

    pub fn tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    fn index_of(&self, id: u64) -> Option<usize> {
        self.tabs.iter().position(|t| t.doc.id == id)
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), 0.0));
    }

    pub fn open_options(&self) -> OpenOptions {
        OpenOptions {
            default_encoding: encoding::encoding_for_name(&self.settings.default_encoding).unwrap_or(encoding_rs::BIG5),
            enforce_default_encoding: self.settings.enforce_default_encoding,
            max_text_size: self.settings.max_text_file_mb.max(1) * 1024 * 1024,
        }
    }

    fn activate(&mut self, idx: usize) {
        if idx < self.tabs.len() && idx != self.active {
            self.prev_active = self.active;
            self.active = idx;
            self.focus_editor = true;
        }
    }

    pub fn new_document(&mut self) {
        let s = &self.settings;
        let enc = encoding::encoding_for_name(&s.new_doc_encoding).unwrap_or(encoding_rs::UTF_8);
        let nl = match s.new_doc_newline.as_str() {
            "CRLF" => NewLineFormat::Dos,
            "CR" => NewLineFormat::Mac,
            _ => NewLineFormat::Unix,
        };
        let doc = Document::new_untitled(enc, s.new_doc_bom, nl, &s.new_doc_syntax);
        self.tabs.push(Tab::new(doc));
        let idx = self.tabs.len() - 1;
        self.activate(idx);
        self.active = idx;
    }

    /// 開啟檔案（已開啟則切換到該分頁）；成功回傳 true
    pub fn open_path(&mut self, path: &Path) -> bool {
        let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if let Some(i) = self.tabs.iter().position(|t| t.doc.path.as_deref().is_some_and(|p| p.canonicalize().ok().as_deref() == Some(&canon))) {
            self.activate(i);
            return true;
        }
        match Document::open(&canon, &self.open_options()) {
            Ok(mut doc) => {
                use crate::core::settings::OpenViewMode;
                match self.settings.open_view_mode {
                    OpenViewMode::Text => {
                        let _ = doc.set_mode(EditMode::Text);
                    }
                    OpenViewMode::Column => {
                        let _ = doc.set_mode(EditMode::Column);
                    }
                    OpenViewMode::Hex => {
                        let _ = doc.set_mode(EditMode::Hex);
                    }
                    OpenViewMode::Auto => {}
                }
                if self.settings.restore_caret_position
                    && let Some(&(l, c)) = self.settings.caret_positions.get(&canon.to_string_lossy().to_string())
                {
                    doc.editor.set_caret(l, c);
                }
                let mut notes = Vec::new();
                if doc.hex_only {
                    notes.push(format!("檔案大於 {} MB，以十六進位模式開啟", self.settings.max_text_file_mb));
                }
                if doc.has_decode_error {
                    notes.push(format!("無法以 {} 正確解碼，請以「檢視 → 編碼」重新解碼", doc.encoding.name()));
                }
                if doc.mixed_newlines {
                    notes.push("檔案混用多種換行字元，存檔時會統一".to_string());
                }
                // 只剩一個未修改的空白未命名文件時直接取代
                let replace_blank = self.tabs.len() == 1
                    && self.tabs[0].doc.path.is_none()
                    && !self.tabs[0].doc.is_modified()
                    && self.tabs[0].doc.editor.lines.len() == 1
                    && self.tabs[0].doc.editor.lines[0].is_empty();
                if replace_blank {
                    self.tabs[0] = Tab::new(doc);
                    self.active = 0;
                } else {
                    self.tabs.push(Tab::new(doc));
                    let idx = self.tabs.len() - 1;
                    self.activate(idx);
                }
                self.settings.add_recent_file(canon);
                self.set_status(if notes.is_empty() { format!("已開啟 {}", path.display()) } else { notes.join("；") });
                true
            }
            Err(e) => {
                self.confirm = Some(Confirm::Message { title: "開啟失敗".into(), text: format!("{}\n{e}", path.display()) });
                false
            }
        }
    }

    pub fn open_dialog(&mut self) {
        let mut dlg = rfd::FileDialog::new().set_title("開啟舊檔");
        if let Some(dir) = self.doc().path.as_ref().and_then(|p| p.parent()) {
            dlg = dlg.set_directory(dir);
        }
        if let Some(files) = dlg.pick_files() {
            for f in files {
                self.open_path(&f);
            }
        }
    }

    /// 存檔（必要時詢問路徑或確認）；`as_new` 為真時一律詢問路徑
    pub fn save_tab(&mut self, idx: usize, as_new: bool) -> bool {
        let id = self.tabs[idx].doc.id;
        let path = if as_new || self.tabs[idx].doc.path.is_none() {
            let name = self.tabs[idx].doc.display_name();
            let mut dlg = rfd::FileDialog::new().set_title("另存新檔").set_file_name(&name);
            if let Some(dir) = self.tabs[idx].doc.path.as_ref().and_then(|p| p.parent()) {
                dlg = dlg.set_directory(dir);
            }
            match dlg.save_file() {
                Some(p) => Some(p),
                None => return false,
            }
        } else {
            None
        };
        // 存檔前檢查檔案是否在開啟後被外部修改（P3-4）
        if path.is_none() && self.tabs[idx].doc.changed_on_disk() {
            self.confirm = Some(Confirm::OverwriteExternal { id, path });
            return false;
        }
        self.save_with(id, path, false)
    }

    /// 實際存檔並處理錯誤
    fn save_with(&mut self, id: u64, path: Option<PathBuf>, allow_lossy: bool) -> bool {
        let Some(idx) = self.index_of(id) else { return false };
        let target = path.clone().or_else(|| self.tabs[idx].doc.path.clone());
        if self.settings.auto_backup
            && let Some(t) = &target
            && t.exists()
        {
            let mut bak = t.as_os_str().to_owned();
            bak.push(".bak");
            let _ = std::fs::copy(t, PathBuf::from(bak));
        }
        let result = self.tabs[idx].doc.save(path.as_deref(), allow_lossy);
        match result {
            Ok(()) => {
                self.tabs[idx].disk_changed = false;
                if let Some(p) = self.tabs[idx].doc.path.clone() {
                    self.settings.add_recent_file(p.clone());
                    self.set_status(format!("已儲存 {}", p.display()));
                }
                true
            }
            Err(SaveError::DecodeErrors) => {
                self.confirm = Some(Confirm::SaveLossy { id, path });
                false
            }
            Err(SaveError::Unmappable(u)) => {
                let message = SaveError::Unmappable(u).to_string();
                self.confirm = Some(Confirm::SaveUnmappable { id, path, message });
                false
            }
            Err(e) => {
                self.confirm = Some(Confirm::Message { title: "存檔失敗".into(), text: e.to_string() });
                false
            }
        }
    }

    /// 要求關閉分頁：有未存檔修改的先詢問
    pub fn request_close(&mut self, ids: Vec<u64>, exit: bool) {
        let mut clean = Vec::new();
        let mut dirty = Vec::new();
        for id in ids {
            match self.index_of(id) {
                Some(i) if self.tabs[i].doc.is_modified() => dirty.push(id),
                Some(_) => clean.push(id),
                None => {}
            }
        }
        for id in clean {
            self.close_now(id);
        }
        if dirty.is_empty() {
            if exit {
                self.allow_close = true;
            }
        } else {
            self.confirm = Some(Confirm::Close { ids: dirty, exit });
        }
    }

    fn close_now(&mut self, id: u64) {
        let Some(i) = self.index_of(id) else { return };
        self.remember_caret(i);
        self.tabs.remove(i);
        if self.tabs.is_empty() {
            self.new_document();
            self.active = 0;
        } else {
            if self.active >= self.tabs.len() || self.active > i {
                self.active = self.active.saturating_sub(1).min(self.tabs.len() - 1);
            }
            self.prev_active = self.prev_active.min(self.tabs.len() - 1);
        }
        self.focus_editor = true;
    }

    fn remember_caret(&mut self, i: usize) {
        if let Some(p) = self.tabs[i].doc.path.clone() {
            let h = self.tabs[i].doc.editor.selection.head;
            self.settings.remember_caret(&p, h.line, h.col);
        }
    }

    fn reload_tab(&mut self, id: u64) {
        let Some(i) = self.index_of(id) else { return };
        let opts = self.open_options();
        match self.tabs[i].doc.reload(&opts) {
            Ok(()) => {
                self.tabs[i].disk_changed = false;
                self.tabs[i].view.ensure_visible = true;
                self.set_status("已重新載入");
            }
            Err(e) => self.confirm = Some(Confirm::Message { title: "重新載入失敗".into(), text: e.to_string() }),
        }
    }

    // ── 設定、主題、字型、快捷鍵 ────────────────────────────────

    pub fn rebuild_bindings(&mut self) {
        let mut b = Vec::new();
        for &c in commands::ALL {
            let keys: Vec<String> = match self.settings.key_bindings.get(c.id()) {
                Some(list) => list.clone(),
                None => c.default_keys().split('|').filter(|k| !k.is_empty()).map(str::to_string).collect(),
            };
            for k in keys {
                // macOS 的 ⌘Q 為系統結束程式，不指派給「不自動換行」
                if cfg!(target_os = "macos") && c == C::NoWrap && k == "Ctrl+Q" {
                    continue;
                }
                if let Some(sc) = commands::parse_shortcut(&k) {
                    b.push((sc, c));
                }
            }
        }
        self.bindings = b;
    }

    pub fn shortcut_text(&self, c: C, ctx: &egui::Context) -> String {
        self.bindings.iter().find(|(_, cc)| *cc == c).map(|(sc, _)| ctx.format_shortcut(sc)).unwrap_or_default()
    }

    pub fn apply_theme(&mut self, ctx: &egui::Context) {
        ctx.set_theme(match self.settings.theme {
            ThemeChoice::System => egui::ThemePreference::System,
            ThemeChoice::Light => egui::ThemePreference::Light,
            ThemeChoice::Dark => egui::ThemePreference::Dark,
        });
    }

    pub fn is_dark(ctx: &egui::Context) -> bool {
        ctx.global_style().visuals.dark_mode
    }

    pub fn syntax_theme(&self, dark: bool) -> String {
        if dark { self.settings.syntax_theme_dark.clone() } else { self.settings.syntax_theme_light.clone() }
    }

    pub fn load_spell(&mut self) {
        match SpellChecker::load(&self.settings.spell_language, &self.settings.personal_dictionary) {
            Ok(s) => {
                self.spell = Some(s);
                self.spell_error = None;
            }
            Err(e) => {
                self.spell = None;
                self.spell_error = Some(e.clone());
                self.set_status(e);
            }
        }
    }

    fn load_icons(&mut self, ctx: &egui::Context) {
        macro_rules! icons {
            ($($name:literal),*) => {
                $(
                    let bytes: &[u8] = include_bytes!(concat!("../../assets/icons/", $name, ".png"));
                    if let Ok(img) = image::load_from_memory(bytes) {
                        let rgba = img.to_rgba8();
                        let size = [rgba.width() as usize, rgba.height() as usize];
                        let ci = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                        self.icons.insert($name, ctx.load_texture($name, ci, egui::TextureOptions::NEAREST));
                    }
                )*
            };
        }
        icons!(
            "new", "fileopen", "filesave", "saveall", "fileclose", "closeall", "print", "undo", "redo", "cut", "copy",
            "paste", "indent", "unindent", "comment", "uncomment", "find", "findnext", "findprev", "replace",
            "goposition", "nowrap", "wrapbywin", "wrapbycol", "textmode", "columnmode", "hexmode", "bookmark_toggle",
            "bookmark_next", "bookmark_prev", "bookmark_clear", "preview", "encoding", "syntax", "reload", "quit",
            "help", "down"
        );
    }

    // ── 快捷鍵 ──────────────────────────────────────────────────

    /// 只在文字欄位有焦點時也允許的命令（不會與文字欄位自身的編輯快捷鍵衝突）
    fn allowed_in_text_field(c: C) -> bool {
        matches!(
            c,
            C::New | C::Open | C::Save | C::SaveAs | C::SaveAll | C::Close | C::Find | C::FindNext | C::FindPrevious
                | C::Replace | C::QuickFind | C::FindInFiles | C::GoToLine | C::GoToPosition | C::NextWindow
                | C::PreviousWindow | C::ToggleWindow | C::FullScreen | C::Print | C::RecentFilesList | C::SwitchTab1
                | C::SwitchTab2 | C::SwitchTab3 | C::SwitchTab4 | C::SwitchTab5 | C::SwitchTab6 | C::SwitchTab7
                | C::SwitchTab8 | C::SwitchLastTab
        )
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let focused = ctx.memory(|m| m.focused());
        let editor_focus = focused.is_none() || self.editor_has_focus(ctx);
        let mut fired: Vec<C> = Vec::new();
        let mut alt_fired = false;
        let mut pasted: Option<String> = None;
        let bindings = self.bindings.clone();
        let capturing = matches!(self.dialog, Some(Dialog::Options(ref o)) if o.capturing.is_some());
        if capturing || self.confirm.is_some() {
            return;
        }
        ctx.input_mut(|i| {
            i.events.retain(|ev| match ev {
                Event::Key { key, pressed: true, modifiers, .. } => {
                    let hit = bindings
                        .iter()
                        .find(|(sc, _)| sc.logical_key == *key && commands::modifiers_match(*modifiers, sc.modifiers));
                    match hit {
                        Some((sc, c)) if editor_focus || Self::allowed_in_text_field(*c) => {
                            fired.push(*c);
                            alt_fired |= sc.modifiers.alt;
                            false
                        }
                        _ => true,
                    }
                }
                Event::Copy if editor_focus => {
                    fired.push(C::Copy);
                    false
                }
                Event::Cut if editor_focus => {
                    fired.push(C::Cut);
                    false
                }
                Event::Paste(s) if editor_focus => {
                    pasted = Some(s.clone());
                    fired.push(C::Paste);
                    false
                }
                _ => true,
            });
            if alt_fired {
                // macOS 上 Option+按鍵會另外產生特殊字元的文字事件，觸發快捷鍵時一併丟棄
                i.events.retain(|ev| !matches!(ev, Event::Text(_)));
            }
        });
        if let Some(s) = pasted {
            self.set_editor_clipboard(&s);
        }
        for c in fired {
            self.run(ctx, c);
        }
    }

    fn editor_has_focus(&self, ctx: &egui::Context) -> bool {
        let Some(f) = ctx.memory(|m| m.focused()) else { return false };
        let doc_id = self.doc().id;
        f == view::editor_id(doc_id) || f == hexview::hex_id(doc_id)
    }

    // ── 剪貼簿 ──────────────────────────────────────────────────

    pub fn copy_to_system(&mut self, text: &str) {
        if let Some(cb) = self.clipboard.as_mut() {
            let _ = cb.set_text(text.to_string());
        }
    }

    pub fn system_clipboard(&mut self) -> Option<String> {
        self.clipboard.as_mut().and_then(|cb| cb.get_text().ok())
    }

    /// 以系統剪貼簿內容更新編輯器剪貼簿（內容相同時保留列塊旗標）
    fn set_editor_clipboard(&mut self, text: &str) {
        let ed = &mut self.doc_mut().editor;
        let normalized = text.replace("\r\n", "\n");
        if ed.clipboard != normalized {
            ed.clipboard = normalized;
            ed.clipboard_is_block = false;
        }
    }

    fn sync_clipboard_from_system(&mut self) {
        if let Some(t) = self.system_clipboard() {
            self.set_editor_clipboard(&t);
        }
    }

    // ── 定時工作 ────────────────────────────────────────────────

    fn periodic(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if now - self.last_disk_check > 1.5 {
            self.last_disk_check = now;
            for t in &mut self.tabs {
                if !t.disk_changed && t.doc.changed_on_disk() {
                    t.disk_changed = true;
                }
            }
        }
        if self.settings.auto_save && now - self.last_autosave > self.settings.auto_save_minutes.max(1) as f64 * 60.0 {
            self.last_autosave = now;
            let ids: Vec<u64> = self.tabs.iter().filter(|t| t.doc.path.is_some() && t.doc.is_modified() && !t.disk_changed).map(|t| t.doc.id).collect();
            for id in ids {
                self.save_with(id, None, false);
            }
        }
        if let Some((_, t)) = &mut self.status {
            if *t == 0.0 {
                *t = now;
            } else if now - *t > 8.0 {
                self.status = None;
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(1500));
    }

    fn save_session(&mut self) {
        for i in 0..self.tabs.len() {
            self.remember_caret(i);
        }
        self.settings.session_files = self
            .tabs
            .iter()
            .filter_map(|t| {
                let p = t.doc.path.clone()?;
                let h = t.doc.editor.selection.head;
                Some(SessionFile { path: p, line: h.line, col: h.col })
            })
            .collect();
        self.settings.session_active = self.active;
        if self.settings.purge_history_on_exit {
            self.settings.purge_history();
        }
        if let Err(e) = self.settings.save() {
            eprintln!("無法儲存設定：{e}");
        }
    }

    // ── 介面元件 ────────────────────────────────────────────────

    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        let mut activate = None;
        let mut close = None;
        let mut menu_cmd: Option<(usize, C)> = None;
        egui::ScrollArea::horizontal().id_salt("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (i, t) in self.tabs.iter().enumerate() {
                    let mut name = t.doc.display_name();
                    if t.doc.is_modified() {
                        name.push_str(" *");
                    }
                    if t.doc.read_only {
                        name.push_str(" 🔒");
                    }
                    let selected = i == self.active;
                    let resp = ui.add(egui::Button::selectable(selected, name)).on_hover_text(
                        t.doc.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "尚未儲存".into()),
                    );
                    if resp.clicked() {
                        activate = Some(i);
                    }
                    if resp.middle_clicked() {
                        close = Some(t.doc.id);
                    }
                    resp.context_menu(|ui| {
                        for c in [C::Save, C::SaveAs, C::Reload, C::Close, C::CloseOthers, C::CloseLeft, C::CloseRight, C::CloseAll] {
                            if ui.button(c.menu_text()).clicked() {
                                menu_cmd = Some((i, c));
                                ui.close();
                            }
                        }
                        if let Some(p) = &t.doc.path
                            && ui.button("複製完整路徑").clicked()
                        {
                            ui.ctx().copy_text(p.display().to_string());
                            ui.close();
                        }
                    });
                    if ui.add(egui::Button::new("×").small().frame(false)).on_hover_text("關閉").clicked() {
                        close = Some(t.doc.id);
                    }
                    ui.separator();
                }
            });
        });
        if let Some(i) = activate {
            self.activate(i);
        }
        if let Some(id) = close {
            self.request_close(vec![id], false);
        }
        if let Some((i, c)) = menu_cmd {
            self.activate(i);
            self.active = i;
            self.pending.push(c);
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let doc = self.doc();
        ui.horizontal(|ui| {
            let ed = &doc.editor;
            if doc.mode == EditMode::Hex {
                if let Some(hb) = &doc.hex {
                    ui.monospace(format!("位移 {:X}h / {:X}h", hb.caret, hb.len()));
                    ui.separator();
                    let sel = hb.selection().map(|(s, e)| e - s).unwrap_or(0);
                    ui.monospace(format!("選取 {sel}"));
                }
            } else {
                let h = ed.selection.head;
                let vcol = editor::char_col_to_visual_col(&ed.lines[h.line], h.col);
                ui.monospace(format!("行 {}/{}  欄 {}", h.line + 1, ed.lines.len(), vcol + 1));
                ui.separator();
                ui.monospace(format!("字元 {}", ed.char_pos(h)));
                ui.separator();
                if ed.selection.is_block() {
                    let b = ed.selection.visual_bounds(ed);
                    ui.monospace(format!("列塊 {}×{}", b.max_line - b.min_line + 1, b.max_vcol - b.min_vcol));
                } else {
                    ui.monospace(format!("選取 {}", ed.selection_char_count()));
                }
            }
            ui.separator();
            let bom = if doc.has_bom { ".BOM" } else { "" };
            let nl = match doc.newline {
                NewLineFormat::Dos => "CRLF",
                NewLineFormat::Unix => "LF",
                NewLineFormat::Mac => "CR",
            };
            ui.monospace(format!("{}{} {}{}", doc.encoding.name(), bom, nl, if doc.mixed_newlines { "(混合)" } else { "" }));
            ui.separator();
            ui.monospace(&doc.syntax);
            ui.separator();
            ui.monospace(match doc.mode {
                EditMode::Text => "文字",
                EditMode::Column => "列塊",
                EditMode::Hex => "十六進位",
            });
            ui.separator();
            ui.monospace(if doc.read_only { "唯讀" } else { "可寫" });
            ui.separator();
            let ovr = if doc.mode == EditMode::Hex { doc.hex.as_ref().is_some_and(|h| h.overwrite) } else { ed.overwrite_mode };
            ui.monospace(if ovr { "OVR" } else { "INS" });
            if doc.selecting_mode {
                ui.separator();
                ui.colored_label(Color32::from_rgb(200, 120, 0), "選取中");
            }
            if self.macros.recording {
                ui.separator();
                ui.colored_label(Color32::RED, "● 錄製巨集");
            }
            if let Some((msg, _)) = &self.status {
                ui.separator();
                ui.label(msg);
            }
        });
    }

    /// 外部修改通知橫幅（P1-10：只在使用者選擇重新載入或忽略時消失）
    fn disk_banner(&mut self, ui: &mut egui::Ui) {
        let t = &self.tabs[self.active];
        if !t.disk_changed {
            return;
        }
        let id = t.doc.id;
        let modified = t.doc.is_modified();
        let exists = t.doc.path.as_ref().is_some_and(|p| p.exists());
        egui::Frame::new().fill(Color32::from_rgb(255, 236, 170)).inner_margin(6.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.colored_label(Color32::BLACK, if exists { "⚠ 檔案已被其他程式修改。" } else { "⚠ 檔案已被其他程式刪除或移動。" });
                if modified {
                    ui.colored_label(Color32::from_rgb(160, 0, 0), "重新載入會遺失目前未存檔的修改。");
                }
                if exists && ui.button("重新載入").clicked() {
                    if modified {
                        self.confirm = Some(Confirm::ReloadModified { id });
                    } else {
                        self.reload_tab(id);
                    }
                }
                if ui.button("忽略").clicked() {
                    let t = &mut self.tabs[self.active];
                    t.doc.ignore_disk_change();
                    t.disk_changed = false;
                }
            });
        });
    }

    fn editor_area(&mut self, ui: &mut egui::Ui) {
        let dark = Self::is_dark(ui.ctx());
        let theme = self.syntax_theme(dark);
        editor::set_tab_size(self.settings.tab_size);
        let idx = self.active;
        let mut out = ViewOutput::default();
        let tab = &mut self.tabs[idx];
        let family = self.fonts.editor_family.clone();
        let focus_now = std::mem::take(&mut self.focus_editor);
        let child = ui;
        if tab.doc.mode == EditMode::Hex {
            let focused = hexview::show(child, &mut tab.doc, &mut tab.hex, &self.settings, &mut self.widths, &family, dark);
            out.focused = focused;
            if focus_now {
                child.memory_mut(|m| m.request_focus(hexview::hex_id(tab.doc.id)));
            }
        } else {
            let mut env = ViewEnv {
                settings: &self.settings,
                widths: &mut self.widths,
                spell: self.spell.as_ref(),
                theme_name: &theme,
                is_dark: dark,
                editor_family: &family,
            };
            view::show(child, &mut tab.doc, &mut tab.view, &mut env, &mut out);
            if focus_now {
                child.memory_mut(|m| m.request_focus(view::editor_id(tab.doc.id)));
            }
        }

        if self.macros.recording {
            for a in &out.actions {
                self.macros.record_action(a.clone());
            }
        }
        if let Some(t) = out.copy {
            self.copy_to_system(&t);
        }
        if out.paste {
            self.sync_clipboard_from_system();
            let fill = self.settings.auto_fill_column_paste;
            let doc = self.doc_mut();
            let hint = doc.to_end_hint();
            doc.edit(crate::core::document::EditKind::Other, hint, |ed| ed.paste_from_clipboard(fill));
        }
        if let Some(w) = out.add_word {
            if !self.settings.personal_dictionary.contains(&w) {
                self.settings.personal_dictionary.push(w.clone());
            }
            if let Some(s) = self.spell.as_mut() {
                s.add_word(&w);
            }
        }
        self.pending.extend(out.commands);
    }
}

/// 介面細節：較緊湊的間距
fn egui_extras_style(ctx: &egui::Context) {
    ctx.global_style_mut(|s| {
        s.spacing.button_padding = egui::vec2(5.0, 2.0);
        s.spacing.item_spacing = egui::vec2(6.0, 3.0);
    });
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // 字型：設定改變時重建
        if self.font_applied.as_deref() != Some(self.settings.font_family.as_str()) {
            let fam = self.settings.font_family.clone();
            self.fonts.apply(&ctx, &fam);
            self.font_applied = Some(fam);
        }

        // 拖放開檔
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        for p in dropped {
            self.open_path(&p);
        }

        // 關閉視窗：有未存檔修改時先詢問
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            let ids = self.tabs.iter().map(|t| t.doc.id).collect();
            self.request_close(ids, true);
        }
        if self.allow_close {
            self.save_session();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // 記錄視窗尺寸（全螢幕時不記錄），下次啟動沿用
        if !self.fullscreen
            && let Some(r) = ctx.input(|i| i.viewport().inner_rect)
        {
            self.settings.window_size = Some((r.width(), r.height()));
        }

        // Ctrl/⌘ + 滾輪縮放字型
        let zoom = ctx.input(|i| if i.modifiers.command { i.smooth_scroll_delta().y } else { 0.0 });
        if zoom != 0.0 && ui.rect_contains_pointer(ui.max_rect()) {
            self.settings.font_size = (self.settings.font_size + zoom.signum()).clamp(6.0, 72.0);
        }

        self.handle_shortcuts(&ctx);
        self.poll_macro(&ctx);
        let pending = std::mem::take(&mut self.pending);
        for c in pending {
            self.run(&ctx, c);
        }
        self.periodic(&ctx);

        let show_bars = !self.post_it;
        if self.settings.show_menubar && show_bars {
            egui::Panel::top("menubar").show(ui, |ui| self.menu_bar(ui));
        }
        if self.settings.show_toolbar && show_bars {
            egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        }
        if show_bars {
            egui::Panel::top("tabs").show(ui, |ui| self.tab_bar(ui));
        }
        if self.settings.show_statusbar && show_bars {
            egui::Panel::bottom("statusbar").show(ui, |ui| self.status_bar(ui));
        }
        if self.fif.show_results {
            egui::Panel::bottom("fif-results").resizable(true).default_size(180.0).show(ui, |ui| self.find_results_panel(ui));
        }
        egui::CentralPanel::default().frame(egui::Frame::new()).show(ui, |ui| {
            self.disk_banner(ui);
            self.find_bar(ui);
            self.goto_bar(ui);
            self.editor_area(ui);
        });

        self.show_dialogs(&ctx);
        self.show_confirm(&ctx);
        self.fif.poll(&ctx);

        // 測試用截圖
        if let Some((path, frames)) = self.screenshot.as_mut() {
            *frames += 1;
            // 視窗尚未可見時截圖請求會被丟棄，因此每 30 幀重試一次
            if *frames % 30 == 8 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            let shot = ctx.input(|i| {
                i.raw.events.iter().find_map(|e| if let Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None })
            });
            if let Some(img) = shot {
                let [w, h] = img.size;
                let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, raw)
                    && let Err(e) = buf.save(path.as_path()) {
                        eprintln!("無法儲存截圖：{e}");
                    }
                self.allow_close = true;
            }
            ctx.request_repaint();
        }
    }

    fn on_exit(&mut self) {
        self.save_session();
    }
}
