//! 使用者設定（對應原版「選項」對話框與 MadEdit.cfg），以 JSON 存於系統設定目錄

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 自動換行模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WrapMode {
    #[default]
    NoWrap,
    ByWindow,
    ByColumn,
}

/// 開檔時的檢視模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OpenViewMode {
    /// 依檔案內容自動判斷（二進位檔以十六進位開啟）
    #[default]
    Auto,
    Text,
    Column,
    Hex,
}

/// 深淺色主題
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

/// 工作階段中的一個檔案（啟動時重新開啟）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFile {
    pub path: PathBuf,
    pub line: usize,
    pub col: usize,
}

/// 排序選項
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct SortOptions {
    pub descending: bool,
    pub case_sensitive: bool,
    pub numeric: bool,
    pub remove_duplicates: bool,
}


/// 全形／半形轉換選項
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WidthOptions {
    pub letters: bool,
    pub digits: bool,
    pub symbols: bool,
    pub space: bool,
    pub katakana: bool,
}

impl Default for WidthOptions {
    fn default() -> Self {
        Self {
            letters: true,
            digits: true,
            symbols: true,
            space: true,
            katakana: true,
        }
    }
}

/// 插入遞增數字選項
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NumberOptions {
    pub initial: i64,
    pub step: i64,
    pub total: usize,
    /// 0 十進位、1 十六進位、2 八進位、3 二進位
    pub format: u8,
    pub uppercase_hex: bool,
    pub align_left: bool,
    pub zero_pad: bool,
    pub prefix: String,
    pub postfix: String,
}

impl Default for NumberOptions {
    fn default() -> Self {
        Self {
            initial: 1,
            step: 1,
            total: 10,
            format: 0,
            uppercase_hex: true,
            align_left: false,
            zero_pad: false,
            prefix: String::new(),
            postfix: String::new(),
        }
    }
}

/// 列印（輸出 HTML 後交給瀏覽器列印）選項
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrintOptions {
    pub syntax_colors: bool,
    pub line_numbers: bool,
    pub bookmarks: bool,
    pub end_of_line: bool,
    pub tab_chars: bool,
    pub space_chars: bool,
    pub header: bool,
    pub header_left: String,
    pub header_center: String,
    pub header_right: String,
    pub footer: bool,
    pub footer_left: String,
    pub footer_center: String,
    pub footer_right: String,
    /// 十六進位模式列印位移標頭：0 無、1 僅第一頁、2 每頁
    pub hex_offset_header: u8,
}

impl Default for PrintOptions {
    fn default() -> Self {
        Self {
            syntax_colors: true,
            line_numbers: true,
            bookmarks: false,
            end_of_line: false,
            tab_chars: false,
            space_chars: false,
            header: true,
            header_left: "%f".into(),
            header_center: String::new(),
            header_right: "%n/%s".into(),
            footer: true,
            footer_left: String::new(),
            footer_center: "%d %t".into(),
            footer_right: String::new(),
            hex_offset_header: 1,
        }
    }
}

/// 在多個檔案中尋找的設定
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FindInFilesOptions {
    pub directory: String,
    pub file_filter: String,
    pub exclude_filter: String,
    pub subdirectories: bool,
    /// 只在已開啟的檔案中搜尋
    pub opened_files_only: bool,
}

impl Default for FindInFilesOptions {
    fn default() -> Self {
        Self {
            directory: String::new(),
            file_filter: "*".into(),
            exclude_filter: String::new(),
            subdirectories: true,
            opened_files_only: false,
        }
    }
}

