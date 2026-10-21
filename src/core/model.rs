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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewLineFormat {
    Dos,  // CRLF
    Unix, // LF
    Mac,  // CR
}

impl NewLineFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            NewLineFormat::Dos => "CRLF (Windows)",
            NewLineFormat::Unix => "LF (Unix/macOS)",
            NewLineFormat::Mac => "CR (Classic Mac)",
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
}
