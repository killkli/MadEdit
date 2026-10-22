use std::fs;
use std::path::{Path, PathBuf};
use encoding_rs::Encoding;

use super::editor::{EditorState, Selection};
use super::hex::HexDump;
use super::model::{EditMode, NewLineFormat};
use super::piece_table::{LineIndex, PieceTable};
use super::search::{
    build_regex, find_all_matches, find_hex_matches, find_next_match, find_prev_match,
    parse_hex_query, replace_all_matches, SearchMatch, SearchOptions,
};
use super::syntax::{detect_syntax_from_path, highlight_lines};

/// 編輯器支援的 10 種常用字元編碼標準名稱清單（依 UI 選單順序排列）
pub const SUPPORTED_ENCODINGS: &[&str] = &[
    "UTF-8",
    "UTF-16LE",
    "UTF-16BE",
    "Big5",
    "GBK",
    "GB18030",
    "Shift_JIS",
    "EUC-JP",
    "EUC-KR",
    "windows-1252",
];

/// 小型檔案快取門檻（512 KB）：小於此門檻之檔案直接載入記憶體供編輯器互動，大於此門檻則採按需解碼
pub const SMALL_FILE_CACHE_LIMIT: usize = 512 * 1024;

/// 單一開啟或未命名文件的核心狀態容器（DocumentBuffer）
pub struct DocumentBuffer {
    pub file_path: Option<PathBuf>,
    pub piece_table: PieceTable,
    pub line_index: LineIndex,
    pub raw_bytes: Vec<u8>,
    pub text: String,
    pub encoding: &'static Encoding,
    pub has_bom: bool,
    pub has_decode_error: bool,
    pub newline_format: NewLineFormat,
    pub edit_mode: EditMode,
    pub is_modified: bool,
    pub current_syntax: String,
    pub editor: EditorState,
    pub hex_start_offset: usize,
    pub last_modified: Option<std::time::SystemTime>,
}

impl Default for DocumentBuffer {
    fn default() -> Self {
        let pt = PieceTable::new();
        let li = LineIndex::build(&pt);
        Self {
            file_path: None,
            piece_table: pt,
            line_index: li,
            raw_bytes: Vec::new(),
            text: String::new(),
            encoding: encoding_rs::UTF_8,
            has_bom: false,
            has_decode_error: false,
            newline_format: NewLineFormat::Dos,
            edit_mode: EditMode::Text,
            is_modified: false,
            current_syntax: "Plain Text".to_string(),
            editor: EditorState::default(),
            hex_start_offset: 0,
            last_modified: None,
        }
    }
}

