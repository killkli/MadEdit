//! 十六進位模式的位元組緩衝區、游標與復原紀錄

use super::piece_table::PieceTable;

/// 單筆位元組修改（供復原／重做）
#[derive(Debug, Clone)]
struct HexEdit {
    offset: usize,
    old: Vec<u8>,
    new: Vec<u8>,
    caret_before: usize,
    caret_after: usize,
    serial_before: u64,
    serial_after: u64,
}

/// 十六進位模式的資料與游標狀態
pub struct HexBuffer {
    pub data: PieceTable,
    /// 游標所在位元組位移
    pub caret: usize,
    /// 游標是否位於該位元組的低半位（第二個十六進位數字）
    pub low_nibble: bool,
    /// 選取錨點（None 表示無選取）
    pub anchor: Option<usize>,
    /// 游標位於右側文字欄（否則位於十六進位欄）
    pub in_text_pane: bool,
    /// 捲動：最上方顯示的列（每列 16 位元組）
    pub top_row: usize,
    /// 覆寫模式（Hex 模式預設覆寫；插入模式下輸入會插入新位元組）
    pub overwrite: bool,
    undo: Vec<HexEdit>,
    redo: Vec<HexEdit>,
    /// 目前狀態序號與存檔時序號，用於判斷是否有未存檔修改
    serial: u64,
    next_serial: u64,
    saved_serial: u64,
}

pub const BYTES_PER_ROW: usize = 16;

