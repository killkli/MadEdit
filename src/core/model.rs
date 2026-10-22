/// 編輯器的三種檢視與操作模式：文字模式、列塊模式、十六進位模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMode {
    Text = 0,
    Column = 1,
    Hex = 2,
}

impl From<i32> for EditMode {
    fn from(val: i32) -> Self {
        match val {
            1 => EditMode::Column,
            2 => EditMode::Hex,
            _ => EditMode::Text,
        }
    }
}

/// 文件支援的換行格式選單名稱清單
pub const SUPPORTED_NEWLINES: [&str; 3] = [
    "CRLF (Windows)",
    "LF (Unix/macOS)",
    "CR (Classic Mac)",
];

/// 文件行尾換行字元格式（CRLF / LF / CR）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewLineFormat {
    Dos,  // CRLF (Windows)
    Unix, // LF (Unix/macOS)
    Mac,  // CR (Classic Mac)
}

impl NewLineFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            NewLineFormat::Dos => "CRLF (Windows)",
            NewLineFormat::Unix => "LF (Unix/macOS)",
            NewLineFormat::Mac => "CR (Classic Mac)",
        }
    }

    pub fn newline_str(&self) -> &'static str {
        match self {
            NewLineFormat::Dos => "\r\n",
            NewLineFormat::Unix => "\n",
            NewLineFormat::Mac => "\r",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        let trimmed = label.trim();
        if trimmed.starts_with("CRLF") || trimmed == "Dos" {
            Some(NewLineFormat::Dos)
        } else if trimmed.starts_with("LF") || trimmed == "Unix" {
            Some(NewLineFormat::Unix)
        } else if trimmed.starts_with("CR") || trimmed == "Mac" {
            Some(NewLineFormat::Mac)
        } else {
            None
        }
    }

    pub fn detect(bytes: &[u8]) -> Self {
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'\r' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    return NewLineFormat::Dos;
                }
                return NewLineFormat::Mac;
            } else if b == b'\n' {
                return NewLineFormat::Unix;
            }
        }
        NewLineFormat::Dos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_newline_detection() {
        assert_eq!(NewLineFormat::detect(b"line1\r\nline2"), NewLineFormat::Dos);
        assert_eq!(NewLineFormat::detect(b"line1\nline2"), NewLineFormat::Unix);
        assert_eq!(NewLineFormat::detect(b"line1\rline2"), NewLineFormat::Mac);
    }

    #[test]
    fn test_newline_from_label() {
        assert_eq!(NewLineFormat::from_label("CRLF (Windows)"), Some(NewLineFormat::Dos));
        assert_eq!(NewLineFormat::from_label("CRLF"), Some(NewLineFormat::Dos));
        assert_eq!(NewLineFormat::from_label("LF (Unix/macOS)"), Some(NewLineFormat::Unix));
        assert_eq!(NewLineFormat::from_label("LF"), Some(NewLineFormat::Unix));
        assert_eq!(NewLineFormat::from_label("CR (Classic Mac)"), Some(NewLineFormat::Mac));
        assert_eq!(NewLineFormat::from_label("CR"), Some(NewLineFormat::Mac));
        assert_eq!(NewLineFormat::from_label("Unknown"), None);
    }
}