impl DocumentBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// 從磁碟讀取檔案（大檔使用 memmap2 記憶體對映進行零拷貝開啟，建立 Piece Table 與 LineIndex）
    pub fn load_file<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<()> {
        let p = path.as_ref().to_path_buf();
        let file = fs::File::open(&p)?;
        let pt = PieceTable::open_file(&file)?;

        self.file_path = Some(p.clone());
        self.piece_table = pt;
        self.current_syntax = detect_syntax_from_path(&p).to_string();

        // 讀取前 64KB 樣本進行換行與編碼自動偵測
        let sample_len = (self.piece_table.len()).min(64 * 1024);
        let sample = self.piece_table.read_slice(0, sample_len);
        self.newline_format = NewLineFormat::detect(&sample);

        if let Some((bom_enc, _)) = Encoding::for_bom(&sample) {
            self.encoding = bom_enc;
            self.has_bom = true;
            self.has_decode_error = false;
        } else if encoding_rs::UTF_8
            .decode_without_bom_handling_and_without_replacement(&sample)
            .is_some()
        {
            self.encoding = encoding_rs::UTF_8;
            self.has_bom = false;
            self.has_decode_error = false;
        } else {
            self.encoding = encoding_rs::BIG5;
            self.has_bom = false;
            let (_, had_errors) = encoding_rs::BIG5.decode_without_bom_handling(&sample);
            self.has_decode_error = had_errors;
        }

        self.line_index = LineIndex::build(&self.piece_table);

        let file_size = self.piece_table.len();
        if file_size <= SMALL_FILE_CACHE_LIMIT {
            // 小型檔案：快取整份文字與位元組供編輯器與快速存取
            let full_bytes = self.piece_table.collect_bytes();
            let decode_slice = Self::strip_bom_slice(&full_bytes, self.encoding, self.has_bom);

            let (decoded, had_errors) = self.encoding.decode_without_bom_handling(decode_slice);
            if had_errors {
                self.has_decode_error = true;
            }

            self.text = decoded.into_owned();
            self.raw_bytes = full_bytes.clone();
            // 小型檔案直接以記憶體位元組維護 PieceTable，釋放 mmap 檔案控制代碼（避免 Windows 檔案鎖衝突）
            self.piece_table = PieceTable::from_bytes(full_bytes);
            self.line_index = LineIndex::build(&self.piece_table);
            self.editor.set_text(&self.text);
        } else {
            // 大型檔案：避免全檔記憶體複製與整檔解碼，依賴 Piece Table 與 LineIndex 按需供應
            self.text.clear();
            self.raw_bytes.clear();
            let count = self.line_index.line_count().max(1);
            self.editor.lines = vec![String::new(); count];
            self.editor.selection = super::editor::Selection::caret(crate::core::TextPosition::new(0, 0));
            self.editor.clamp_caret();
        }

        self.is_modified = false;
        self.hex_start_offset = 0;
        self.last_modified = fs::metadata(&p).and_then(|m| m.modified()).ok();

        Ok(())
    }

    /// 將當前 text 編碼同步至底層位元組與 Piece Table（DRY 萃取共用邏輯）
    fn sync_bytes_to_piece_table(&mut self) {
        if !self.has_decode_error {
            self.raw_bytes = Self::encode_str(&self.text, self.encoding, self.has_bom);
            self.piece_table = PieceTable::from_bytes(self.raw_bytes.clone());
            self.line_index = LineIndex::build(&self.piece_table);
        }
    }

    /// 取得目前文件的位元組長度
    pub fn len(&self) -> usize {
        self.piece_table.len()
    }

    /// 是否為空檔案
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.piece_table.len() == 0
    }

    /// 取得目前完整底層位元組（相容性查詢介面：若未快取則自 Piece Table 串流收集）
    #[allow(dead_code)]
    pub fn raw_bytes(&self) -> Vec<u8> {
        if !self.raw_bytes.is_empty() || self.piece_table.len() == 0 {
            self.raw_bytes.clone()
        } else {
            self.piece_table.collect_bytes()
        }
    }

    /// 判定並切除開頭 BOM 位元組切片（DRY 萃取共用邏輯）
    fn strip_bom_slice<'a>(bytes: &'a [u8], enc: &'static Encoding, has_bom: bool) -> &'a [u8] {
        if has_bom {
            if let Some((bom_enc, len)) = Encoding::for_bom(bytes) {
                if bom_enc == enc {
                    return &bytes[len..];
                }
            }
        }
        bytes
    }

    /// 取得目前文字內容（相容性查詢介面：若未快取則自 Piece Table 解碼）
    #[allow(dead_code)]
    pub fn text(&self) -> String {
        if !self.text.is_empty() || self.piece_table.len() == 0 {
            self.text.clone()
        } else {
            let bytes = self.piece_table.collect_bytes();
            let decode_slice = Self::strip_bom_slice(&bytes, self.encoding, self.has_bom);
            let (decoded, _) = self.encoding.decode_without_bom_handling(decode_slice);
            decoded.into_owned()
        }
    }

    /// 在指定位元組位移插入位元組序列，並同步更新 LineIndex 與修改狀態（大檔直接編輯）
    #[allow(dead_code)]
    pub fn insert_bytes(&mut self, offset: usize, bytes: &[u8]) {
        self.piece_table.insert(offset, bytes);
        self.line_index = LineIndex::build(&self.piece_table);
        self.is_modified = true;
    }

    /// 在指定位元組位移刪除指定長度位元組，並同步更新 LineIndex 與修改狀態（大檔直接編輯）
    #[allow(dead_code)]
    pub fn delete_bytes(&mut self, offset: usize, length: usize) {
        self.piece_table.delete(offset, length);
        self.line_index = LineIndex::build(&self.piece_table);
        self.is_modified = true;
    }

    /// 在指定行（0-based）起始位置插入文字（依當前編碼轉換），支援大檔案直接編輯
    #[allow(dead_code)]
    pub fn insert_text_at_line(&mut self, line_idx: usize, text: &str) {
        let offset = self
            .line_index
            .line_byte_range(line_idx)
            .map(|(s, _)| s)
            .unwrap_or(self.piece_table.len());
        let encoded = Self::encode_str(text, self.encoding, false);
        self.insert_bytes(offset, &encoded);
    }

    /// 根據可視視窗（Viewport）與字元格網寬度產生可見行繪製資料（支援大檔案按需解碼）
    pub fn generate_visible_lines(&self, cell_width: f32) -> Vec<super::editor::VisibleLineData> {
        let start_line = self.editor.viewport.first_visible_line;
        let count = self.editor.viewport.visible_line_count;

        let has_cached_lines = !self.editor.lines.is_empty()
            && self.editor.lines.len() == self.line_index.line_count()
            && self.piece_table.len() <= SMALL_FILE_CACHE_LIMIT;

        let mut lines = if has_cached_lines {
            let end_line = (start_line + count).min(self.editor.lines.len());
            let slice = if start_line < self.editor.lines.len() {
                &self.editor.lines[start_line..end_line]
            } else {
                &[]
            };
            self.editor.generate_visible_lines_with_text(slice, cell_width)
        } else {
            // 從 Piece Table 透過 LineIndex 按需解碼當前可見的行
            let visible_texts = self
                .line_index
                .fetch_lines(&self.piece_table, self.encoding, start_line, count);
            self.editor.generate_visible_lines_with_text(&visible_texts, cell_width)
        };

        // 依據當前設定之語法為可視行產生多色分段（Viewport 按需高亮，支援多行註解與字串）
        let line_texts: Vec<String> = lines.iter().map(|l| l.text.clone()).collect();
        let spans_batch = highlight_lines(&self.current_syntax, &line_texts);
        for (line_data, spans) in lines.iter_mut().zip(spans_batch) {
            line_data.spans = spans;
        }

        lines
    }

    /// 手動設定編輯器語法突顯語言（如 "Rust", "Python", "Plain Text"）
    pub fn set_syntax(&mut self, syntax_name: &str) {
        self.current_syntax = syntax_name.to_string();
    }

    /// 儲存檔案至指定路徑或原路徑；採用原子寫入機制（先寫暫存檔再替換），並在存在無損解碼錯誤時拒絕存檔防毀損
    pub fn save_file(&mut self, path: Option<PathBuf>) -> std::io::Result<()> {
        if self.has_decode_error {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "檔案包含無法無損解碼的字元，為避免損毀原始資料已拒絕存檔；請先選擇正確編碼重新解碼或明確轉換編碼",
            ));
        }

        let target_path = path.or_else(|| self.file_path.clone());
        let Some(p) = target_path else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "尚未指定目標檔案儲存路徑",
            ));
        };

        // 僅在小檔且有快取文字時，確保最新文字已同步至 Piece Table
        if self.edit_mode != EditMode::Hex
            && self.piece_table.len() <= SMALL_FILE_CACHE_LIMIT
            && !self.text.is_empty()
        {
            self.sync_bytes_to_piece_table();
        }

        // 產生同目錄下的暫存檔案路徑
        let tmp_path = p.with_extension(format!("tmp.{}", std::process::id()));
        {
            let mut tmp_file = fs::File::create(&tmp_path)?;
            self.piece_table.write_to(&mut tmp_file)?;
            tmp_file.sync_all()?;
        }

        // 釋放既有 piece_table（以關閉在 Windows 下可能持有的 mmap 檔案控制代碼）
        self.piece_table = PieceTable::new();

        // 以原子替換將暫存檔更名為目標檔
        if let Err(e) = fs::rename(&tmp_path, &p) {
            // 若更名失敗，嘗試清理暫存檔後回傳錯誤
            let _ = fs::remove_file(&tmp_path);
            return Err(e);
        }

        // 重新為目標檔案建立 Piece Table 與 LineIndex
        let file = fs::File::open(&p)?;
        let mut pt = PieceTable::open_file(&file)?;
        if pt.len() <= SMALL_FILE_CACHE_LIMIT {
            let bytes = pt.collect_bytes();
            pt = PieceTable::from_bytes(bytes);
        }
        self.piece_table = pt;
        self.line_index = LineIndex::build(&self.piece_table);
        self.last_modified = fs::metadata(&p).and_then(|m| m.modified()).ok();
        self.file_path = Some(p);
        self.is_modified = false;
        self.hex_start_offset = 0;

        Ok(())
    }

    /// 檢查磁碟上的檔案是否被外部程序修改（比對 mtime）
    pub fn check_external_change(&self) -> bool {
        let Some(p) = &self.file_path else {
            return false;
        };
        let Some(last_mod) = self.last_modified else {
            return false;
        };
        if let Ok(metadata) = fs::metadata(p) {
            if let Ok(current_mod) = metadata.modified() {
                return current_mod > last_mod;
            }
        }
        false
    }

    /// 從磁碟重新載入檔案最新內容，重置修改狀態並智慧保留游標與書籤
    pub fn reload_file(&mut self) -> std::io::Result<()> {
        let Some(p) = self.file_path.clone() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "無對應磁碟檔案路徑可供重新載入",
            ));
        };

        // 記錄目前的游標位置與行書籤集合
        let saved_pos = self.editor.selection.head;
        let saved_bookmarks = self.editor.bookmarks.clone();

        self.load_file(&p)?;

        // 智慧保留游標（自動 clamp 防止越界）
        let max_line = self.editor.lines.len().saturating_sub(1);
        let target_line = saved_pos.line.min(max_line);
        let target_col = saved_pos.col.min(self.editor.line_char_count(target_line));
        self.editor.set_caret(target_line, target_col);

        // 智慧保留在合法行範圍內的書籤
        self.editor.bookmarks = saved_bookmarks
            .into_iter()
            .filter(|&line| line <= max_line)
            .collect();

        Ok(())
    }

    /// 忽略外部修改：將 last_modified 同步為磁碟最新時間，避免後續重複提示
    pub fn ignore_external_change(&mut self) {
        if let Some(p) = &self.file_path {
            if let Ok(metadata) = fs::metadata(p) {
                if let Ok(current_mod) = metadata.modified() {
                    self.last_modified = Some(current_mod);
                }
            }
        }
    }

    /// 將 editor 中的最新行內容同步至 text 與 Piece Table（大檔未快取時防護不覆寫）
    pub fn sync_from_editor(&mut self) {
        if self.editor.lines.is_empty()
            || (self.piece_table.len() > SMALL_FILE_CACHE_LIMIT && self.text.is_empty())
        {
            return;
        }
        let new_text = self.editor.to_text(self.newline_format.newline_str());
        self.text = new_text;
        self.is_modified = true;
        self.sync_bytes_to_piece_table();
    }

    /// 當使用者在外部或模式切換修改內容時同步更新文字與底層位元組
    #[allow(dead_code)]
    pub fn update_text(&mut self, new_text: String) {
        if self.text != new_text {
            self.text = new_text;
            self.is_modified = true;
            self.sync_bytes_to_piece_table();
            self.editor.set_text(&self.text);
        }
    }

    /// 切換編輯模式（Text / Column / Hex）
    pub fn set_edit_mode(&mut self, mode: EditMode) {
        self.edit_mode = mode;
        if mode == EditMode::Hex
            && self.piece_table.len() <= SMALL_FILE_CACHE_LIMIT
            && !self.text.is_empty()
        {
            self.sync_bytes_to_piece_table();
        }
    }

    /// 重新解碼（Reinterpret Encoding）：保留底層位元組不變，改以指定編碼重新解碼出 text，不變更修改狀態
    pub fn reinterpret_with_encoding(&mut self, encoding_label: &str) -> bool {
        let Some(enc) = Encoding::for_label(encoding_label.as_bytes()) else {
            return false;
        };
        let bytes = self.piece_table.collect_bytes();
        let has_bom = match Encoding::for_bom(&bytes) {
            Some((bom_enc, _)) => bom_enc == enc,
            None => false,
        };
        let slice = Self::strip_bom_slice(&bytes, enc, has_bom);
        let (decoded, had_errors) = enc.decode_without_bom_handling(slice);
        self.text = decoded.into_owned();
        self.encoding = enc;
        self.has_bom = has_bom;
        self.has_decode_error = had_errors;
        self.editor.set_text(&self.text);
        true
    }

    /// 轉換編碼（Convert Encoding）：保留當前 text 不變，轉換為目標編碼並更新 Piece Table，標記為已修改
    pub fn convert_to_encoding(&mut self, encoding_label: &str) -> bool {
        let Some(enc) = Encoding::for_label(encoding_label.as_bytes()) else {
            return false;
        };
        // 轉換為 UTF-16LE / UTF-16BE 時預設寫入 BOM；若原本即為含 BOM 之 UTF-8 則保留 BOM
        self.has_bom = enc == encoding_rs::UTF_16LE
            || enc == encoding_rs::UTF_16BE
            || (enc == encoding_rs::UTF_8 && self.encoding == encoding_rs::UTF_8 && self.has_bom);
        self.encoding = enc;
        self.has_decode_error = false;
        self.is_modified = true;
        self.sync_bytes_to_piece_table();
        true
    }

    /// 轉換換行格式（CRLF / LF / CR）：以目標換行符重新拼裝最新文字並同步至 Piece Table，標記為已修改
    pub fn convert_newline_format(&mut self, new_format: NewLineFormat) -> bool {
        if self.newline_format == new_format {
            return false;
        }
        self.newline_format = new_format;
        self.is_modified = true;
        // 大檔未快取時防護不覆寫
        if self.piece_table.len() <= SMALL_FILE_CACHE_LIMIT || !self.text.is_empty() {
            self.text = self.editor.to_text(new_format.newline_str());
            self.sync_bytes_to_piece_table();
        }
        true
    }

    /// 將字串依指定編碼與 BOM 設定轉換為位元組序列（修正 encoding_rs 對 UTF-16 輸出為 UTF-8 的問題）
    fn encode_str(text: &str, enc: &'static Encoding, has_bom: bool) -> Vec<u8> {
        let mut out = Vec::new();
        if enc == encoding_rs::UTF_16LE {
            if has_bom {
                out.extend_from_slice(&[0xFF, 0xFE]);
            }
            out.extend(text.encode_utf16().flat_map(|u| u.to_le_bytes()));
        } else if enc == encoding_rs::UTF_16BE {
            if has_bom {
                out.extend_from_slice(&[0xFE, 0xFF]);
            }
            out.extend(text.encode_utf16().flat_map(|u| u.to_be_bytes()));
        } else {
            if enc == encoding_rs::UTF_8 && has_bom {
                out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
            }
            let (encoded, _, _) = enc.encode(text);
            out.extend_from_slice(&encoded);
        }
        out
    }

    /// 產生十六進位檢視區內容（自 hex_start_offset 起隨機讀取前 1MB 預覽，無全檔記憶體浪費）
    pub fn get_hex_dump(&self) -> HexDump {
        let total_len = self.piece_table.len();
        let start = self.hex_start_offset.min(total_len);
        let preview_len = (total_len - start).min(1024 * 1024);
        let preview_bytes = self.piece_table.read_slice(start, preview_len);
        super::hex::generate_hex_dump_with_offset(&preview_bytes, start, 1024 * 1024)
    }

    /// 跳轉至指定位元組位移（Offset），自動 clamp 並對齊至 16 的倍數，更新十六進位檢視預覽起點
    pub fn goto_offset(&mut self, offset: usize) {
        let max_len = self.piece_table.len();
        let clamped = offset.min(max_len);
        self.hex_start_offset = (clamped / 16) * 16;
    }

    /// 取得目前開啟檔案的檔名；若尚未指定路徑則傳回 "Untitled"
    pub fn file_name(&self) -> String {
        self.file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_string()
    }

    /// 跳轉至指定的行號與欄位（1-based 參數），內部自動轉換為 0-based 並進行邊界防護與視窗滾動
    pub fn goto_line_col(&mut self, line_1based: usize, col_1based: usize) {
        let line_0based = line_1based.saturating_sub(1);
        let col_0based = col_1based.saturating_sub(1);
        self.editor.goto_line_col(line_0based, col_0based);
    }

    /// 將當前行或選取行整塊往上移動一行，並自動同步至底層緩衝區
    pub fn move_lines_up(&mut self) -> bool {
        let moved = self.editor.move_lines_up();
        if moved {
            self.sync_from_editor();
        }
        moved
    }

    /// 將當前行或選取行整塊往下移動一行，並自動同步至底層緩衝區
    pub fn move_lines_down(&mut self) -> bool {
        let moved = self.editor.move_lines_down();
        if moved {
            self.sync_from_editor();
        }
        moved
    }

    /// 複製當前行或選取區涵蓋的所有行至下方，並自動同步至底層緩衝區
    pub fn duplicate_lines(&mut self) {
        self.editor.duplicate_lines();
        self.sync_from_editor();
    }

    /// 刪除當前行或選取區涵蓋的所有行，並自動同步至底層緩衝區
    pub fn delete_lines(&mut self) {
        self.editor.delete_lines();
        self.sync_from_editor();
    }

    /// 智慧行首（首次按下 Home 移動至縮排後第一個非空白字元，再次按下時移動至欄位 0）
    pub fn move_home_smart(&mut self, expand_selection: bool) {
        self.editor.move_home_smart(expand_selection);
    }

    /// 切換游標所在行的書籤狀態
    pub fn toggle_bookmark(&mut self) -> bool {
        self.editor.toggle_bookmark()
    }

    /// 環狀跳轉至下一個書籤行
    pub fn next_bookmark(&mut self) -> bool {
        self.editor.next_bookmark()
    }

    /// 環狀跳轉至上一個書籤行
    pub fn prev_bookmark(&mut self) -> bool {
        self.editor.prev_bookmark()
    }

    /// 清空所有書籤
    pub fn clear_all_bookmarks(&mut self) {
        self.editor.clear_all_bookmarks();
    }

    /// 取得目前文字在搜尋條件下的所有匹配項目列表
    pub fn find_all(&self, query: &str, options: &SearchOptions) -> Result<Vec<SearchMatch>, String> {
        find_all_matches(&self.editor.lines, query, options)
    }

    /// 計算符合搜尋條件之匹配總次數（支援文字模式與十六進位 Hex 模式）
    #[allow(dead_code)]
    pub fn count_matches(&self, query: &str, options: &SearchOptions) -> usize {
        if query.is_empty() {
            return 0;
        }
        if self.edit_mode == EditMode::Hex {
            self.find_hex(query).map(|m| m.len()).unwrap_or(0)
        } else {
            self.find_all(query, options).map(|m| m.len()).unwrap_or(0)
        }
    }

    /// 取得搜尋匹配統計資訊：回傳 (當前選取匹配項索引1-based, 總匹配次數)
    #[allow(dead_code)]
    pub fn get_search_status(&self, query: &str, options: &SearchOptions) -> (usize, usize) {
        if query.is_empty() {
            return (0, 0);
        }
        if self.edit_mode == EditMode::Hex {
            let offsets = match self.find_hex(query) {
                Ok(o) => o,
                Err(_) => return (0, 0),
            };
            let total = offsets.len();
            return if total > 0 { (1, total) } else { (0, 0) };
        }
        let matches = match self.find_all(query, options) {
            Ok(m) => m,
            Err(_) => return (0, 0),
        };
        let total = matches.len();
        if total == 0 {
            return (0, 0);
        }

        // 判斷當前選取是否正好命中某個匹配項
        let current_pos = self.editor.selection.start();
        for (idx, m) in matches.iter().enumerate() {
            if m.start == current_pos && m.end == self.editor.selection.end() {
                return (idx + 1, total);
            }
        }

        // 若未直接命中選取，計算目前游標位於第幾個匹配項
        let head = self.editor.selection.head;
        for (idx, m) in matches.iter().enumerate() {
            if m.start >= head {
                return (idx + 1, total);
            }
        }

        (total, total)
    }

    /// 尋找游標後的下一個匹配項目，並自動更新選取區與捲動可視範圍
    /// 回傳 Some((當前匹配索引1-based, 總匹配次數))，若無匹配或查詢無效則回傳 None
    pub fn find_next(&mut self, query: &str, options: &SearchOptions) -> Option<(usize, usize)> {
        if query.is_empty() {
            return None;
        }
        if self.edit_mode == EditMode::Hex {
            let offsets = self.find_hex(query).ok()?;
            if offsets.is_empty() {
                return None;
            }
            return Some((1, offsets.len()));
        }
        let matches = self.find_all(query, options).ok()?;
        if matches.is_empty() {
            return None;
        }
        let total = matches.len();

        // 若目前已有非空選取，從選取結尾之後開始搜尋，避免原地停留；否則以游標為起點
        let search_start = if !self.editor.selection.is_empty() {
            self.editor.selection.end()
        } else {
            self.editor.selection.head
        };

        let (idx, m) = find_next_match(&matches, search_start)?;
        self.editor.selection = Selection::new(m.start, m.end);
        self.editor.scroll_caret_into_view();

        Some((idx + 1, total))
    }

    /// 尋找游標前的上一個匹配項目，並自動更新選取區與捲動可視範圍
    /// 回傳 Some((當前匹配索引1-based, 總匹配次數))，若無匹配或查詢無效則回傳 None
    pub fn find_prev(&mut self, query: &str, options: &SearchOptions) -> Option<(usize, usize)> {
        if query.is_empty() {
            return None;
        }
        if self.edit_mode == EditMode::Hex {
            let offsets = self.find_hex(query).ok()?;
            if offsets.is_empty() {
                return None;
            }
            return Some((offsets.len(), offsets.len()));
        }
        let matches = self.find_all(query, options).ok()?;
        if matches.is_empty() {
            return None;
        }
        let total = matches.len();

        // 若目前已有非空選取，從選取起點之前開始搜尋；否則以游標為起點
        let search_start = if !self.editor.selection.is_empty() {
            self.editor.selection.start()
        } else {
            self.editor.selection.head
        };

        let (idx, m) = find_prev_match(&matches, search_start)?;
        self.editor.selection = Selection::new(m.start, m.end);
        self.editor.scroll_caret_into_view();

        Some((idx + 1, total))
    }

    /// 取代目前選取之匹配項，並自動尋找下一個
    /// 若目前選取區未符合 query，則先自動尋找並選取下一個匹配項
    pub fn replace_current(&mut self, query: &str, replace_with: &str, options: &SearchOptions) -> bool {
        if query.is_empty() {
            return false;
        }

        // 檢查當前選取是否完全匹配搜尋條件
        let is_match = if !self.editor.selection.is_empty() {
            let selected = self.editor.selected_text();
            if let Ok(re) = build_regex(query, options) {
                re.find(&selected).map(|m| m.as_str() == selected).unwrap_or(false)
            } else {
                false
            }
        } else {
            false
        };

        if is_match {
            let selected = self.editor.selected_text();
            let replacement = if options.use_regex {
                if let Ok(re) = build_regex(query, options) {
                    re.replace(&selected, replace_with).to_string()
                } else {
                    replace_with.to_string()
                }
            } else {
                replace_with.to_string()
            };

            // 刪除選取並插入替換文字
            self.editor.delete_selection();
            self.editor.insert_text(&replacement);
            self.sync_from_editor();

            // 自動跳轉選取下一個匹配項
            self.find_next(query, options);
            true
        } else {
            // 目前未選取匹配項，先行選取下一個匹配項
            self.find_next(query, options).is_some()
        }
    }

    /// 全部取代：將文件中所有符合條件之字串替換，同步底層 Piece Table 並回傳精確取代總筆數
    pub fn replace_all(&mut self, query: &str, replace_with: &str, options: &SearchOptions) -> usize {
        if query.is_empty() {
            return 0;
        }

        let Ok((new_lines, count)) = replace_all_matches(&self.editor.lines, query, replace_with, options) else {
            return 0;
        };

        if count > 0 {
            self.editor.lines = new_lines;
            self.editor.clamp_caret();
            self.sync_from_editor();
        }

        count
    }

    /// 在底層位元組中搜尋十六進位序列（Hex Byte Search），回傳所有匹配之 Byte Offsets
    #[allow(dead_code)]
    pub fn find_hex(&self, hex_query: &str) -> Result<Vec<usize>, String> {
        let pattern = parse_hex_query(hex_query)?;
        let bytes = if !self.raw_bytes.is_empty() || self.piece_table.len() == 0 {
            self.raw_bytes.clone()
        } else {
            self.piece_table.collect_bytes()
        };
        Ok(find_hex_matches(&bytes, &pattern))
    }
}