impl HexBuffer {
    pub fn new(data: PieceTable) -> Self {
        Self {
            data,
            caret: 0,
            low_nibble: false,
            anchor: None,
            in_text_pane: false,
            top_row: 0,
            overwrite: true,
            undo: Vec::new(),
            redo: Vec::new(),
            serial: 0,
            next_serial: 1,
            saved_serial: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// 是否有未存檔修改
    pub fn is_modified(&self) -> bool {
        self.serial != self.saved_serial
    }

    /// 是否曾經修改過內容（不論是否已復原到原狀，供離開 Hex 模式時判斷是否需要回寫文字）
    pub fn has_edits(&self) -> bool {
        !self.undo.is_empty() || !self.redo.is_empty()
    }

    pub fn mark_saved(&mut self) {
        self.saved_serial = self.serial;
    }

    /// 選取範圍 [start, end)
    pub fn selection(&self) -> Option<(usize, usize)> {
        let a = self.anchor?;
        let (s, e) = if a <= self.caret { (a, self.caret) } else { (self.caret, a) };
        // 游標所在位元組也算在選取內，與原版行為一致
        let e = (e + 1).min(self.len());
        (s < e).then_some((s, e))
    }

    /// 移動游標；extend 為真時延伸選取
    pub fn move_to(&mut self, offset: usize, extend: bool) {
        let max = self.len();
        let target = offset.min(max);
        if extend {
            if self.anchor.is_none() {
                self.anchor = Some(self.caret);
            }
        } else {
            self.anchor = None;
        }
        self.caret = target;
        self.low_nibble = false;
    }

    fn apply(&mut self, offset: usize, old_len: usize, new: Vec<u8>, caret_after: usize) {
        let old = self.data.read_slice(offset, old_len);
        if old == new {
            return;
        }
        self.data.delete(offset, old_len);
        self.data.insert(offset, &new);
        let serial_after = self.next_serial;
        self.next_serial += 1;
        let edit = HexEdit {
            offset,
            old,
            new,
            caret_before: self.caret,
            caret_after,
            serial_before: self.serial,
            serial_after,
        };
        self.serial = serial_after;
        self.caret = caret_after.min(self.len());
        self.undo.push(edit);
        self.redo.clear();
    }

    /// 在十六進位欄輸入一個十六進位數字
    pub fn type_hex_digit(&mut self, digit: u8) {
        let caret = self.caret;
        let current = if caret < self.len() && (self.overwrite || self.low_nibble) {
            self.data.read_slice(caret, 1)[0]
        } else {
            0
        };
        let value = if self.low_nibble {
            (current & 0xF0) | digit
        } else {
            (digit << 4) | (current & 0x0F)
        };
        let replace_len = if caret < self.len() && (self.overwrite || self.low_nibble) { 1 } else { 0 };
        let (caret_after, low_after) = if self.low_nibble { (caret + 1, false) } else { (caret, true) };
        self.anchor = None;
        self.apply(caret, replace_len, vec![value], caret_after);
        self.caret = caret_after.min(self.len());
        self.low_nibble = low_after;
    }

    /// 在文字欄輸入位元組（已依文件編碼轉換）
    pub fn type_bytes(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if let Some((s, e)) = self.selection() {
            self.anchor = None;
            self.caret = s;
            self.apply(s, e - s, bytes.to_vec(), s + bytes.len());
            return;
        }
        let caret = self.caret;
        let replace_len = if self.overwrite { bytes.len().min(self.len().saturating_sub(caret)) } else { 0 };
        self.apply(caret, replace_len, bytes.to_vec(), caret + bytes.len());
        self.low_nibble = false;
    }

    /// 刪除選取範圍或游標處位元組（backspace 為真時刪除前一個位元組）
    pub fn delete(&mut self, backspace: bool) {
        if let Some((s, e)) = self.selection() {
            self.anchor = None;
            self.caret = s;
            self.apply(s, e - s, Vec::new(), s);
            return;
        }
        if backspace {
            if self.caret == 0 {
                return;
            }
            let at = self.caret - 1;
            self.caret = at + 1;
            self.apply(at, 1, Vec::new(), at);
        } else if self.caret < self.len() {
            let at = self.caret;
            self.apply(at, 1, Vec::new(), at);
        }
        self.low_nibble = false;
    }

    /// 以 data 取代選取範圍（或插入於游標處）
    pub fn paste(&mut self, data: &[u8]) {
        let (s, e) = self.selection().unwrap_or((self.caret, self.caret));
        let replace = if self.anchor.is_none() && self.overwrite {
            data.len().min(self.len().saturating_sub(s))
        } else {
            e - s
        };
        self.anchor = None;
        self.caret = s;
        self.apply(s, replace, data.to_vec(), s + data.len());
    }

    /// 選取範圍內的位元組
    pub fn selected_bytes(&self) -> Vec<u8> {
        match self.selection() {
            Some((s, e)) => self.data.read_slice(s, e - s),
            None => Vec::new(),
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(edit) = self.undo.pop() else { return false };
        self.data.delete(edit.offset, edit.new.len());
        self.data.insert(edit.offset, &edit.old);
        self.caret = edit.caret_before.min(self.len());
        self.anchor = None;
        self.low_nibble = false;
        self.serial = edit.serial_before;
        self.redo.push(edit);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(edit) = self.redo.pop() else { return false };
        self.data.delete(edit.offset, edit.old.len());
        self.data.insert(edit.offset, &edit.new);
        self.caret = edit.caret_after.min(self.len());
        self.anchor = None;
        self.low_nibble = false;
        self.serial = edit.serial_after;
        self.undo.push(edit);
        true
    }

    /// 總列數（至少 1 列，供空檔案顯示游標）
    pub fn row_count(&self) -> usize {
        self.len() / BYTES_PER_ROW + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_typing_overwrite_and_undo() {
        let mut hb = HexBuffer::new(PieceTable::from_bytes(vec![0x00, 0x11, 0x22]));
        hb.type_hex_digit(0xA);
        hb.type_hex_digit(0xB);
        assert_eq!(hb.data.collect_bytes(), vec![0xAB, 0x11, 0x22]);
        assert_eq!(hb.caret, 1);
        assert!(hb.is_modified());
        hb.undo();
        hb.undo();
        assert_eq!(hb.data.collect_bytes(), vec![0x00, 0x11, 0x22]);
        hb.redo();
        assert_eq!(hb.data.collect_bytes(), vec![0xA0, 0x11, 0x22]);
    }

    #[test]
    fn test_hex_insert_delete_and_selection() {
        let mut hb = HexBuffer::new(PieceTable::from_bytes(b"ABCD".to_vec()));
        hb.overwrite = false;
        hb.move_to(2, false);
        hb.type_bytes(b"xy");
        assert_eq!(hb.data.collect_bytes(), b"ABxyCD");
        hb.move_to(1, false);
        hb.move_to(2, true);
        assert_eq!(hb.selected_bytes(), b"Bx");
        hb.delete(false);
        assert_eq!(hb.data.collect_bytes(), b"AyCD");
        hb.move_to(4, false);
        hb.delete(true);
        assert_eq!(hb.data.collect_bytes(), b"AyC");
    }

    #[test]
    fn test_modified_flag_after_save_and_undo() {
        let mut hb = HexBuffer::new(PieceTable::from_bytes(vec![1, 2]));
        hb.type_hex_digit(0xF);
        hb.mark_saved();
        assert!(!hb.is_modified());
        hb.undo();
        assert!(hb.is_modified());
        hb.redo();
        assert!(!hb.is_modified());
    }
}
