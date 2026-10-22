use std::collections::BTreeSet;
use super::syntax::HighlightSpan;

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

/// 選取範圍種類（連續串流選取或二維矩形列塊選取）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionKind {
    #[default]
    Stream,
    Block,
}

/// 矩形選取的二維視覺幾何邊界
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualBounds {
    pub min_line: usize,
    pub max_line: usize,
    pub min_vcol: usize,
    pub max_vcol: usize,
}

impl VisualBounds {
    pub fn new(min_line: usize, max_line: usize, min_vcol: usize, max_vcol: usize) -> Self {
        Self { min_line, max_line, min_vcol, max_vcol }
    }

    pub fn line_count(&self) -> usize {
        self.max_line - self.min_line + 1
    }

    pub fn col_width(&self) -> usize {
        self.max_vcol.saturating_sub(self.min_vcol)
    }

    pub fn is_vertical_line(&self) -> bool {
        self.min_vcol == self.max_vcol && self.max_line > self.min_line
    }
}

/// 文件中的文字選取範圍（由錨點 anchor、游標頭部 head 與選取種類組成）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selection {
    pub anchor: TextPosition,
    pub head: TextPosition,
    pub kind: SelectionKind,
    pub anchor_vcol: Option<usize>,
    pub head_vcol: Option<usize>,
}

impl Selection {
    pub fn is_block(&self) -> bool {
        self.kind == SelectionKind::Block
    }

    #[allow(dead_code)]
    pub fn block(
        anchor: TextPosition,
        head: TextPosition,
        anchor_vcol: usize,
        head_vcol: usize,
    ) -> Self {
        Self {
            anchor,
            head,
            kind: SelectionKind::Block,
            anchor_vcol: Some(anchor_vcol),
            head_vcol: Some(head_vcol),
        }
    }

    /// 計算矩形選取的二維邊界範圍
    pub fn visual_bounds(&self, editor: &EditorState) -> VisualBounds {
        let min_line = self.anchor.line.min(self.head.line);
        let max_line = self.anchor.line.max(self.head.line);

        let v1 = self.anchor_vcol.unwrap_or_else(|| {
            let line_text = editor.lines.get(self.anchor.line).map(|s| s.as_str()).unwrap_or("");
            char_col_to_visual_col(line_text, self.anchor.col)
        });
        let v2 = self.head_vcol.unwrap_or_else(|| {
            let line_text = editor.lines.get(self.head.line).map(|s| s.as_str()).unwrap_or("");
            char_col_to_visual_col(line_text, self.head.col)
        });

        let min_vcol = v1.min(v2);
        let max_vcol = v1.max(v2);
        VisualBounds::new(min_line, max_line, min_vcol, max_vcol)
    }
    pub fn new(anchor: TextPosition, head: TextPosition) -> Self {
        Self {
            anchor,
            head,
            kind: SelectionKind::Stream,
            anchor_vcol: None,
            head_vcol: None,
        }
    }