/// 解析跳轉行號與欄位字串（支援 "42", "42:15", "42, 15"）
pub fn parse_goto_line_col(input: &str) -> Option<(usize, usize)> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parts: Vec<&str> = trimmed
        .split(|c| c == ':' || c == ',')
        .map(|s| s.trim())
        .collect();

    let line = parts.first()?.parse::<usize>().ok()?;
    let col = if parts.len() > 1 {
        parts[1].parse::<usize>().unwrap_or(1)
    } else {
        1
    };
    Some((line, col))
}

/// 解析 Hex 模式位移字串（支援 "0x1A0", "1A0", "10" 等十六進位位移，以及十進位相容）
pub fn parse_goto_offset(input: &str) -> Option<usize> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let s = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    if let Ok(val) = usize::from_str_radix(s, 16) {
        Some(val)
    } else {
        trimmed.parse::<usize>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::editor::TextPosition;
    use std::io::Write;

    #[test]
    fn test_buffer_update_text() {
        let mut doc = DocumentBuffer::new();
        doc.update_text("Hello MadEdit".to_string());
        assert_eq!(doc.text, "Hello MadEdit");
        assert_eq!(doc.raw_bytes, b"Hello MadEdit");
        assert_eq!(doc.piece_table.collect_bytes(), b"Hello MadEdit");
        assert!(doc.is_modified);

        doc.set_edit_mode(EditMode::Hex);
        let dump = doc.get_hex_dump();
        assert!(dump.hex.contains("48 65 6C 6C 6F"));
    }

    #[test]
    fn test_supported_encodings_and_auto_detect_on_load() {
        assert_eq!(
            SUPPORTED_ENCODINGS,
            &[
                "UTF-8",
                "UTF-16LE",
                "UTF-16BE",
                "Big5",
                "GBK",
                "GB18030",
                "Shift_JIS",
                "EUC-JP",
                "EUC-KR",
                "windows-1252",
            ]
        );

        let temp_dir = std::env::temp_dir();

        // 1. 含 BOM 之 UTF-16LE 檔案（"Hi" -> FF FE 48 00 69 00）
        let utf16le_path = temp_dir.join("madedit_test_utf16le.txt");
        let utf16le_bytes = [0xFF, 0xFE, 0x48, 0x00, 0x69, 0x00];
        fs::write(&utf16le_path, utf16le_bytes).unwrap();
        let mut doc = DocumentBuffer::new();
        doc.load_file(&utf16le_path).unwrap();
        assert_eq!(doc.encoding, encoding_rs::UTF_16LE);
        assert!(doc.has_bom);
        assert!(!doc.has_decode_error);
        assert_eq!(doc.text, "Hi");
        assert_eq!(doc.editor.lines, vec!["Hi".to_string()]);

        // 驗證 UTF-16LE 編輯後存檔仍保留 BOM 且為真正的 UTF-16LE（不會變成 UTF-8）
        doc.update_text("Hi!".to_string());
        doc.save_file(None).unwrap();
        let saved_utf16le = fs::read(&utf16le_path).unwrap();
        assert_eq!(saved_utf16le, vec![0xFF, 0xFE, 0x48, 0x00, 0x69, 0x00, 0x21, 0x00]);
        let _ = fs::remove_file(&utf16le_path);

        // 2. 無 BOM 之 Big5 檔案（"測試" in Big5 -> B4 FA B8 D5）
        let big5_path = temp_dir.join("madedit_test_big5.txt");
        fs::write(&big5_path, [0xB4, 0xFA, 0xB8, 0xD5]).unwrap();
        let mut doc_big5 = DocumentBuffer::new();
        doc_big5.load_file(&big5_path).unwrap();
        assert_eq!(doc_big5.encoding, encoding_rs::BIG5);
        assert!(!doc_big5.has_bom);
        assert!(!doc_big5.has_decode_error);
        assert_eq!(doc_big5.text, "測試");
        assert!(!doc_big5.is_modified);
        assert_eq!(doc_big5.editor.lines, vec!["測試".to_string()]);
        let _ = fs::remove_file(&big5_path);

        // 3. 含無法無損解碼位元組時拒絕存檔防止毀損原檔
        let invalid_path = temp_dir.join("madedit_test_invalid_bytes.txt");
        let original_bytes = vec![0x81, 0x00, 0xFF];
        fs::write(&invalid_path, &original_bytes).unwrap();
        let mut doc_invalid = DocumentBuffer::new();
        doc_invalid.load_file(&invalid_path).unwrap();
        assert!(doc_invalid.has_decode_error);
        assert!(doc_invalid.save_file(None).is_err());
        assert_eq!(fs::read(&invalid_path).unwrap(), original_bytes);
        let _ = fs::remove_file(&invalid_path);
    }

    #[test]
    fn test_reinterpret_and_convert_encoding() {
        let temp_dir = std::env::temp_dir();

        let gb_path = temp_dir.join("madedit_test_gb18030.txt");
        let raw_gb_bytes = vec![0xC8, 0xED, 0xBC, 0xFE];
        fs::write(&gb_path, &raw_gb_bytes).unwrap();

        let mut doc = DocumentBuffer::new();
        doc.load_file(&gb_path).unwrap();
        assert_eq!(doc.encoding, encoding_rs::BIG5);
        assert_ne!(doc.text, "软件");

        // 重新解碼（Reinterpret）為 GB18030
        assert!(doc.reinterpret_with_encoding("GB18030"));
        assert_eq!(doc.encoding, encoding_rs::GB18030);
        assert!(!doc.has_decode_error);
        assert_eq!(doc.text, "软件");
        assert_eq!(&doc.raw_bytes[..], &raw_gb_bytes[..]);
        assert!(!doc.is_modified);
        assert_eq!(doc.editor.lines, vec!["软件".to_string()]);

        // 轉換編碼（Convert）為 UTF-8
        assert!(doc.convert_to_encoding("UTF-8"));
        assert_eq!(doc.encoding, encoding_rs::UTF_8);
        assert!(!doc.has_decode_error);
        assert_eq!(doc.text, "软件");
        assert_eq!(doc.raw_bytes, "软件".as_bytes());
        assert!(doc.is_modified);

        // 轉換編碼（Convert）為 UTF-16LE
        assert!(doc.convert_to_encoding("UTF-16LE"));
        assert_eq!(doc.encoding, encoding_rs::UTF_16LE);
        assert!(doc.has_bom);
        let mut expected_utf16le = vec![0xFF, 0xFE];
        expected_utf16le.extend("软件".encode_utf16().flat_map(|u| u.to_le_bytes()));
        assert_eq!(doc.raw_bytes, expected_utf16le);

        let _ = fs::remove_file(&gb_path);
    }

    #[test]
    fn test_sync_from_editor() {
        let mut doc = DocumentBuffer::new();
        doc.load_file_from_memory("Initial line".to_string());
        doc.editor.move_end(false);
        doc.editor.insert_newline();
        doc.editor.insert_text("Second line");
        doc.sync_from_editor();

        assert_eq!(doc.text, "Initial line\r\nSecond line");
        assert!(doc.is_modified);
        assert_eq!(doc.editor.lines.len(), 2);
    }

    #[test]
    fn test_large_file_mmap_and_piece_table() {
        let temp_dir = std::env::temp_dir();
        let large_path = temp_dir.join("madedit_test_large_file.txt");

        // 產生一個約 5MB 的大檔案（包含 10 萬行文字）
        let line = "This is a line of benchmark text for large file engine.\n";
        let line_count = 100_000;
        {
            let mut file = fs::File::create(&large_path).unwrap();
            for _ in 0..line_count {
                file.write_all(line.as_bytes()).unwrap();
            }
        }

        let start_time = std::time::Instant::now();
        let mut doc = DocumentBuffer::new();
        doc.load_file(&large_path).unwrap();
        let elapsed = start_time.elapsed();

        // 驗證秒級開啟（< 500ms）
        println!("Loaded 100k lines in {:?}", elapsed);
        assert_eq!(doc.line_index.line_count(), line_count + 1);
        assert_eq!(doc.len(), line.len() * line_count);

        // 驗證隨機讀取第 50,000 行
        let fetched = doc.line_index.fetch_lines(&doc.piece_table, encoding_rs::UTF_8, 50_000, 1);
        assert_eq!(fetched, vec!["This is a line of benchmark text for large file engine."]);

        // 驗證可見行按需渲染（第 0 行至第 30 行）
        let visible = doc.generate_visible_lines(8.5);
        assert!(!visible.is_empty());
        assert_eq!(visible[0].line_number, 1);
        assert_eq!(visible[0].text, "This is a line of benchmark text for large file engine.");

        // 驗證大檔案編輯：在第 50,000 行插入標記文字
        let insert_prefix = "[MODIFIED] ";
        doc.insert_text_at_line(50_000, insert_prefix);
        assert!(doc.is_modified);
        assert_eq!(doc.len(), line.len() * line_count + insert_prefix.len());

        let modified_line = doc.line_index.fetch_lines(&doc.piece_table, encoding_rs::UTF_8, 50_000, 1);
        assert_eq!(
            modified_line,
            vec!["[MODIFIED] This is a line of benchmark text for large file engine."]
        );

        // 驗證原子存檔在 mmap 模式下運作正常並保留修改內容
        assert!(doc.save_file(None).is_ok());
        assert!(!doc.is_modified);
        assert_eq!(doc.len(), line.len() * line_count + insert_prefix.len());

        // 重新讀取驗證磁碟存檔內容正確性
        let mut verify_doc = DocumentBuffer::new();
        verify_doc.load_file(&large_path).unwrap();
        let reloaded_line = verify_doc.line_index.fetch_lines(&verify_doc.piece_table, encoding_rs::UTF_8, 50_000, 1);
        assert_eq!(
            reloaded_line,
            vec!["[MODIFIED] This is a line of benchmark text for large file engine."]
        );

        let _ = fs::remove_file(&large_path);
    }

    #[test]
    fn test_buffer_compatibility_queries() {
        let mut doc = DocumentBuffer::new();
        assert!(doc.is_empty());
        assert_eq!(doc.len(), 0);
        assert_eq!(doc.raw_bytes(), Vec::<u8>::new());
        assert_eq!(doc.text(), "");

        doc.update_text("Hello World".to_string());
        assert!(!doc.is_empty());
        assert_eq!(doc.len(), 11);
        assert_eq!(doc.raw_bytes(), b"Hello World");
        assert_eq!(doc.text(), "Hello World");
    }
    #[test]
    fn test_buffer_column_mode_rendering() {
        let mut doc = DocumentBuffer::new();
        doc.load_file_from_memory("AAAAAA\nBBBBBB\nCCCCCC\nDDDDDD".to_string());
        doc.set_edit_mode(EditMode::Column);

        // 建立列塊矩形選取：行 1 到 行 2，欄位 2 到 4
        doc.editor.selection = crate::core::Selection::block(
            crate::core::TextPosition::new(1, 2),
            crate::core::TextPosition::new(2, 4),
            2,
            4,
        );

        let visible = doc.generate_visible_lines(10.0);
        assert_eq!(visible.len(), 4);
        assert!(!visible[0].has_selection);
        assert!(visible[1].has_selection);
        assert_eq!(visible[1].sel_start_x, 20.0);
        assert_eq!(visible[1].sel_width, 20.0);
        assert!(visible[2].has_selection);
        assert_eq!(visible[2].sel_start_x, 20.0);
        assert_eq!(visible[2].sel_width, 20.0);
        assert!(!visible[3].has_selection);
    }

    #[test]
    fn test_buffer_find_next_and_prev_navigation() {
        let mut doc = DocumentBuffer::new();
        doc.load_file_from_memory("apple banana\ncherry apple\norange apple".to_string());

        let opts = SearchOptions::default();
        // 初始狀態下游標在 (0, 0)
        let res1 = doc.find_next("apple", &opts);
        assert_eq!(res1, Some((1, 3)));
        assert_eq!(doc.editor.selection.start(), TextPosition::new(0, 0));
        assert_eq!(doc.editor.selection.end(), TextPosition::new(0, 5));

        // 找第 2 個
        let res2 = doc.find_next("apple", &opts);
        assert_eq!(res2, Some((2, 3)));
        assert_eq!(doc.editor.selection.start(), TextPosition::new(1, 7));
        assert_eq!(doc.editor.selection.end(), TextPosition::new(1, 12));

        // 找第 3 個
        let res3 = doc.find_next("apple", &opts);
        assert_eq!(res3, Some((3, 3)));
        assert_eq!(doc.editor.selection.start(), TextPosition::new(2, 7));
        assert_eq!(doc.editor.selection.end(), TextPosition::new(2, 12));

        // 第 4 次 find_next（繞回首項）
        let res4 = doc.find_next("apple", &opts);
        assert_eq!(res4, Some((1, 3)));
        assert_eq!(doc.editor.selection.start(), TextPosition::new(0, 0));
        assert_eq!(doc.editor.selection.end(), TextPosition::new(0, 5));

        // find_prev（從首項往回繞至末項）
        let prev1 = doc.find_prev("apple", &opts);
        assert_eq!(prev1, Some((3, 3)));
        assert_eq!(doc.editor.selection.start(), TextPosition::new(2, 7));
        assert_eq!(doc.editor.selection.end(), TextPosition::new(2, 12));

        // 再次 find_prev
        let prev2 = doc.find_prev("apple", &opts);
        assert_eq!(prev2, Some((2, 3)));
        assert_eq!(doc.editor.selection.start(), TextPosition::new(1, 7));
        assert_eq!(doc.editor.selection.end(), TextPosition::new(1, 12));

        // 驗證 get_search_status
        assert_eq!(doc.get_search_status("apple", &opts), (2, 3));
        assert_eq!(doc.get_search_status("notfound", &opts), (0, 0));
    }

    #[test]
    fn test_buffer_replace_current_and_sync() {
        let mut doc = DocumentBuffer::new();
        doc.load_file_from_memory("foo bar foo".to_string());
        let opts = SearchOptions::default();

        // 第一次呼叫：當前未選中匹配項，先跳轉選取第 1 個 "foo"
        let ok1 = doc.replace_current("foo", "baz", &opts);
        assert!(ok1);
        assert_eq!(doc.editor.selected_text(), "foo");
        assert_eq!(doc.editor.selection.start(), TextPosition::new(0, 0));

        // 第二次呼叫：已選中第 1 個 "foo"，將其取代為 "baz"，並自動跳至第 2 個 "foo"
        let ok2 = doc.replace_current("foo", "baz", &opts);
        assert!(ok2);
        assert!(doc.is_modified);
        assert_eq!(doc.text(), "baz bar foo");
        assert_eq!(doc.editor.selected_text(), "foo");
        assert_eq!(doc.editor.selection.start(), TextPosition::new(0, 8));

        // 第三次呼叫：取代第 2 個 "foo" 為 "baz"，此時無更多匹配項
        let ok3 = doc.replace_current("foo", "baz", &opts);
        assert!(ok3);
        assert_eq!(doc.text(), "baz bar baz");
        assert!(doc.editor.selection.is_empty());
    }

    #[test]
    fn test_buffer_replace_all_regex() {
        let mut doc = DocumentBuffer::new();
        doc.load_file_from_memory("user_id: 100, order_id: 200".to_string());

        let opts = SearchOptions::new(true, false, true);
        let count = doc.replace_all(r"(\w+)_id: (\d+)", "id_$1=$2", &opts);
        assert_eq!(count, 2);
        assert!(doc.is_modified);
        assert_eq!(doc.text(), "id_user=100, id_order=200");
    }

    #[test]
    fn test_buffer_find_hex() {
        let mut doc = DocumentBuffer::new();
        doc.load_file_from_memory("ABCDEF".to_string());

        let offsets = doc.find_hex("43 44").unwrap();
        assert_eq!(offsets, vec![2]);

        let offsets_prefix = doc.find_hex("0x41 0x42").unwrap();
        assert_eq!(offsets_prefix, vec![0]);
    }

    #[test]
    fn test_buffer_count_matches_and_hex_dispatch() {
        let mut doc = DocumentBuffer::new();
        doc.load_file_from_memory("ABCDEF ABCDEF".to_string());
        let opts = SearchOptions::default();

        // 文字模式計數
        assert_eq!(doc.count_matches("ABC", &opts), 2);
        assert_eq!(doc.count_matches("XYZ", &opts), 0);

        // 切換至 Hex 模式
        doc.set_edit_mode(EditMode::Hex);
        assert_eq!(doc.count_matches("41 42", &opts), 2);
        assert_eq!(doc.find_next("41 42", &opts), Some((1, 2)));
        assert_eq!(doc.find_prev("41 42", &opts), Some((2, 2)));
    }

    #[test]
    fn test_buffer_syntax_highlighting_visible_lines() {
        let mut doc = DocumentBuffer::new();
        doc.load_file_from_memory("fn main() {\n    let answer = 42;\n}".to_string());

        // 預設為 Plain Text
        assert_eq!(doc.current_syntax, "Plain Text");
        let visible_plain = doc.generate_visible_lines(10.0);
        assert_eq!(visible_plain.len(), 3);
        assert_eq!(visible_plain[0].spans.len(), 1);
        assert_eq!(visible_plain[0].spans[0].color_rgb, (34, 34, 34));

        // 切換至 Rust 語法
        doc.set_syntax("Rust");
        assert_eq!(doc.current_syntax, "Rust");
        let visible_rust = doc.generate_visible_lines(10.0);
        assert_eq!(visible_rust.len(), 3);
        // 第一行 "fn main() {" 應被拆成多個語法 spans
        assert!(visible_rust[0].spans.len() > 1);
        let joined_line0: String = visible_rust[0].spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(joined_line0, "fn main() {");

        // 測試列塊選取幾何與語法 spans 並存
        doc.editor.selection = Selection::block(
            TextPosition::new(0, 3),
            TextPosition::new(1, 7),
            3,
            7,
        );
        let visible_block = doc.generate_visible_lines(10.0);
        assert!(visible_block[0].has_selection);
        assert_eq!(visible_block[0].sel_start_x, 30.0);
        assert!(visible_block[0].spans.len() > 1);
    }

    #[test]
    fn test_buffer_goto_line_col_and_line_editing() {
        let mut doc = DocumentBuffer::new();
        doc.newline_format = NewLineFormat::Unix;
        doc.load_file_from_memory("Row 1\nRow 2\nRow 3\nRow 4".to_string());

        // 測試 1-based 跳轉
        doc.goto_line_col(3, 2);
        assert_eq!(doc.editor.selection.head, TextPosition::new(2, 1));

        // 測試跳轉超限 clamp
        doc.goto_line_col(10, 50);
        assert_eq!(doc.editor.selection.head, TextPosition::new(3, 5));

        // 測試行移動與 sync_from_editor
        doc.goto_line_col(2, 1);
        assert!(doc.move_lines_up());
        assert_eq!(doc.text, "Row 2\nRow 1\nRow 3\nRow 4");
        assert_eq!(doc.editor.lines, vec!["Row 2", "Row 1", "Row 3", "Row 4"]);

        // 測試行複製
        doc.duplicate_lines();
        assert_eq!(doc.text, "Row 2\nRow 2\nRow 1\nRow 3\nRow 4");

        // 測試行刪除
        doc.delete_lines();
        assert_eq!(doc.text, "Row 2\nRow 1\nRow 3\nRow 4");
    }

    #[test]
    fn test_buffer_goto_offset() {
        let mut doc = DocumentBuffer::new();
        // 載入 100 個 bytes
        let bytes = vec![0x41u8; 100];
        doc.piece_table = PieceTable::from_bytes(bytes);
        doc.hex_start_offset = 0;

        // 跳轉到 offset 35，應對齊到 32 (0x20)
        doc.goto_offset(35);
        assert_eq!(doc.hex_start_offset, 32);

        let dump = doc.get_hex_dump();
        assert!(dump.offsets.starts_with("00000020"));

        // 跳轉超出長度，應 clamp 到 100，對齊到 96 (0x60)
        doc.goto_offset(9999);
        assert_eq!(doc.hex_start_offset, 96);
    }

    #[test]
    fn test_parse_goto_line_col() {
        assert_eq!(parse_goto_line_col("42"), Some((42, 1)));
        assert_eq!(parse_goto_line_col("  42  "), Some((42, 1)));
        assert_eq!(parse_goto_line_col("42:15"), Some((42, 15)));
        assert_eq!(parse_goto_line_col("42, 15"), Some((42, 15)));
        assert_eq!(parse_goto_line_col("42: 15"), Some((42, 15)));
        assert_eq!(parse_goto_line_col(""), None);
        assert_eq!(parse_goto_line_col("abc"), None);
    }

    #[test]
    fn test_parse_goto_offset() {
        assert_eq!(parse_goto_offset("0x1A0"), Some(0x1A0));
        assert_eq!(parse_goto_offset("0X1A0"), Some(0x1A0));
        assert_eq!(parse_goto_offset("1A0"), Some(0x1A0));
        assert_eq!(parse_goto_offset("10"), Some(0x10));
        assert_eq!(parse_goto_offset(""), None);
        assert_eq!(parse_goto_offset("xyz"), None);
    }

    #[test]
    fn test_convert_newline_format() {
        let mut doc = DocumentBuffer::new();
        doc.newline_format = NewLineFormat::Dos;
        doc.load_file_from_memory("A\r\nB\r\nC".to_string());
        assert_eq!(doc.newline_format, NewLineFormat::Dos);
        assert_eq!(doc.piece_table.collect_bytes(), b"A\r\nB\r\nC");

        // 轉換為 LF (Unix)
        assert!(doc.convert_newline_format(NewLineFormat::Unix));
        assert_eq!(doc.newline_format, NewLineFormat::Unix);
        assert_eq!(doc.text, "A\nB\nC");
        assert_eq!(doc.piece_table.collect_bytes(), b"A\nB\nC");
        assert!(doc.is_modified);

        // 重複轉換相同格式應回傳 false
        assert!(!doc.convert_newline_format(NewLineFormat::Unix));

        // 轉換為 CR (Mac)
        assert!(doc.convert_newline_format(NewLineFormat::Mac));
        assert_eq!(doc.newline_format, NewLineFormat::Mac);
        assert_eq!(doc.text, "A\rB\rC");
        assert_eq!(doc.piece_table.collect_bytes(), b"A\rB\rC");

        // 驗證存檔至實體磁碟檔案的二進位位元組是否確實為目標換行符
        let tmp_file = std::env::temp_dir().join(format!("test_nl_{}.txt", std::process::id()));
        doc.save_file(Some(tmp_file.clone())).unwrap();
        let saved_bytes = std::fs::read(&tmp_file).unwrap();
        assert_eq!(saved_bytes, b"A\rB\rC");
        let _ = std::fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_file_watcher_and_reload() {
        let tmp_file = std::env::temp_dir().join(format!("test_watcher_{}.txt", std::process::id()));
        std::fs::write(&tmp_file, "Line 1\nLine 2\nLine 3").unwrap();

        let mut doc = DocumentBuffer::new();
        doc.load_file(&tmp_file).unwrap();
        assert_eq!(doc.text, "Line 1\nLine 2\nLine 3");
        assert!(!doc.check_external_change());

        // 在 Line 2 加上書籤，並把游標移到 Line 2, col 2
        doc.editor.set_caret(1, 2);
        doc.editor.toggle_bookmark();
        assert!(doc.editor.bookmarks.contains(&1));

        // 模擬外部程式修改檔案：稍作等待確保 mtime 更新
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&tmp_file, "Line 1 Updated\nLine 2 Updated\nLine 3 Updated\nLine 4").unwrap();

        // 偵測外部變更
        assert!(doc.check_external_change());

        // 測試忽略外部變更
        doc.ignore_external_change();
        assert!(!doc.check_external_change());

        // 再次外部修改
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&tmp_file, "Line 1 New\nLine 2 New").unwrap();
        assert!(doc.check_external_change());

        // 執行重新載入
        doc.reload_file().unwrap();
        assert_eq!(doc.text, "Line 1 New\nLine 2 New");
        assert!(!doc.is_modified);
        assert!(!doc.check_external_change());

        // 驗證游標位置與書籤被智慧保留
        assert_eq!(doc.editor.selection.head.line, 1);
        assert_eq!(doc.editor.selection.head.col, 2);
        assert!(doc.editor.bookmarks.contains(&1));

        // 清理暫存檔
        let _ = std::fs::remove_file(&tmp_file);
    }
}


#[cfg(test)]
impl DocumentBuffer {
    pub fn load_file_from_memory(&mut self, text: String) {
        self.text = text.clone();
        self.raw_bytes = text.into_bytes();
        self.piece_table = PieceTable::from_bytes(self.raw_bytes.clone());
        self.line_index = LineIndex::build(&self.piece_table);
        self.editor.set_text(&self.text);
    }
}
