use std::fs;
use std::path::{Path, PathBuf};
use encoding_rs::Encoding;

use super::editor::EditorState;
use super::hex::{generate_hex_dump, HexDump};
use super::model::{EditMode, NewLineFormat};

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

/// 單一開啟或未命名文件的核心狀態容器（DocumentBuffer）
pub struct DocumentBuffer {
    pub file_path: Option<PathBuf>,
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
        Self {
            file_path: None,
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

    /// 從磁碟讀取檔案並自動偵測換行格式與字元編碼（優先識別 BOM，其次嚴格 UTF-8，否則 fallback 至 Big5）
    pub fn load_file<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<()> {
        let p = path.as_ref().to_path_buf();
        let bytes = fs::read(&p)?;

        self.file_path = Some(p);
        self.newline_format = NewLineFormat::detect(&bytes);

        if let Some((bom_enc, bom_len)) = Encoding::for_bom(&bytes) {
            let (decoded, had_errors) = bom_enc.decode_without_bom_handling(&bytes[bom_len..]);
            self.text = decoded.into_owned();
            self.encoding = bom_enc;
            self.has_bom = true;
            self.has_decode_error = had_errors;
        } else if let Some(utf8_text) =
            encoding_rs::UTF_8.decode_without_bom_handling_and_without_replacement(&bytes)
        {
            self.text = utf8_text.into_owned();
            self.encoding = encoding_rs::UTF_8;
            self.has_bom = false;
            self.has_decode_error = false;
        } else {
            let (decoded, had_errors) = encoding_rs::BIG5.decode_without_bom_handling(&bytes);
            self.text = decoded.into_owned();
            self.encoding = encoding_rs::BIG5;
            self.has_bom = false;
            self.has_decode_error = had_errors;
        }

        self.raw_bytes = bytes;
        self.is_modified = false;
        self.editor.set_text(&self.text);

        Ok(())
    }

    /// 儲存檔案至指定路徑或原路徑；若當前存在無損解碼錯誤（has_decode_error），拒絕存檔以防止毀損原檔
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

        // 在文字或列塊模式下，依當前編碼與 BOM 設定將文字重新編碼為位元組
        if self.edit_mode != EditMode::Hex {
            self.raw_bytes = Self::encode_str(&self.text, self.encoding, self.has_bom);
        }

        fs::write(&p, &self.raw_bytes)?;
        self.file_path = Some(p);
        self.is_modified = false;

        Ok(())
    }

    /// 將 editor 中的最新行內容同步至 text 與 raw_bytes
    pub fn sync_from_editor(&mut self) {
        let new_text = self.editor.to_text(self.newline_format.newline_str());
        self.text = new_text;
        self.is_modified = true;
        if !self.has_decode_error {
            self.raw_bytes = Self::encode_str(&self.text, self.encoding, self.has_bom);
        }
    }

    /// 當使用者在外部或模式切換修改內容時同步更新文字與底層位元組
    #[allow(dead_code)]
    pub fn update_text(&mut self, new_text: String) {
        if self.text != new_text {
            self.text = new_text;
            self.is_modified = true;
            if !self.has_decode_error {
                self.raw_bytes = Self::encode_str(&self.text, self.encoding, self.has_bom);
            }
            self.editor.set_text(&self.text);
        }
    }

    /// 切換編輯模式（Text / Column / Hex）
    pub fn set_edit_mode(&mut self, mode: EditMode) {
        self.edit_mode = mode;
        if mode == EditMode::Hex && !self.has_decode_error {
            self.raw_bytes = Self::encode_str(&self.text, self.encoding, self.has_bom);
        }
    }

    /// 重新解碼（Reinterpret Encoding）：保留底層 raw_bytes 不變，改以指定編碼重新解碼出 text，不變更修改狀態
    pub fn reinterpret_with_encoding(&mut self, encoding_label: &str) -> bool {
        let Some(enc) = Encoding::for_label(encoding_label.as_bytes()) else {
            return false;
        };
        let (slice, has_bom) = match Encoding::for_bom(&self.raw_bytes) {
            Some((bom_enc, bom_len)) if bom_enc == enc => (&self.raw_bytes[bom_len..], true),
            _ => (&self.raw_bytes[..], false),
        };
        let (decoded, had_errors) = enc.decode_without_bom_handling(slice);
        self.text = decoded.into_owned();
        self.encoding = enc;
        self.has_bom = has_bom;
        self.has_decode_error = had_errors;
        self.editor.set_text(&self.text);
        true
    }

    /// 轉換編碼（Convert Encoding）：保留當前 text 不變，轉換為目標編碼並更新 raw_bytes，標記為已修改
    pub fn convert_to_encoding(&mut self, encoding_label: &str) -> bool {
        let Some(enc) = Encoding::for_label(encoding_label.as_bytes()) else {
            return false;
        };
        // 轉換為 UTF-16LE / UTF-16BE 時預設寫入 BOM；若原本即為含 BOM 之 UTF-8 則保留 BOM
        self.has_bom = enc == encoding_rs::UTF_16LE
            || enc == encoding_rs::UTF_16BE
            || (enc == encoding_rs::UTF_8 && self.encoding == encoding_rs::UTF_8 && self.has_bom);
        self.encoding = enc;
        self.raw_bytes = Self::encode_str(&self.text, enc, self.has_bom);
        self.has_decode_error = false;
        self.is_modified = true;
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

    /// 產生十六進位檢視區內容（預設最多預覽前 1MB 位元組）
    pub fn get_hex_dump(&self) -> HexDump {
        generate_hex_dump(&self.raw_bytes, 1024 * 1024)
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

    #[test]
    fn test_buffer_update_text() {
        let mut doc = DocumentBuffer::new();
        doc.update_text("Hello MadEdit".to_string());
        assert_eq!(doc.text, "Hello MadEdit");
        assert_eq!(doc.raw_bytes, b"Hello MadEdit");
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
        assert_eq!(doc.raw_gb_bytes(), &raw_gb_bytes[..]);
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
}

#[cfg(test)]
impl DocumentBuffer {
    pub fn raw_gb_bytes(&self) -> &[u8] {
        &self.raw_bytes
    }
    pub fn load_file_from_memory(&mut self, text: String) {
        self.text = text.clone();
        self.raw_bytes = text.into_bytes();
        self.editor.set_text(&self.text);
    }
}