    pub fn caret(pos: TextPosition) -> Self {
        Self {
            anchor: pos,
            head: pos,
            kind: SelectionKind::Stream,
            anchor_vcol: None,
            head_vcol: None,
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

/// 從單行文字中，根據視覺欄位區間 [min_vcol, max_vcol] 擷取子字串與對應字元起訖索引 (start_char, end_char)
/// 若行末短於 max_vcol 且 pad_spaces 為 true，則會補齊空白字元以維持矩形形狀
pub fn extract_visual_slice(
    line: &str,
    min_vcol: usize,
    max_vcol: usize,
) -> (String, usize, usize) {
    let mut current_vcol = 0;
    let mut start_char = None;
    let mut end_char = None;
    let mut result = String::new();

    for (i, c) in line.chars().enumerate() {
        let char_cols = if c == '\t' {
            let next_tab = ((current_vcol / 4) + 1) * 4;
            next_tab - current_vcol
        } else {
            char_visual_columns(c)
        };

        let next_vcol = current_vcol + char_cols;

        // 判斷該字元是否與 [min_vcol, max_vcol) 重疊
        if next_vcol > min_vcol && current_vcol < max_vcol {
            if start_char.is_none() {
                start_char = Some(i);
            }
            end_char = Some(i + 1);
            result.push(c);
        }

        current_vcol = next_vcol;
    }

    let char_count = line.chars().count();
    let s_char = start_char.unwrap_or(char_count);
    let e_char = end_char.unwrap_or(s_char);

    (result, s_char, e_char)
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

/// 核心編輯狀態：管理行清單快取、選取狀態、游標導覽、書籤與基礎編輯邏輯
#[derive(Debug, Clone)]
pub struct EditorState {
    pub lines: Vec<String>,
    pub selection: Selection,
    pub viewport: Viewport,
    pub clipboard: String,
    pub bookmarks: BTreeSet<usize>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            lines: vec![String::new()],
            selection: Selection::default(),
            viewport: Viewport::default(),
            clipboard: String::new(),
            bookmarks: BTreeSet::new(),
        }
    }
}

/// 供 UI 繪製的單一可見行資料
#[derive(Debug, Clone, PartialEq)]
pub struct VisibleLineData {
    pub line_number: i32,
    pub text: String,
    pub spans: Vec<HighlightSpan>,
    pub has_selection: bool,
    pub sel_start_x: f32,
    pub sel_width: f32,
    pub has_caret: bool,
    pub caret_x: f32,
    pub has_bookmark: bool,
}

#[allow(dead_code)]
impl EditorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 更新列塊矩形選取的頭部位置與目標視覺欄位
    pub fn update_block_selection(&mut self, head_pos: TextPosition, target_vcol: usize) {
        let anchor = self.selection.anchor;
        let anchor_vcol = self.selection.anchor_vcol.unwrap_or_else(|| {
            let line_text = self.lines.get(anchor.line).map(|s| s.as_str()).unwrap_or("");
            char_col_to_visual_col(line_text, anchor.col)
        });
        self.selection = Selection::block(anchor, head_pos, anchor_vcol, target_vcol);
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

    /// 智慧行首（首次按下 Home 移動至縮排後第一個非空白字元，再次按下時移動至欄位 0）
    pub fn move_home_smart(&mut self, expand_selection: bool) {
        let head = self.selection.head;
        let line_text = self.lines.get(head.line).map(|s| s.as_str()).unwrap_or("");
        let first_non_ws = line_text.chars().position(|c| !c.is_whitespace());

        let target_col = match first_non_ws {
            Some(idx) => {
                if head.col > idx {
                    idx
                } else if head.col == idx {
                    0
                } else {
                    idx
                }
            }
            None => 0,
        };
        self.update_caret(TextPosition::new(head.line, target_col), expand_selection);
    }

    /// 跳轉至指定的行號與欄位（0-based，具備自動 clamp 與視窗滾動）
    pub fn goto_line_col(&mut self, line: usize, col: usize) {
        self.set_caret(line, col);
    }

    /// 將當前行或選取行整塊往上移動一行
    pub fn move_lines_up(&mut self) -> bool {
        if self.lines.is_empty() {
            return false;
        }
        let min_line = self.selection.start().line;
        let max_line = self.selection.end().line;
        if min_line == 0 {
            return false;
        }

        // 把 min_line - 1 這行抽出，插入到 max_line 的位置
        let removed = self.lines.remove(min_line - 1);
        self.lines.insert(max_line, removed);

        // 書籤同步移動
        let target = min_line - 1;
        let mut new_bookmarks = BTreeSet::new();
        for &bm in &self.bookmarks {
            if bm == target {
                new_bookmarks.insert(max_line);
            } else if bm >= min_line && bm <= max_line {
                new_bookmarks.insert(bm - 1);
            } else {
                new_bookmarks.insert(bm);
            }
        }
        self.bookmarks = new_bookmarks;

        self.selection.anchor.line -= 1;
        self.selection.head.line -= 1;
        self.clamp_caret();
        self.scroll_caret_into_view();
        true
    }

    /// 將當前行或選取行整塊往下移動一行
    pub fn move_lines_down(&mut self) -> bool {
        if self.lines.is_empty() {
            return false;
        }
        let min_line = self.selection.start().line;
        let max_line = self.selection.end().line;
        if max_line + 1 >= self.lines.len() {
            return false;
        }

        // 把 max_line + 1 這行抽出，插入到 min_line 的位置
        let removed = self.lines.remove(max_line + 1);
        self.lines.insert(min_line, removed);

        // 書籤同步移動
        let target = max_line + 1;
        let mut new_bookmarks = BTreeSet::new();
        for &bm in &self.bookmarks {
            if bm == target {
                new_bookmarks.insert(min_line);
            } else if bm >= min_line && bm <= max_line {
                new_bookmarks.insert(bm + 1);
            } else {
                new_bookmarks.insert(bm);
            }
        }
        self.bookmarks = new_bookmarks;

        self.selection.anchor.line += 1;
        self.selection.head.line += 1;
        self.clamp_caret();
        self.scroll_caret_into_view();
        true
    }

    /// 當自指定行開始刪除了 count 行時，同步校正書籤集合
    fn adjust_bookmarks_after_removal(&mut self, start_line: usize, count: usize) {
        if count == 0 {
            return;
        }
        let end_line = start_line + count - 1;
        let mut new_bookmarks = BTreeSet::new();
        for &bm in &self.bookmarks {
            if bm < start_line {
                new_bookmarks.insert(bm);
            } else if bm > end_line {
                new_bookmarks.insert(bm - count);
            }
        }
        self.bookmarks = new_bookmarks;
    }

    /// 當自指定行後插入了 count 行時，同步校正書籤集合
    fn adjust_bookmarks_after_insertion(&mut self, after_line: usize, count: usize) {
        if count == 0 {
            return;
        }
        let mut new_bookmarks = BTreeSet::new();
        for &bm in &self.bookmarks {
            if bm > after_line {
                new_bookmarks.insert(bm + count);
            } else {
                new_bookmarks.insert(bm);
            }
        }
        self.bookmarks = new_bookmarks;
    }

    /// 複製當前行或選取區涵蓋的所有行至下方
    pub fn duplicate_lines(&mut self) {
        if self.lines.is_empty() {
            return;
        }
        let min_line = self.selection.start().line;
        let max_line = self.selection.end().line;

        let cloned_lines: Vec<String> = self.lines[min_line..=max_line].to_vec();
        let delta = max_line - min_line + 1;

        for (offset, line_str) in cloned_lines.into_iter().enumerate() {
            self.lines.insert(max_line + 1 + offset, line_str);
        }

        // 書籤同步位移
        self.adjust_bookmarks_after_insertion(max_line, delta);

        // 游標與選區移動至新複製出的區塊
        self.selection.anchor.line += delta;
        self.selection.head.line += delta;
        self.clamp_caret();
        self.scroll_caret_into_view();
    }

    /// 刪除當前行或選取區涵蓋的所有行
    pub fn delete_lines(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
            self.selection = Selection::caret(TextPosition::new(0, 0));
            self.bookmarks.clear();
            return;
        }
        let min_line = self.selection.start().line;
        let max_line = self.selection.end().line;

        self.lines.drain(min_line..=max_line);
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }

        // 書籤同步移除與前移
        let count = max_line - min_line + 1;
        self.adjust_bookmarks_after_removal(min_line, count);

        let new_line = min_line.min(self.lines.len() - 1);
        let col = self.selection.head.col.min(self.line_char_count(new_line));
        self.selection = Selection::caret(TextPosition::new(new_line, col));
        self.clamp_caret();
        self.scroll_caret_into_view();
    }

