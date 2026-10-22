/// 文字在文件中的 0-based 邏輯座標（行索引與字元索引）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct TextPosition {
    pub line: usize,
    pub col: usize,
}

impl TextPosition {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

/// 文件中的文字選取範圍（由錨點 anchor 與游標頭部 head 組成）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selection {
    pub anchor: TextPosition,
    pub head: TextPosition,
}

impl Selection {
    pub fn new(anchor: TextPosition, head: TextPosition) -> Self {
        Self { anchor, head }
    }

    pub fn caret(pos: TextPosition) -> Self {
        Self {
            anchor: pos,
            head: pos,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    pub fn start(&self) -> TextPosition {
        self.anchor.min(self.head)
    }

    pub fn end(&self) -> TextPosition {
        self.anchor.max(self.head)
    }
}

/// 編輯器可視視窗範圍
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub struct Viewport {
    pub first_visible_line: usize,
    pub visible_line_count: usize,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            first_visible_line: 0,
            visible_line_count: 30,
        }
    }
}

/// 判斷單一字元的視覺欄寬（ASCII 半形為 1，CJK 全形字元為 2，控制字元為 1）
pub fn char_visual_columns(c: char) -> usize {
    if c == '\t' {
        return 4;
    }
    let u = c as u32;
    if (0x2E80..=0x9FFF).contains(&u)
        || (0xAC00..=0xD7AF).contains(&u)
        || (0xF900..=0xFAFF).contains(&u)
        || (0xFE10..=0xFE1F).contains(&u)
        || (0xFE30..=0xFE6F).contains(&u)
        || (0xFF00..=0xFF60).contains(&u)
        || (0xFFE0..=0xFFE6).contains(&u)
        || (0x20000..=0x2FA1F).contains(&u)
    {
        2
    } else {
        1
    }
}

/// 計算字串中指定字元索引（char_idx）在畫面上的視覺欄位位置（考慮 CJK 雙寬與 Tab 展開至 4 欄倍數）
pub fn char_col_to_visual_col(line: &str, char_idx: usize) -> usize {
    let mut visual_col = 0;
    for (i, c) in line.chars().enumerate() {
        if i >= char_idx {
            break;
        }
        if c == '\t' {
            let next_tab = ((visual_col / 4) + 1) * 4;
            visual_col = next_tab;
        } else {
            visual_col += char_visual_columns(c);
        }
    }
    visual_col
}

/// 根據畫面上的視覺欄位位置（visual_col）反查對應的字元索引（char_idx）
pub fn visual_col_to_char_col(line: &str, target_visual_col: usize) -> usize {
    let mut current_visual_col = 0;
    for (i, c) in line.chars().enumerate() {
        if current_visual_col >= target_visual_col {
            return i;
        }
        let cols = if c == '\t' {
            let next_tab = ((current_visual_col / 4) + 1) * 4;
            next_tab - current_visual_col
        } else {
            char_visual_columns(c)
        };
        if current_visual_col + cols > target_visual_col {
            return i;
        }
        current_visual_col += cols;
    }
    line.chars().count()
}

/// 將字串中的 Tab 字元依格網展開為相應數量的空白字元，確保畫面字元寬度與游標格網嚴格對齊
pub fn expand_tabs(line: &str, tab_size: usize) -> String {
    let mut out = String::new();
    let mut visual_col = 0;
    for c in line.chars() {
        if c == '\t' {
            let next_tab = ((visual_col / tab_size) + 1) * tab_size;
            let spaces = next_tab - visual_col;
            for _ in 0..spaces {
                out.push(' ');
            }
            visual_col = next_tab;
        } else {
            visual_col += char_visual_columns(c);
            out.push(c);
        }
    }
    out
}

/// 核心編輯狀態：管理行清單快取、選取狀態、游標導覽與基礎編輯邏輯
#[derive(Debug, Clone)]
pub struct EditorState {
    pub lines: Vec<String>,
    pub selection: Selection,
    pub viewport: Viewport,
    pub clipboard: String,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            lines: vec![String::new()],
            selection: Selection::default(),
            viewport: Viewport::default(),
            clipboard: String::new(),
        }
    }
}

