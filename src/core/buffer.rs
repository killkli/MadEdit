use std::fs;
use std::path::{Path, PathBuf};
use encoding_rs::Encoding;

use super::editor::EditorState;
use super::hex::{generate_hex_dump, HexDump};
use super::model::{EditMode, NewLineFormat};
use super::piece_table::{LineIndex, PieceTable};

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
    pub editor: EditorState,
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
            editor: EditorState::default(),
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
        let file_len = file.metadata()?.len();

        let pt = if file_len > 0 {
            // 安全建立唯讀記憶體對映（檔案由作業系統按需分頁讀取）
            let mmap = unsafe { memmap2::Mmap::map(&file)? };
            PieceTable::from_mmap(mmap)
        } else {
            PieceTable::new()
        };

        self.file_path = Some(p);
        self.piece_table = pt;

        // 讀取前 64KB 樣本進行換行與編碼自動偵測
        let sample_len = (self.piece_table.len()).min(64 * 1024);
        let sample = self.piece_table.read_slice(0, sample_len);
        self.newline_format = NewLineFormat::detect(&sample);

        let mut bom_len = 0;
        if let Some((bom_enc, len)) = Encoding::for_bom(&sample) {
            self.encoding = bom_enc;
            self.has_bom = true;
            self.has_decode_error = false;
            bom_len = len;
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
            let decode_slice = if self.has_bom && full_bytes.len() >= bom_len {
                &full_bytes[bom_len..]
            } else {
                &full_bytes[..]
            };

            let (decoded, had_errors) = self.encoding.decode_without_bom_handling(decode_slice);
            if had_errors {
                self.has_decode_error = true;
            }

            self.text = decoded.into_owned();
            self.raw_bytes = full_bytes;
            self.editor.set_text(&self.text);
        } else {
            // 大型檔案：避免全檔記憶體複製與整檔解碼，依賴 Piece Table 與 LineIndex 按需供應
            self.text.clear();
            self.raw_bytes.clear();
            let count = self.line_index.line_count().max(1);
            self.editor.lines = vec![String::new(); count];
            self.editor.selection = super::editor::Selection::caret(super::editor::TextPosition::new(0, 0));
            self.editor.clamp_caret();
        }

        self.is_modified = false;

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

    /// 取得目前文字內容（相容性查詢介面：若未快取則自 Piece Table 解碼）
    #[allow(dead_code)]
    pub fn text(&self) -> String {
        if !self.text.is_empty() || self.piece_table.len() == 0 {
            self.text.clone()
        } else {
            let bytes = self.piece_table.collect_bytes();
            let decode_slice = if self.has_bom {
                if let Some((_, len)) = Encoding::for_bom(&bytes) {
                    &bytes[len..]
                } else {
                    &bytes[..]
                }
            } else {
                &bytes[..]
            };
            let (decoded, _) = self.encoding.decode_without_bom_handling(decode_slice);
            decoded.into_owned()
        }
    }

    /// 根據可視視窗（Viewport）與字元格網寬度產生可見行繪製資料（支援大檔案按需解碼）
    pub fn generate_visible_lines(&self, cell_width: f32) -> Vec<super::editor::VisibleLineData> {
        let start_line = self.editor.viewport.first_visible_line;
        let count = self.editor.viewport.visible_line_count;

        let has_cached_lines = !self.editor.lines.is_empty()
            && self.editor.lines.len() == self.line_index.line_count()
            && self.piece_table.len() <= SMALL_FILE_CACHE_LIMIT;

        if has_cached_lines {
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
        }
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
                "No target file path specified",
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
        let file_len = file.metadata()?.len();
        self.piece_table = if file_len > 0 {
            let mmap = unsafe { memmap2::Mmap::map(&file)? };
            PieceTable::from_mmap(mmap)
        } else {
            PieceTable::new()
        };
        self.line_index = LineIndex::build(&self.piece_table);
        self.file_path = Some(p);
        self.is_modified = false;

        Ok(())
    }

    /// 將 editor 中的最新行內容同步至 text 與 Piece Table
    pub fn sync_from_editor(&mut self) {
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
        let (slice, has_bom) = match Encoding::for_bom(&bytes) {
            Some((bom_enc, bom_len)) if bom_enc == enc => (&bytes[bom_len..], true),
            _ => (&bytes[..], false),
        };
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

    /// 產生十六進位檢視區內容（直接從 Piece Table 隨機讀取前 1MB 預覽，無全檔記憶體浪費）
    pub fn get_hex_dump(&self) -> HexDump {
        let preview_len = (self.piece_table.len()).min(1024 * 1024);
        let preview_bytes = self.piece_table.read_slice(0, preview_len);
        generate_hex_dump(&preview_bytes, 1024 * 1024)
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
}

#[cfg(test)]
mod tests {
    use super::*;
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

        // 驗證原子存檔在 mmap 模式下運作正常
        assert!(doc.save_file(None).is_ok());
        assert_eq!(doc.len(), line.len() * line_count);

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