    /// 切換游標所在行的書籤狀態（新增或移除），回傳切換後該行是否包含書籤
    pub fn toggle_bookmark(&mut self) -> bool {
        let line = self.selection.head.line;
        if self.bookmarks.contains(&line) {
            self.bookmarks.remove(&line);
            false
        } else {
            self.bookmarks.insert(line);
            true
        }
    }

    /// 環狀跳轉至下一個書籤行（若無則回傳 false）
    pub fn next_bookmark(&mut self) -> bool {
        if self.bookmarks.is_empty() {
            return false;
        }
        let cur_line = self.selection.head.line;
        let next_line = self
            .bookmarks
            .range((cur_line + 1)..)
            .next()
            .copied()
            .or_else(|| self.bookmarks.iter().next().copied());

        if let Some(target) = next_line {
            self.set_caret(target, 0);
            true
        } else {
            false
        }
    }

    /// 環狀跳轉至上一個書籤行（若無則回傳 false）
    pub fn prev_bookmark(&mut self) -> bool {
        if self.bookmarks.is_empty() {
            return false;
        }
        let cur_line = self.selection.head.line;
        let prev_line = self
            .bookmarks
            .range(..cur_line)
            .next_back()
            .copied()
            .or_else(|| self.bookmarks.iter().next_back().copied());

        if let Some(target) = prev_line {
            self.set_caret(target, 0);
            true
        } else {
            false
        }
    }