/// 全部設定
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    // ── 檢視 ──
    /// 編輯區字型（空字串表示自動挑選）
    pub font_family: String,
    pub font_size: f32,
    /// 等寬模式：每個字元依固定格子寬度排列（關閉時依字型實際字寬排列）
    pub fixed_width: bool,
    pub tab_size: usize,
    pub indent_size: usize,
    /// 行距百分比（100–250）
    pub line_spacing: u32,
    pub wrap_mode: WrapMode,
    pub wrap_column: usize,
    pub show_line_numbers: bool,
    pub show_bookmarks: bool,
    pub show_col_hint: bool,
    pub col_hint_position: usize,
    pub show_ruler: bool,
    pub show_eol: bool,
    pub show_tab: bool,
    pub show_space: bool,
    pub mark_active_line: bool,
    pub mark_brace_pair: bool,
    pub right_to_left: bool,
    pub typewriter_mode: bool,
    pub spell_check: bool,
    pub spell_language: String,
    pub theme: ThemeChoice,
    pub syntax_theme_light: String,
    pub syntax_theme_dark: String,
    pub show_toolbar: bool,
    pub show_menubar: bool,
    pub show_statusbar: bool,
    pub always_on_top: bool,

    // ── 一般 ──
    pub default_encoding: String,
    pub enforce_default_encoding: bool,
    /// 超過此大小（MB）的檔案只以十六進位模式開啟
    pub max_text_file_mb: u64,
    pub open_view_mode: OpenViewMode,
    pub max_search_result_chars: usize,
    pub reload_files_on_startup: bool,
    pub restore_caret_position: bool,
    pub auto_save: bool,
    pub auto_save_minutes: u32,
    pub auto_backup: bool,
    pub purge_history_on_exit: bool,

    // ── 編輯 ──
    pub date_time_format: String,
    pub insert_spaces_for_tab: bool,
    pub auto_indent: bool,
    pub auto_complete_pair: bool,
    /// 自動補全成對字元時，若有選取則以成對字元包住選取內容（否則取代選取）
    pub wrap_selection_with_pair: bool,
    pub auto_copy_mouse_selection: bool,
    pub middle_click_paste: bool,
    pub auto_fill_column_paste: bool,
    pub double_click_highlight: bool,

    // ── 新文件 ──
    pub new_doc_encoding: String,
    pub new_doc_bom: bool,
    /// "CRLF" / "LF" / "CR"
    pub new_doc_newline: String,
    pub new_doc_syntax: String,

    // ── 其他 ──
    pub print: PrintOptions,
    pub sort: SortOptions,
    pub width: WidthOptions,
    pub numbers: NumberOptions,
    pub find_in_files: FindInFilesOptions,
    /// 快捷鍵覆寫：命令識別碼 → 按鍵清單（如 "Ctrl+Shift+K"）
    pub key_bindings: HashMap<String, Vec<String>>,
    pub astyle_options: String,
    pub search_match_case: bool,
    pub search_whole_word: bool,
    pub search_regex: bool,
    pub search_wrap_around: bool,
    pub search_in_selection: bool,

    // ── 歷史 ──
    pub recent_files: Vec<PathBuf>,
    pub recent_finds: Vec<String>,
    pub recent_replaces: Vec<String>,
    pub recent_dirs: Vec<String>,
    pub session_files: Vec<SessionFile>,
    pub session_active: usize,
    /// 各檔案上次的游標位置
    pub caret_positions: HashMap<String, (usize, usize)>,
    pub window_size: Option<(f32, f32)>,
    pub personal_dictionary: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_family: String::new(),
            font_size: 14.0,
            fixed_width: true,
            tab_size: 4,
            indent_size: 4,
            line_spacing: 100,
            wrap_mode: WrapMode::NoWrap,
            wrap_column: 80,
            show_line_numbers: true,
            show_bookmarks: true,
            show_col_hint: false,
            col_hint_position: 80,
            show_ruler: true,
            show_eol: false,
            show_tab: false,
            show_space: false,
            mark_active_line: true,
            mark_brace_pair: true,
            right_to_left: false,
            typewriter_mode: false,
            spell_check: false,
            spell_language: "en_US".into(),
            theme: ThemeChoice::System,
            syntax_theme_light: "InspiredGitHub".into(),
            syntax_theme_dark: "base16-ocean.dark".into(),
            show_toolbar: true,
            show_menubar: true,
            show_statusbar: true,
            always_on_top: false,

            default_encoding: "Big5".into(),
            enforce_default_encoding: false,
            max_text_file_mb: 100,
            open_view_mode: OpenViewMode::Auto,
            max_search_result_chars: 100,
            reload_files_on_startup: true,
            restore_caret_position: true,
            auto_save: false,
            auto_save_minutes: 10,
            auto_backup: false,
            purge_history_on_exit: false,

            date_time_format: "%Y-%m-%d %H:%M:%S".into(),
            insert_spaces_for_tab: false,
            auto_indent: true,
            auto_complete_pair: false,
            wrap_selection_with_pair: true,
            auto_copy_mouse_selection: false,
            middle_click_paste: cfg!(target_os = "linux"),
            auto_fill_column_paste: true,
            double_click_highlight: true,

            new_doc_encoding: "UTF-8".into(),
            new_doc_bom: false,
            new_doc_newline: if cfg!(windows) { "CRLF".into() } else { "LF".into() },
            new_doc_syntax: "Plain Text".into(),

            print: PrintOptions::default(),
            sort: SortOptions::default(),
            width: WidthOptions::default(),
            numbers: NumberOptions::default(),
            find_in_files: FindInFilesOptions::default(),
            key_bindings: HashMap::new(),
            astyle_options: "--style=allman --indent=spaces=4".into(),
            search_match_case: false,
            search_whole_word: false,
            search_regex: false,
            search_wrap_around: true,
            search_in_selection: false,

            recent_files: Vec::new(),
            recent_finds: Vec::new(),
            recent_replaces: Vec::new(),
            recent_dirs: Vec::new(),
            session_files: Vec::new(),
            session_active: 0,
            caret_positions: HashMap::new(),
            window_size: None,
            personal_dictionary: Vec::new(),
        }
    }
}

/// 歷史清單上限
const MAX_RECENT: usize = 20;

impl Settings {
    /// 設定檔路徑（例如 macOS 的 ~/Library/Application Support/MadEdit/settings.json）
    pub fn path() -> Option<PathBuf> {
        // 單元測試不讀寫使用者的設定檔
        if cfg!(test) {
            return None;
        }
        dirs::config_dir().map(|d| d.join("MadEdit").join("settings.json"))
    }