/// 供 UI 繪製的單一可見行資料
#[derive(Debug, Clone, PartialEq)]
pub struct VisibleLineData {
    pub line_number: i32,
    pub text: String,
    pub has_selection: bool,
    pub sel_start_x: f32,
    pub sel_width: f32,
    pub has_caret: bool,
    pub caret_x: f32,
}

#[allow(dead_code)]
impl EditorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 從整篇文字內容初始化行列表
    pub fn from_text(text: &str) -> Self {
        let mut state = Self::default();
        state.set_text(text);
        state
    }

    /// 設定文字並重新切分行
    pub fn set_text(&mut self, text: &str) {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let mut lines: Vec<String> = normalized.split('\n').map(|s| s.to_string()).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        self.lines = lines;
        self.selection = Selection::caret(TextPosition::new(0, 0));
        self.clamp_caret();
    }

    /// 組合目前所有行成單一字串（使用指定換行符號）
    pub fn to_text(&self, newline: &str) -> String {
        self.lines.join(newline)
    }

    /// 行數
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// 指定行長度（字元數）
    pub fn line_char_count(&self, line_idx: usize) -> usize {
        self.lines.get(line_idx).map_or(0, |l| l.chars().count())
    }

    /// 確保游標與選取錨點在有效範圍內
    fn clamp_pos(&self, pos: TextPosition) -> TextPosition {
        if self.lines.is_empty() {
            return TextPosition::new(0, 0);
        }
        let line = pos.line.min(self.lines.len() - 1);
        let col = pos.col.min(self.line_char_count(line));
        TextPosition::new(line, col)
    }

    pub fn clamp_caret(&mut self) {
        self.selection.anchor = self.clamp_pos(self.selection.anchor);
        self.selection.head = self.clamp_pos(self.selection.head);
    }

    /// 設定游標位置並清除選取
    pub fn set_caret(&mut self, line: usize, col: usize) {
        let pos = self.clamp_pos(TextPosition::new(line, col));
        self.selection = Selection::caret(pos);
        self.scroll_caret_into_view();
    }

    /// 更新游標位置（expand_selection 為 true 時維持 anchor 擴展選取）
    fn update_caret(&mut self, new_pos: TextPosition, expand_selection: bool) {
        let pos = self.clamp_pos(new_pos);
        if expand_selection {
            self.selection.head = pos;
        } else {
            self.selection = Selection::caret(pos);
        }
        self.scroll_caret_into_view();
    }

    pub fn move_left(&mut self, expand_selection: bool) {
        let head = self.selection.head;
        if head.col > 0 {
            self.update_caret(TextPosition::new(head.line, head.col - 1), expand_selection);
        } else if head.line > 0 {
            let prev_line = head.line - 1;
            let col = self.line_char_count(prev_line);
            self.update_caret(TextPosition::new(prev_line, col), expand_selection);
        }
    }

    pub fn move_right(&mut self, expand_selection: bool) {
        let head = self.selection.head;
        let line_len = self.line_char_count(head.line);
        if head.col < line_len {
            self.update_caret(TextPosition::new(head.line, head.col + 1), expand_selection);
        } else if head.line + 1 < self.lines.len() {
            self.update_caret(TextPosition::new(head.line + 1, 0), expand_selection);
        }
    }

    pub fn move_up(&mut self, expand_selection: bool) {
        let head = self.selection.head;
        if head.line > 0 {
            self.update_caret(TextPosition::new(head.line - 1, head.col), expand_selection);
        } else {
            self.update_caret(TextPosition::new(0, 0), expand_selection);
        }
    }

    pub fn move_down(&mut self, expand_selection: bool) {
        let head = self.selection.head;
        if head.line + 1 < self.lines.len() {
            self.update_caret(TextPosition::new(head.line + 1, head.col), expand_selection);
        } else {
            let last_line = self.lines.len() - 1;
            let last_col = self.line_char_count(last_line);
            self.update_caret(TextPosition::new(last_line, last_col), expand_selection);
        }
    }

    pub fn move_home(&mut self, expand_selection: bool) {
        let head = self.selection.head;
        self.update_caret(TextPosition::new(head.line, 0), expand_selection);
    }

    pub fn move_end(&mut self, expand_selection: bool) {
        let head = self.selection.head;
        let col = self.line_char_count(head.line);
        self.update_caret(TextPosition::new(head.line, col), expand_selection);
    }

    pub fn move_page_up(&mut self, expand_selection: bool) {
        let page_size = self.viewport.visible_line_count.max(1);
        let head = self.selection.head;
        let target_line = head.line.saturating_sub(page_size);
        self.update_caret(TextPosition::new(target_line, head.col), expand_selection);
    }

    pub fn move_page_down(&mut self, expand_selection: bool) {
        let page_size = self.viewport.visible_line_count.max(1);
        let head = self.selection.head;
        let target_line = (head.line + page_size).min(self.lines.len() - 1);
        self.update_caret(TextPosition::new(target_line, head.col), expand_selection);
    }

    pub fn select_all(&mut self) {
        if self.lines.is_empty() {
            return;
        }
        let last_line = self.lines.len() - 1;
        let last_col = self.line_char_count(last_line);
        self.selection = Selection::new(
            TextPosition::new(0, 0),
            TextPosition::new(last_line, last_col),
        );
    }

    /// 取得選取範圍內的文字字串
    pub fn selected_text(&self) -> String {
        if self.selection.is_empty() {
            return String::new();
        }
        let start = self.selection.start();
        let end = self.selection.end();

        if start.line == end.line {
            let line = &self.lines[start.line];
            let chars: Vec<char> = line.chars().collect();
            chars[start.col..end.col].iter().collect()
        } else {
            let mut result = String::new();
            let line0: Vec<char> = self.lines[start.line].chars().collect();
            result.push_str(&line0[start.col..].iter().collect::<String>());
            result.push('\n');

            for line_idx in (start.line + 1)..end.line {
                result.push_str(&self.lines[line_idx]);
                result.push('\n');
            }

            let line_end: Vec<char> = self.lines[end.line].chars().collect();
            result.push_str(&line_end[..end.col].iter().collect::<String>());
            result
        }
    }

    /// 刪除選取範圍；若有選取並刪除成功回傳 true
    pub fn delete_selection(&mut self) -> bool {
        if self.selection.is_empty() {
            return false;
        }
        let start = self.selection.start();
        let end = self.selection.end();

        if start.line == end.line {
            let chars: Vec<char> = self.lines[start.line].chars().collect();
            let mut new_line: String = chars[..start.col].iter().collect();
            new_line.push_str(&chars[end.col..].iter().collect::<String>());
            self.lines[start.line] = new_line;
        } else {
            let start_chars: Vec<char> = self.lines[start.line].chars().collect();
            let end_chars: Vec<char> = self.lines[end.line].chars().collect();

            let mut merged: String = start_chars[..start.col].iter().collect();
            merged.push_str(&end_chars[end.col..].iter().collect::<String>());

            self.lines[start.line] = merged;
            self.lines.drain((start.line + 1)..=end.line);
        }

        self.selection = Selection::caret(start);
        self.clamp_caret();
        self.scroll_caret_into_view();
        true
    }

    /// 在游標位置插入字元（若有選取區則先刪除選取）
    pub fn insert_char(&mut self, c: char) {
        if c == '\n' {
            self.insert_newline();
            return;
        }
        self.delete_selection();
        let head = self.selection.head;
        let mut chars: Vec<char> = self.lines[head.line].chars().collect();
        chars.insert(head.col, c);
        self.lines[head.line] = chars.into_iter().collect();

        let new_pos = TextPosition::new(head.line, head.col + 1);
        self.selection = Selection::caret(new_pos);
        self.scroll_caret_into_view();
    }

    /// 在游標位置插入字串
    pub fn insert_text(&mut self, text: &str) {
        self.delete_selection();
        for c in text.chars() {
            if c == '\r' {
                continue;
            }
            if c == '\n' {
                self.insert_newline();
            } else {
                self.insert_char(c);
            }
        }
    }

    /// 在游標處斷行（Enter）
    pub fn insert_newline(&mut self) {
        self.delete_selection();
        let head = self.selection.head;
        let chars: Vec<char> = self.lines[head.line].chars().collect();

        let left: String = chars[..head.col].iter().collect();
        let right: String = chars[head.col..].iter().collect();

        self.lines[head.line] = left;
        self.lines.insert(head.line + 1, right);

        let new_pos = TextPosition::new(head.line + 1, 0);
        self.selection = Selection::caret(new_pos);
        self.scroll_caret_into_view();
    }

    /// Backspace 刪除鍵處理
    pub fn delete_backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        let head = self.selection.head;
        if head.col > 0 {
            let mut chars: Vec<char> = self.lines[head.line].chars().collect();
            chars.remove(head.col - 1);
            self.lines[head.line] = chars.into_iter().collect();

            let new_pos = TextPosition::new(head.line, head.col - 1);
            self.selection = Selection::caret(new_pos);
        } else if head.line > 0 {
            let current_line = self.lines.remove(head.line);
            let prev_line_idx = head.line - 1;
            let prev_len = self.line_char_count(prev_line_idx);

            self.lines[prev_line_idx].push_str(&current_line);
            self.selection = Selection::caret(TextPosition::new(prev_line_idx, prev_len));
        }
        self.scroll_caret_into_view();
    }

    /// Delete 往前刪除鍵處理
    pub fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        let head = self.selection.head;
        let line_len = self.line_char_count(head.line);

        if head.col < line_len {
            let mut chars: Vec<char> = self.lines[head.line].chars().collect();
            chars.remove(head.col);
            self.lines[head.line] = chars.into_iter().collect();
        } else if head.line + 1 < self.lines.len() {
            let next_line = self.lines.remove(head.line + 1);
            self.lines[head.line].push_str(&next_line);
        }
        self.scroll_caret_into_view();
    }

    /// 複製選取文字至內部剪貼簿
    pub fn copy_to_clipboard(&mut self) -> String {
        let text = self.selected_text();
        if !text.is_empty() {
            self.clipboard = text.clone();
        }
        text
    }

    /// 剪下選取文字至內部剪貼簿並刪除選取
    pub fn cut_to_clipboard(&mut self) -> String {
        let text = self.copy_to_clipboard();
        if !text.is_empty() {
            self.delete_selection();
        }
        text
    }

    /// 從內部剪貼簿貼上文字至游標處
    pub fn paste_from_clipboard(&mut self) {
        if !self.clipboard.is_empty() {
            let text = self.clipboard.clone();
            self.insert_text(&text);
        }
    }

    /// 確保游標在當前視窗範圍內（自動捲動）
    pub fn scroll_caret_into_view(&mut self) {
        let head = self.selection.head;
        let line = head.line;
        if line < self.viewport.first_visible_line {
            self.viewport.first_visible_line = line;
        } else if line >= self.viewport.first_visible_line + self.viewport.visible_line_count {
            self.viewport.first_visible_line = line.saturating_sub(self.viewport.visible_line_count - 1);
        }
    }

    /// 依行數進行畫面垂直捲動
    pub fn scroll_by(&mut self, lines: i32) {
        if lines < 0 {
            self.viewport.first_visible_line =
                self.viewport.first_visible_line.saturating_sub((-lines) as usize);
        } else if lines > 0 {
            let max_first = self.lines.len().saturating_sub(1);
            self.viewport.first_visible_line =
                (self.viewport.first_visible_line + lines as usize).min(max_first);
        }
    }

    /// 根據目前可視視窗（Viewport）與字元格網寬度產生可見行繪製資料
    pub fn generate_visible_lines(&self, cell_width: f32) -> Vec<VisibleLineData> {
        let start_line = self.viewport.first_visible_line;
        let count = self.viewport.visible_line_count;
        let end_line = (start_line + count).min(self.lines.len());
        let slice = if start_line < self.lines.len() {
            &self.lines[start_line..end_line]
        } else {
            &[]
        };
        self.generate_visible_lines_with_text(slice, cell_width)
    }

    /// 根據傳入的可見行文字清單與字元格網寬度產生可見行繪製資料（支援大檔案按需解碼）
    pub fn generate_visible_lines_with_text(
        &self,
        visible_texts: &[String],
        cell_width: f32,
    ) -> Vec<VisibleLineData> {
        let mut result = Vec::new();
        let start_line = self.viewport.first_visible_line;
        let has_sel = !self.selection.is_empty();
        let (sel_start, sel_end) = if has_sel {
            (self.selection.start(), self.selection.end())
        } else {
            (TextPosition::default(), TextPosition::default())
        };

        for (i, line_text) in visible_texts.iter().enumerate() {
            let line_idx = start_line + i;
            let line_number = (line_idx + 1) as i32;

            // 游標判斷
            let has_caret = self.selection.head.line == line_idx;
            let caret_x = if has_caret {
                let vcol = char_col_to_visual_col(line_text, self.selection.head.col);
                vcol as f32 * cell_width
            } else {
                0.0
            };

            // 選取範圍判斷
            let mut line_has_sel = false;
            let mut sel_start_x = 0.0;
            let mut sel_width = 0.0;

            if has_sel && line_idx >= sel_start.line && line_idx <= sel_end.line {
                line_has_sel = true;
                let start_char_col = if line_idx == sel_start.line {
                    sel_start.col
                } else {
                    0
                };
                let end_char_col = if line_idx == sel_end.line {
                    sel_end.col
                } else {
                    line_text.chars().count()
                };

                let start_v = char_col_to_visual_col(line_text, start_char_col);
                let end_v = char_col_to_visual_col(line_text, end_char_col);
                sel_start_x = start_v as f32 * cell_width;
                let width_cols = if end_v > start_v {
                    end_v - start_v
                } else if line_idx < sel_end.line {
                    1
                } else {
                    0
                };
                sel_width = width_cols as f32 * cell_width;
            }

            result.push(VisibleLineData {
                line_number,
                text: expand_tabs(line_text, 4),
                has_selection: line_has_sel && sel_width > 0.0,
                sel_start_x,
                sel_width,
                has_caret,
                caret_x,
            });
        }
        result
    }

    /// 將滑鼠像素座標轉換為文字座標（TextPosition）
    pub fn pos_from_pixel(&self, x: f32, y: f32, cell_width: f32, line_height: f32) -> TextPosition {
        if self.lines.is_empty() {
            return TextPosition::new(0, 0);
        }
        let line_offset = (y / line_height).max(0.0) as usize;
        let line_idx = (self.viewport.first_visible_line + line_offset).min(self.lines.len() - 1);
        let line_text = &self.lines[line_idx];

        let target_vcol = (x / cell_width).max(0.0).round() as usize;
        let char_col = visual_col_to_char_col(line_text, target_vcol);
        self.clamp_pos(TextPosition::new(line_idx, char_col))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_position_and_selection() {
        let p1 = TextPosition::new(0, 5);
        let p2 = TextPosition::new(1, 2);
        let sel = Selection::new(p2, p1);

        assert!(!sel.is_empty());
        assert_eq!(sel.start(), p1);
        assert_eq!(sel.end(), p2);

        let caret = Selection::caret(p1);
        assert!(caret.is_empty());
        assert_eq!(caret.start(), p1);
        assert_eq!(caret.end(), p1);
    }

    #[test]
    fn test_character_cell_grid_width() {
        assert_eq!(char_visual_columns('a'), 1);
        assert_eq!(char_visual_columns('1'), 1);
        assert_eq!(char_visual_columns(' '), 1);

        assert_eq!(char_visual_columns('中'), 2);
        assert_eq!(char_visual_columns('文'), 2);
        assert_eq!(char_visual_columns('，'), 2);
        assert_eq!(char_visual_columns('あ'), 2);
        assert_eq!(char_visual_columns('한'), 2);
    }

    #[test]
    fn test_char_col_to_visual_col_and_roundtrip() {
        let text = "Hello 世界\tEnd";
        assert_eq!(char_col_to_visual_col(text, 0), 0);
        assert_eq!(char_col_to_visual_col(text, 5), 5);
        assert_eq!(char_col_to_visual_col(text, 6), 6);
        assert_eq!(char_col_to_visual_col(text, 7), 8);
        assert_eq!(char_col_to_visual_col(text, 8), 10);
        assert_eq!(char_col_to_visual_col(text, 9), 12);

        assert_eq!(visual_col_to_char_col(text, 0), 0);
        assert_eq!(visual_col_to_char_col(text, 6), 6);
        assert_eq!(visual_col_to_char_col(text, 8), 7);
        assert_eq!(visual_col_to_char_col(text, 10), 8);
        assert_eq!(visual_col_to_char_col(text, 12), 9);
    }

    #[test]
    fn test_editor_navigation_and_editing() {
        let mut editor = EditorState::from_text("Line 1\nLine 2");
        assert_eq!(editor.line_count(), 2);
        assert_eq!(editor.selection.head, TextPosition::new(0, 0));

        // 移動游標
        editor.move_right(false);
        assert_eq!(editor.selection.head, TextPosition::new(0, 1));
        editor.move_end(false);
        assert_eq!(editor.selection.head, TextPosition::new(0, 6));

        // 換行與插入文字
        editor.insert_newline();
        assert_eq!(editor.line_count(), 3);
        assert_eq!(editor.selection.head, TextPosition::new(1, 0));
        editor.insert_text("Inserted");
        assert_eq!(editor.lines[1], "Inserted");

        // 選取與刪除
        editor.select_all();
        assert_eq!(editor.selected_text(), "Line 1\nInserted\nLine 2");
        editor.delete_selection();
        assert_eq!(editor.to_text("\n"), "");
        assert_eq!(editor.line_count(), 1);
    }

    #[test]
    fn test_editor_backspace_and_delete() {
        let mut editor = EditorState::from_text("AB\nCD");
        editor.set_caret(1, 0); // 在 "CD" 行首
        editor.delete_backspace(); // 合併至第一行
        assert_eq!(editor.line_count(), 1);
        assert_eq!(editor.lines[0], "ABCD");
        assert_eq!(editor.selection.head, TextPosition::new(0, 2));

        editor.delete_forward(); // 刪除 'C'
        assert_eq!(editor.lines[0], "ABD");
    }

    #[test]
    fn test_generate_visible_lines_and_pos_from_pixel() {
        let mut editor = EditorState::from_text("First line\nSecond line");
        editor.set_caret(0, 5); // 在 "First" 後面
        let lines = editor.generate_visible_lines(10.0);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].has_caret);
        assert_eq!(lines[0].caret_x, 50.0);
        assert!(!lines[0].has_selection);

        // 建立選取：從 (0, 0) 到 (0, 5)
        editor.selection = Selection::new(TextPosition::new(0, 0), TextPosition::new(0, 5));
        let lines_sel = editor.generate_visible_lines(10.0);
        assert!(lines_sel[0].has_selection);
        assert_eq!(lines_sel[0].sel_start_x, 0.0);
        assert_eq!(lines_sel[0].sel_width, 50.0);

        // 滑鼠像素反查
        let pos = editor.pos_from_pixel(52.0, 5.0, 10.0, 20.0);
        assert_eq!(pos, TextPosition::new(0, 5));
        let pos_line2 = editor.pos_from_pixel(10.0, 25.0, 10.0, 20.0);
        assert_eq!(pos_line2, TextPosition::new(1, 1));
    }

    #[test]
    fn test_expand_tabs_and_clipboard() {
        let raw = "\tA\tB";
        let expanded = expand_tabs(raw, 4);
        assert_eq!(expanded, "    A   B");

        let mut editor = EditorState::from_text("Copy this text");
        editor.select_all();
        let copied = editor.copy_to_clipboard();
        assert_eq!(copied, "Copy this text");
        assert_eq!(editor.clipboard, "Copy this text");

        editor.set_caret(0, 4); // "Copy" 後面
        editor.paste_from_clipboard();
        assert_eq!(editor.lines[0], "CopyCopy this text this text");
    }
}