    /// 清空所有書籤
    pub fn clear_all_bookmarks(&mut self) {
        self.bookmarks.clear();
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
        if self.selection.is_block() {
            let bounds = self.selection.visual_bounds(self);
            let mut result = Vec::new();
            for line_idx in bounds.min_line..=bounds.max_line {
                if let Some(line) = self.lines.get(line_idx) {
                    let (slice, _, _) = extract_visual_slice(line, bounds.min_vcol, bounds.max_vcol);
                    result.push(slice);
                }
            }
            return result.join("\n");
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
        if self.selection.is_block() {
            let bounds = self.selection.visual_bounds(self);
            for line_idx in bounds.min_line..=bounds.max_line {
                if line_idx < self.lines.len() {
                    let (_, start_c, end_c) = extract_visual_slice(&self.lines[line_idx], bounds.min_vcol, bounds.max_vcol);
                    if end_c > start_c {
                        let mut chars: Vec<char> = self.lines[line_idx].chars().collect();
                        let end_bound = end_c.min(chars.len());
                        let start_bound = start_c.min(end_bound);
                        chars.drain(start_bound..end_bound);
                        self.lines[line_idx] = chars.into_iter().collect();
                    }
                }
            }
            let head_line = self.selection.head.line.min(self.lines.len().saturating_sub(1));
            let anchor_line = self.selection.anchor.line.min(self.lines.len().saturating_sub(1));
            let head_char = visual_col_to_char_col(
                self.lines.get(head_line).map(|s| s.as_str()).unwrap_or(""),
                bounds.min_vcol,
            );
            let anchor_char = visual_col_to_char_col(
                self.lines.get(anchor_line).map(|s| s.as_str()).unwrap_or(""),
                bounds.min_vcol,
            );
            self.selection = Selection::block(
                TextPosition::new(anchor_line, anchor_char),
                TextPosition::new(head_line, head_char),
                bounds.min_vcol,
                bounds.min_vcol,
            );
            self.clamp_caret();
            self.scroll_caret_into_view();
            return true;
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
            self.adjust_bookmarks_after_removal(start.line + 1, end.line - start.line);
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
        if self.selection.is_block() {
            let bounds = self.selection.visual_bounds(self);
            if !self.selection.is_empty() {
                self.delete_selection();
            }
            for line_idx in bounds.min_line..=bounds.max_line {
                if line_idx < self.lines.len() {
                    let line_text = &self.lines[line_idx];
                    let char_col = visual_col_to_char_col(line_text, bounds.min_vcol);
                    let mut chars: Vec<char> = line_text.chars().collect();
                    let insert_pos = char_col.min(chars.len());
                    chars.insert(insert_pos, c);
                    self.lines[line_idx] = chars.into_iter().collect();
                }
            }
            let next_vcol = bounds.min_vcol + char_visual_columns(c);
            let head_line = self.selection.head.line.min(self.lines.len().saturating_sub(1));
            let anchor_line = self.selection.anchor.line.min(self.lines.len().saturating_sub(1));
            let head_char = visual_col_to_char_col(
                self.lines.get(head_line).map(|s| s.as_str()).unwrap_or(""),
                next_vcol,
            );
            let anchor_char = visual_col_to_char_col(
                self.lines.get(anchor_line).map(|s| s.as_str()).unwrap_or(""),
                next_vcol,
            );
            self.selection = Selection::block(
                TextPosition::new(anchor_line, anchor_char),
                TextPosition::new(head_line, head_char),
                next_vcol,
                next_vcol,
            );
            self.scroll_caret_into_view();
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
        for c in text.chars() {
            if c == '\r' {
                continue;
            }
            self.insert_char(c);
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

        // 書籤同步後移
        self.adjust_bookmarks_after_insertion(head.line, 1);

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
            self.adjust_bookmarks_after_removal(head.line, 1);
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
            self.adjust_bookmarks_after_removal(head.line + 1, 1);
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
        if self.clipboard.is_empty() {
            return;
        }
        if self.selection.is_block() {
            let bounds = self.selection.visual_bounds(self);
            if !self.selection.is_empty() {
                self.delete_selection();
            }
            let clip_lines: Vec<&str> = self.clipboard.split('\n').map(|s| s.trim_end_matches('\r')).collect();
            let target_line_count = if clip_lines.len() > bounds.line_count() {
                clip_lines.len()
            } else {
                bounds.line_count()
            };

            for idx in 0..target_line_count {
                let line_idx = bounds.min_line + idx;
                while line_idx >= self.lines.len() {
                    self.lines.push(String::new());
                }
                if idx < clip_lines.len() {
                    let text_to_insert = clip_lines[idx];
                    let line_text = &self.lines[line_idx];
                    let char_col = visual_col_to_char_col(line_text, bounds.min_vcol);
                    let mut chars: Vec<char> = line_text.chars().collect();
                    let insert_pos = char_col.min(chars.len());
                    for (ci, ch) in text_to_insert.chars().enumerate() {
                        chars.insert(insert_pos + ci, ch);
                    }
                    self.lines[line_idx] = chars.into_iter().collect();
                }
            }
            let final_line = bounds.min_line + target_line_count.saturating_sub(1);
            let head_char = visual_col_to_char_col(
                self.lines.get(final_line).map(|s| s.as_str()).unwrap_or(""),
                bounds.min_vcol,
            );
            self.selection = Selection::caret(TextPosition::new(final_line, head_char));
            self.scroll_caret_into_view();
            return;
        }
        let text = self.clipboard.clone();
        self.insert_text(&text);
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
        let is_block = self.selection.is_block();
        let (sel_start, sel_end) = if has_sel && !is_block {
            (self.selection.start(), self.selection.end())
        } else {
            (TextPosition::default(), TextPosition::default())
        };

        let bounds = if is_block {
            self.selection.visual_bounds(self)
        } else {
            VisualBounds::new(0, 0, 0, 0)
        };

        for (i, line_text) in visible_texts.iter().enumerate() {
            let line_idx = start_line + i;
            let line_number = (line_idx + 1) as i32;

            // 游標判斷：在列塊模式或垂直游標線下，覆蓋每行游標繪製
            let is_in_block_lines = is_block && line_idx >= bounds.min_line && line_idx <= bounds.max_line;
            let has_caret = if is_in_block_lines && (bounds.is_vertical_line() || !self.selection.is_empty()) {
                true
            } else {
                self.selection.head.line == line_idx
            };

            let caret_x = if has_caret {
                let vcol = if is_block {
                    self.selection.head_vcol.unwrap_or_else(|| {
                        char_col_to_visual_col(line_text, self.selection.head.col)
                    })
                } else {
                    char_col_to_visual_col(line_text, self.selection.head.col)
                };
                vcol as f32 * cell_width
            } else {
                0.0
            };

            // 選取範圍判斷
            let mut line_has_sel = false;
            let mut sel_start_x = 0.0;
            let mut sel_width = 0.0;

            if has_sel && is_block {
                if line_idx >= bounds.min_line && line_idx <= bounds.max_line && bounds.max_vcol > bounds.min_vcol {
                    line_has_sel = true;
                    sel_start_x = bounds.min_vcol as f32 * cell_width;
                    sel_width = (bounds.max_vcol - bounds.min_vcol) as f32 * cell_width;
                }
            } else if has_sel && line_idx >= sel_start.line && line_idx <= sel_end.line {
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
                spans: Vec::new(),
                has_selection: line_has_sel && sel_width > 0.0,
                sel_start_x,
                sel_width,
                has_caret,
                caret_x,
                has_bookmark: self.bookmarks.contains(&line_idx),
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

    #[test]
    fn test_extract_visual_slice() {
        let line = "Hello 世界\tEnd";

        // 擷取 "Hello" (0..5)
        let (s1, start1, end1) = extract_visual_slice(line, 0, 5);
        assert_eq!(s1, "Hello");
        assert_eq!(start1, 0);
        assert_eq!(end1, 5);

        // 擷取 "世界" (6..10)
        let (s2, start2, end2) = extract_visual_slice(line, 6, 10);
        assert_eq!(s2, "世界");
        assert_eq!(start2, 6);
        assert_eq!(end2, 8);

        // 擷取短行超出範圍
        let short_line = "ABC";
        let (s3, start3, end3) = extract_visual_slice(short_line, 2, 6);
        assert_eq!(s3, "C");
        assert_eq!(start3, 2);
        assert_eq!(end3, 3);
    }

    #[test]
    fn test_block_selection_and_rendering() {
        let mut editor = EditorState::from_text("123456789\n123456789\n123456789");
        editor.selection = Selection::block(
            TextPosition::new(0, 2),
            TextPosition::new(2, 6),
            2,
            6,
        );
        assert!(editor.selection.is_block());
        let bounds = editor.selection.visual_bounds(&editor);
        assert_eq!(bounds.min_line, 0);
        assert_eq!(bounds.max_line, 2);
        assert_eq!(bounds.min_vcol, 2);
        assert_eq!(bounds.max_vcol, 6);

        // 驗證矩形可見行渲染
        let visible = editor.generate_visible_lines(10.0);
        assert_eq!(visible.len(), 3);
        for line in &visible {
            assert!(line.has_selection);
            assert_eq!(line.sel_start_x, 20.0);
            assert_eq!(line.sel_width, 40.0);
            assert!(line.has_caret); // 矩形選取覆蓋多行游標
        }
    }

    #[test]
    fn test_block_editing_operations() {
        let mut editor = EditorState::from_text("ABCDEF\nGHIJKL\nMNOPQR");
        editor.selection = Selection::block(
            TextPosition::new(0, 2),
            TextPosition::new(2, 4),
            2,
            4,
        );

        // 1. 驗證矩形複製
        let copied = editor.copy_to_clipboard();
        assert_eq!(copied, "CD\nIJ\nOP");

        // 2. 驗證矩形刪除
        editor.delete_selection();
        assert_eq!(editor.lines, vec!["ABEF", "GHKL", "MNQR"]);

        // 3. 驗證多行矩形連續同步打字（不退化為單行）
        editor.insert_char('X');
        editor.insert_char('Y');
        assert_eq!(editor.lines, vec!["ABXYEF", "GHXYKL", "MNXYQR"]);

        // 4. 驗證列塊貼上：將剛才複製的 "CD\nIJ\nOP" 貼入第 1 欄
        editor.selection = Selection::block(
            TextPosition::new(0, 1),
            TextPosition::new(0, 1),
            1,
            1,
        );
        editor.paste_from_clipboard();
        assert_eq!(editor.lines, vec!["ACDBXYEF", "GIJHXYKL", "MOPNXYQR"]);
    }

    #[test]
    fn test_goto_line_col() {
        let mut editor = EditorState::from_text("line1\nline2\nline3\nline4");
        editor.goto_line_col(2, 3);
        assert_eq!(editor.selection.head, TextPosition::new(2, 3));
        assert_eq!(editor.selection.anchor, TextPosition::new(2, 3));

        // 測試超出最大行數時 clamp
        editor.goto_line_col(10, 2);
        assert_eq!(editor.selection.head, TextPosition::new(3, 2));

        // 測試超出最大欄位時 clamp
        editor.goto_line_col(1, 100);
        assert_eq!(editor.selection.head, TextPosition::new(1, 5));
    }

    #[test]
    fn test_move_lines_up_and_down() {
        let mut editor = EditorState::from_text("Line A\nLine B\nLine C\nLine D");
        
        // 游標在第 1 行 (Line B)
        editor.set_caret(1, 2);
        assert!(editor.move_lines_up());
        assert_eq!(editor.lines, vec!["Line B", "Line A", "Line C", "Line D"]);
        assert_eq!(editor.selection.head, TextPosition::new(0, 2));

        // 已經在首行，無法再上移
        assert!(!editor.move_lines_up());

        // 往下移動
        assert!(editor.move_lines_down());
        assert_eq!(editor.lines, vec!["Line A", "Line B", "Line C", "Line D"]);
        assert_eq!(editor.selection.head, TextPosition::new(1, 2));

        // 測試多行選取整塊移動
        editor.selection = Selection::new(TextPosition::new(1, 0), TextPosition::new(2, 6));
        assert!(editor.move_lines_down());
        assert_eq!(editor.lines, vec!["Line A", "Line D", "Line B", "Line C"]);
        assert_eq!(editor.selection.anchor.line, 2);
        assert_eq!(editor.selection.head.line, 3);
    }

    #[test]
    fn test_duplicate_lines() {
        let mut editor = EditorState::from_text("First\nSecond\nThird");
        
        // 單行無選取複製
        editor.set_caret(1, 3);
        editor.duplicate_lines();
        assert_eq!(editor.lines, vec!["First", "Second", "Second", "Third"]);
        assert_eq!(editor.selection.head, TextPosition::new(2, 3));

        // 多行選取複製
        editor.selection = Selection::new(TextPosition::new(0, 0), TextPosition::new(1, 6));
        editor.duplicate_lines();
        assert_eq!(editor.lines, vec!["First", "Second", "First", "Second", "Second", "Third"]);
    }

    #[test]
    fn test_delete_lines() {
        let mut editor = EditorState::from_text("Row 1\nRow 2\nRow 3\nRow 4");
        
        // 單行刪除
        editor.set_caret(1, 2);
        editor.delete_lines();
        assert_eq!(editor.lines, vec!["Row 1", "Row 3", "Row 4"]);
        assert_eq!(editor.selection.head, TextPosition::new(1, 2));

        // 多行選取刪除
        editor.selection = Selection::new(TextPosition::new(0, 0), TextPosition::new(1, 5));
        editor.delete_lines();
        assert_eq!(editor.lines, vec!["Row 4"]);
        assert_eq!(editor.selection.head.line, 0);

        // 刪除最後剩餘行：保證 lines 不為空
        editor.delete_lines();
        assert_eq!(editor.lines, vec![""]);
        assert_eq!(editor.selection.head, TextPosition::new(0, 0));
    }

    #[test]
    fn test_move_home_smart() {
        let mut editor = EditorState::from_text("    let x = 10;");
        
        // 游標在行末 (col 15)，第一次按 Home 應跳到第一個非空白字元 (col 4)
        editor.set_caret(0, 15);
        editor.move_home_smart(false);
        assert_eq!(editor.selection.head, TextPosition::new(0, 4));

        // 游標已在第一個非空白字元 (col 4)，第二次按 Home 應跳到第 0 欄
        editor.move_home_smart(false);
        assert_eq!(editor.selection.head, TextPosition::new(0, 0));

        // 游標在第 0 欄，再次按 Home 應跳回第一個非空白字元 (col 4)
        editor.move_home_smart(false);
        assert_eq!(editor.selection.head, TextPosition::new(0, 4));
    }

    #[test]
    fn test_bookmarks_toggle_and_navigation() {
        let mut editor = EditorState::from_text("Line 0\nLine 1\nLine 2\nLine 3\nLine 4");
        
        // 切換書籤
        editor.set_caret(1, 0);
        assert!(editor.toggle_bookmark()); // 新增 line 1
        assert!(editor.bookmarks.contains(&1));

        editor.set_caret(3, 0);
        assert!(editor.toggle_bookmark()); // 新增 line 3
        assert_eq!(editor.bookmarks.len(), 2);

        // 再次切換 line 1 應移除
        editor.set_caret(1, 0);
        assert!(!editor.toggle_bookmark()); // 移除 line 1
        assert!(!editor.bookmarks.contains(&1));
        assert_eq!(editor.bookmarks.len(), 1);

        // 加回 line 1 與 line 4
        editor.set_caret(1, 0);
        editor.toggle_bookmark();
        editor.set_caret(4, 0);
        editor.toggle_bookmark();
        // 現在有 line 1, 3, 4

        // 測試 next_bookmark 環狀導覽
        editor.set_caret(0, 0);
        assert!(editor.next_bookmark());
        assert_eq!(editor.selection.head.line, 1);

        assert!(editor.next_bookmark());
        assert_eq!(editor.selection.head.line, 3);

        assert!(editor.next_bookmark());
        assert_eq!(editor.selection.head.line, 4);

        // 環狀回到 line 1
        assert!(editor.next_bookmark());
        assert_eq!(editor.selection.head.line, 1);

        // 測試 prev_bookmark 環狀導覽
        assert!(editor.prev_bookmark());
        assert_eq!(editor.selection.head.line, 4);

        assert!(editor.prev_bookmark());
        assert_eq!(editor.selection.head.line, 3);

        // 清除所有書籤
        editor.clear_all_bookmarks();
        assert!(editor.bookmarks.is_empty());
        assert!(!editor.next_bookmark());
        assert!(!editor.prev_bookmark());
    }

    #[test]
    fn test_bookmarks_maintenance_on_line_editing() {
        let mut editor = EditorState::from_text("Row 0\nRow 1\nRow 2\nRow 3\nRow 4");
        // 在 Row 1 與 Row 3 加上書籤
        editor.set_caret(1, 0);
        editor.toggle_bookmark();
        editor.set_caret(3, 0);
        editor.toggle_bookmark();

        // 1. 在 Row 0 處斷行（Enter），後續書籤應平移至 2 與 4
        editor.set_caret(0, 2);
        editor.insert_newline();
        assert!(editor.bookmarks.contains(&2));
        assert!(editor.bookmarks.contains(&4));
        assert_eq!(editor.bookmarks.len(), 2);

        // 2. 移動行（將 Row 2 上移），書籤應跟隨移動至 1
        editor.set_caret(2, 0);
        editor.move_lines_up();
        assert!(editor.bookmarks.contains(&1));
        assert!(editor.bookmarks.contains(&4));

        // 3. 刪除包含書籤的行（刪除 Row 1 與 Row 2），Row 4 的書籤應前移
        editor.selection = Selection::new(TextPosition::new(1, 0), TextPosition::new(2, 5));
        editor.delete_lines();
        // 原 Row 1 被刪除，原 Row 4 (減 2) 變成了 Row 2
        assert!(!editor.bookmarks.contains(&1));
        assert!(editor.bookmarks.contains(&2));
        assert_eq!(editor.bookmarks.len(), 1);

        // 4. 測試行首按 Backspace 合併行
        let mut ed2 = EditorState::from_text("First\nSecond\nThird");
        ed2.set_caret(2, 0); // 在 Third 加上書籤
        ed2.toggle_bookmark();
        assert!(ed2.bookmarks.contains(&2));

        ed2.set_caret(1, 0); // 游標在 Second 行首按 Backspace 合併到 First
        ed2.delete_backspace();
        assert_eq!(ed2.lines, vec!["FirstSecond", "Third"]);
        // 原本在 line 2 的 Third 書籤應前移至 line 1
        assert!(ed2.bookmarks.contains(&1));
        assert_eq!(ed2.bookmarks.len(), 1);

        // 5. 測試行尾按 Delete 合併行
        ed2.set_caret(0, ed2.line_char_count(0)); // 在 FirstSecond 行尾按 Delete 合併 Third
        ed2.delete_forward();
        assert_eq!(ed2.lines, vec!["FirstSecondThird"]);
        // 合併後 Third 行被刪除，書籤自動清理
        assert!(ed2.bookmarks.is_empty());
    }
}