    /// 載入設定；檔案不存在或格式錯誤時使用預設值
    pub fn load() -> Self {
        Self::load_checked().0
    }

    /// 載入設定，並回傳需要告知使用者的錯誤（設定檔損毀時已另存備份）
    pub fn load_checked() -> (Self, Option<String>) {
        match Self::path() {
            Some(p) => Self::load_from(&p),
            None => (Self::default(), None),
        }
    }

    /// 自指定路徑載入；檔案不存在時靜默使用預設值，讀取或解析失敗時先把原檔複製為 `.bak`
    /// 再使用預設值，避免之後存回設定時永久覆蓋使用者的原始設定
    fn load_from(path: &Path) -> (Self, Option<String>) {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Self::default(), None),
            Err(e) => return (Self::default(), Some(Self::corrupt_message(path, &e.to_string()))),
        };
        match serde_json::from_str(&text) {
            Ok(s) => (s, None),
            Err(e) => (Self::default(), Some(Self::corrupt_message(path, &e.to_string()))),
        }
    }

    /// 備份無法使用的設定檔並產生提示訊息
    fn corrupt_message(path: &Path, err: &str) -> String {
        let mut bak = path.as_os_str().to_owned();
        bak.push(".bak");
        let bak = PathBuf::from(bak);
        match std::fs::copy(path, &bak) {
            Ok(_) => format!("設定檔無法讀取，已改用預設設定。\n原設定檔已備份為 {}\n\n錯誤：{err}", bak.display()),
            Err(e) => format!("設定檔無法讀取，已改用預設設定（備份失敗：{e}）。\n\n錯誤：{err}"),
        }
    }

    /// 寫回設定檔
    pub fn save(&self) -> std::io::Result<()> {
        match Self::path() {
            Some(p) => self.save_to(&p),
            None => Ok(()),
        }
    }

    /// 以原子寫入存到指定路徑，寫入途中中止也不會留下截斷的設定檔
    fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        super::document::atomic_write(path, |f| std::io::Write::write_all(f, json.as_bytes()))
    }

    /// 將項目移到歷史清單最前面
    pub fn push_history<T: PartialEq + Clone>(list: &mut Vec<T>, item: T) {
        list.retain(|x| *x != item);
        list.insert(0, item);
        list.truncate(MAX_RECENT);
    }

    pub fn add_recent_file(&mut self, path: PathBuf) {
        Self::push_history(&mut self.recent_files, path);
    }

    /// 清除所有歷史紀錄
    pub fn purge_history(&mut self) {
        self.recent_files.clear();
        self.recent_finds.clear();
        self.recent_replaces.clear();
        self.recent_dirs.clear();
        self.caret_positions.clear();
    }

    /// 記住檔案的游標位置（最多保留 500 筆）
    pub fn remember_caret(&mut self, path: &std::path::Path, line: usize, col: usize) {
        if self.caret_positions.len() > 500 {
            self.caret_positions.clear();
        }
        self.caret_positions
            .insert(path.to_string_lossy().into_owned(), (line, col));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("madedit_settings_{}_{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn test_settings_save_and_load_roundtrip() {
        let dir = temp_dir("roundtrip");
        let p = dir.join("settings.json");
        let mut s = Settings::default();
        s.add_recent_file(PathBuf::from("/a.txt"));
        s.save_to(&p).unwrap();
        let (loaded, err) = Settings::load_from(&p);
        assert!(err.is_none());
        assert_eq!(loaded.recent_files, vec![PathBuf::from("/a.txt")]);
        // 檔案不存在：靜默使用預設值
        let (_, err) = Settings::load_from(&dir.join("missing.json"));
        assert!(err.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_corrupt_settings_are_backed_up_and_reported() {
        let dir = temp_dir("corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("settings.json");
        let truncated = r#"{"recent_files": ["/a.txt""#;
        std::fs::write(&p, truncated).unwrap();
        let (loaded, err) = Settings::load_from(&p);
        assert!(err.is_some());
        assert!(loaded.recent_files.is_empty());
        // 原始內容保留在 .bak，之後存回預設值也不會遺失
        loaded.save_to(&p).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("settings.json.bak")).unwrap(), truncated);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_settings_roundtrip_and_partial_json() {
        let mut s = Settings { font_size: 18.0, ..Default::default() };
        s.add_recent_file(PathBuf::from("/a"));
        s.add_recent_file(PathBuf::from("/b"));
        s.add_recent_file(PathBuf::from("/a"));
        assert_eq!(s.recent_files, vec![PathBuf::from("/a"), PathBuf::from("/b")]);
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.font_size, 18.0);
        // 舊版設定檔缺少欄位時以預設值補上
        let partial: Settings = serde_json::from_str(r#"{"tab_size": 8}"#).unwrap();
        assert_eq!(partial.tab_size, 8);
        assert!(partial.show_line_numbers);
    }
}
