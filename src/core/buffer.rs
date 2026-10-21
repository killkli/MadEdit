use std::fs;
use std::path::{Path, PathBuf};
use encoding_rs::Encoding;

use super::hex::{generate_hex_dump, HexDump};
use super::model::{EditMode, NewLineFormat};

pub struct DocumentBuffer {
    pub file_path: Option<PathBuf>,
    pub raw_bytes: Vec<u8>,
    pub text: String,
    pub encoding: &'static Encoding,
    pub newline_format: NewLineFormat,
    pub edit_mode: EditMode,
    pub is_modified: bool,
}

impl Default for DocumentBuffer {
    fn default() -> Self {
        Self {
            file_path: None,
            raw_bytes: Vec::new(),
            text: String::new(),
            encoding: encoding_rs::UTF_8,
            newline_format: NewLineFormat::Dos,
            edit_mode: EditMode::Text,
            is_modified: false,
        }
    }
}

impl DocumentBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_file<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<()> {
        let p = path.as_ref().to_path_buf();
        let bytes = fs::read(&p)?;

        self.file_path = Some(p);
        self.raw_bytes = bytes.clone();
        self.newline_format = NewLineFormat::detect(&bytes);

        // Try decoding with UTF-8 first, fallback to Big5 or GBK if needed or keep encoding
        let (decoded, detected_enc, _) = self.encoding.decode(&bytes);
        self.text = decoded.into_owned();
        self.encoding = detected_enc;
        self.is_modified = false;

        Ok(())
    }

    pub fn save_file(&mut self, path: Option<PathBuf>) -> std::io::Result<()> {
        let target_path = path.or_else(|| self.file_path.clone());
        let Some(p) = target_path else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No target file path specified",
            ));
        };

        // If in text or column mode, encode text back to bytes
        if self.edit_mode != EditMode::Hex {
            let (encoded_bytes, _, _) = self.encoding.encode(&self.text);
            self.raw_bytes = encoded_bytes.into_owned();
        }

        fs::write(&p, &self.raw_bytes)?;
        self.file_path = Some(p);
        self.is_modified = false;

        Ok(())
    }

    pub fn update_text(&mut self, new_text: String) {
        if self.text != new_text {
            self.text = new_text;
            self.is_modified = true;
            // Also update raw_bytes if in text mode
            let (encoded, _, _) = self.encoding.encode(&self.text);
            self.raw_bytes = encoded.into_owned();
        }
    }

    pub fn set_edit_mode(&mut self, mode: EditMode) {
        self.edit_mode = mode;
        if mode == EditMode::Hex {
            // Ensure raw_bytes is synced with text
            let (encoded, _, _) = self.encoding.encode(&self.text);
            self.raw_bytes = encoded.into_owned();
        }
    }

    pub fn get_hex_dump(&self) -> HexDump {
        generate_hex_dump(&self.raw_bytes, 1024 * 1024) // up to 1MB preview
    }

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
}
