/// 編輯器的三種檢視與操作模式：文字模式、列塊模式、十六進位模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMode {
    Text = 0,
    Column = 1,
    Hex = 2,
}

/// 文件行尾換行字元格式（CRLF / LF / CR）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewLineFormat {
    Dos,  // CRLF (Windows)
    Unix, // LF (Unix/macOS)
    Mac,  // CR (Classic Mac)
}

impl NewLineFormat {
    pub fn newline_str(&self) -> &'static str {
        match self {
            NewLineFormat::Dos => "\r\n",
            NewLineFormat::Unix => "\n",
            NewLineFormat::Mac => "\r",
        }
    }
}
