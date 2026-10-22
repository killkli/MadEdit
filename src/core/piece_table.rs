use std::io::Write;

/// Piece 片段的資料來源（原始唯讀緩衝區或新增追加緩衝區）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BufferSource {
    Original,
    Add,
}

/// Piece Table 中的單一片段節點
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    pub source: BufferSource,
    pub offset: usize,
    pub length: usize,
}

/// 原始緩衝區容器（支援記憶體位元組或 memmap2 記憶體對映）
pub enum OriginalBuffer {
    Bytes(Vec<u8>),
    Mmap(memmap2::Mmap),
}

impl AsRef<[u8]> for OriginalBuffer {
    fn as_ref(&self) -> &[u8] {
        match self {
            OriginalBuffer::Bytes(b) => b.as_slice(),
            OriginalBuffer::Mmap(m) => m.as_ref(),
        }
    }
}

/// 大檔案編輯核心：Piece Table 資料結構
pub struct PieceTable {
    original: OriginalBuffer,
    add_buffer: Vec<u8>,
    pieces: Vec<Piece>,
    total_len: usize,
}

impl Default for PieceTable {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl PieceTable {
    /// 建立空的 PieceTable
    pub fn new() -> Self {
        Self {
            original: OriginalBuffer::Bytes(Vec::new()),
            add_buffer: Vec::new(),
            pieces: Vec::new(),
            total_len: 0,
        }
    }

    /// 從記憶體位元組序列建立 PieceTable
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        let len = bytes.len();
        let pieces = if len > 0 {
            vec![Piece {
                source: BufferSource::Original,
                offset: 0,
                length: len,
            }]
        } else {
            Vec::new()
        };

        Self {
            original: OriginalBuffer::Bytes(bytes),
            add_buffer: Vec::new(),
            pieces,
            total_len: len,
        }
    }

    /// 從 memmap2 記憶體對映建立 PieceTable（零拷貝大檔開啟）
    pub fn from_mmap(mmap: memmap2::Mmap) -> Self {
        let len = mmap.len();
        let pieces = if len > 0 {
            vec![Piece {
                source: BufferSource::Original,
                offset: 0,
                length: len,
            }]
        } else {
            Vec::new()
        };

        Self {
            original: OriginalBuffer::Mmap(mmap),
            add_buffer: Vec::new(),
            pieces,
            total_len: len,
        }
    }

    /// 目前總位元組長度
    pub fn len(&self) -> usize {
        self.total_len
    }

    pub fn is_empty(&self) -> bool {
        self.total_len == 0
    }

    /// 取得底層對應來源的位元組切片
    fn get_source_slice(&self, source: BufferSource, offset: usize, length: usize) -> &[u8] {
        match source {
            BufferSource::Original => &self.original.as_ref()[offset..offset + length],
            BufferSource::Add => &self.add_buffer[offset..offset + length],
        }
    }

    /// 隨機讀取指定位移與長度的位元組片段
    pub fn read_slice(&self, offset: usize, length: usize) -> Vec<u8> {
        let actual_len = length.min(self.total_len.saturating_sub(offset));
        if actual_len == 0 {
            return Vec::new();
        }

        let mut out = Vec::with_capacity(actual_len);
        let mut curr_offset = 0;
        let end_offset = offset + actual_len;

        for piece in &self.pieces {
            let piece_end = curr_offset + piece.length;
            if piece_end > offset && curr_offset < end_offset {
                let slice_start = offset.saturating_sub(curr_offset);
                let slice_end = (end_offset - curr_offset).min(piece.length);
                let slice_len = slice_end.saturating_sub(slice_start);

                let data = self.get_source_slice(
                    piece.source,
                    piece.offset + slice_start,
                    slice_len,
                );
                out.extend_from_slice(data);
            }
            curr_offset = piece_end;
            if curr_offset >= end_offset {
                break;
            }
        }

        out
    }

    /// 在指定位元組位移（offset）處插入位元組序列
    pub fn insert(&mut self, offset: usize, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        let add_offset = self.add_buffer.len();
        self.add_buffer.extend_from_slice(data);
        let new_piece = Piece {
            source: BufferSource::Add,
            offset: add_offset,
            length: data.len(),
        };

        if self.pieces.is_empty() {
            self.pieces.push(new_piece);
            self.total_len = data.len();
            return;
        }

        let mut curr_offset = 0;
        let mut insert_idx = self.pieces.len();
        let mut split_piece: Option<(Piece, Piece)> = None;

        for (i, piece) in self.pieces.iter().enumerate() {
            let piece_end = curr_offset + piece.length;

            if offset == curr_offset {
                insert_idx = i;
                break;
            } else if offset > curr_offset && offset < piece_end {
                let left_len = offset - curr_offset;
                let right_len = piece.length - left_len;
                let left_piece = Piece {
                    source: piece.source,
                    offset: piece.offset,
                    length: left_len,
                };
                let right_piece = Piece {
                    source: piece.source,
                    offset: piece.offset + left_len,
                    length: right_len,
                };
                split_piece = Some((left_piece, right_piece));
                insert_idx = i;
                break;
            }

            curr_offset = piece_end;
        }

        if let Some((left, right)) = split_piece {
            self.pieces[insert_idx] = left;
            self.pieces.insert(insert_idx + 1, new_piece);
            self.pieces.insert(insert_idx + 2, right);
        } else {
            self.pieces.insert(insert_idx, new_piece);
        }

        self.total_len += data.len();
    }

    /// 在指定位元組位移（offset）處刪除指定長度（length）的位元組
    pub fn delete(&mut self, offset: usize, length: usize) {
        let actual_len = length.min(self.total_len.saturating_sub(offset));
        if actual_len == 0 {
            return;
        }

        let del_start = offset;
        let del_end = offset + actual_len;
        let mut new_pieces = Vec::new();
        let mut curr_offset = 0;

        for piece in &self.pieces {
            let piece_start = curr_offset;
            let piece_end = curr_offset + piece.length;

            if piece_end <= del_start || piece_start >= del_end {
                // 完全在刪除區間外，完整保留
                new_pieces.push(*piece);
            } else {
                // 與刪除區間重疊
                // 保留重疊處前半部
                if piece_start < del_start {
                    let left_len = del_start - piece_start;
                    new_pieces.push(Piece {
                        source: piece.source,
                        offset: piece.offset,
                        length: left_len,
                    });
                }
                // 保留重疊處後半部
                if piece_end > del_end {
                    let right_len = piece_end - del_end;
                    let right_src_offset = piece.offset + (del_end - piece_start);
                    new_pieces.push(Piece {
                        source: piece.source,
                        offset: right_src_offset,
                        length: right_len,
                    });
                }
            }

            curr_offset = piece_end;
        }

        self.pieces = new_pieces;
        self.total_len -= actual_len;
    }

    /// 將全部 Piece 循序收集為一個單部位元組向量（僅建議在中小型檔案使用）
    pub fn collect_bytes(&self) -> Vec<u8> {
        self.read_slice(0, self.total_len)
    }

    /// 串流寫出所有片段至指定的 Writer（用於大檔案無損高速存檔）
    pub fn write_to<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        for piece in &self.pieces {
            let slice = self.get_source_slice(piece.source, piece.offset, piece.length);
            writer.write_all(slice)?;
        }
        Ok(())
    }
}

/// 大檔案行位移索引，支援 O(log N) 行號快速定位與按需解碼
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    line_starts: Vec<usize>,
    total_len: usize,
}

#[allow(dead_code)]
impl LineIndex {
    /// 掃描 PieceTable 中的換行符號建立行索引
    pub fn build(pt: &PieceTable) -> Self {
        let mut line_starts = vec![0];
        let mut curr_offset = 0;

        for piece in &pt.pieces {
            let slice = pt.get_source_slice(piece.source, piece.offset, piece.length);
            for (i, &b) in slice.iter().enumerate() {
                if b == b'\n' {
                    line_starts.push(curr_offset + i + 1);
                }
            }
            curr_offset += piece.length;
        }

        Self {
            line_starts,
            total_len: pt.len(),
        }
    }

    /// 總行數
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// 取得指定行號（0-based）在 Piece Table 中的位元組起訖區間 (start_offset, end_offset)
    pub fn line_byte_range(&self, line_idx: usize) -> Option<(usize, usize)> {
        if line_idx >= self.line_starts.len() {
            return None;
        }
        let start = self.line_starts[line_idx];
        let end = if line_idx + 1 < self.line_starts.len() {
            self.line_starts[line_idx + 1]
        } else {
            self.total_len
        };
        Some((start, end))
    }

    /// 根據位元組位移（offset）透過二分搜尋取得對應行號 (0-based)
    pub fn line_at_offset(&self, offset: usize) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        }
    }

    /// 按需擷取指定範圍的行內容並解碼為文字字串清單（去除行尾換行符號）
    pub fn fetch_lines(
        &self,
        pt: &PieceTable,
        encoding: &'static encoding_rs::Encoding,
        start_line: usize,
        count: usize,
    ) -> Vec<String> {
        let mut lines = Vec::with_capacity(count);
        for i in 0..count {
            let line_idx = start_line + i;
            let Some((start, end)) = self.line_byte_range(line_idx) else {
                break;
            };
            if end >= start {
                let bytes = pt.read_slice(start, end - start);
                let (decoded, _) = encoding.decode_without_bom_handling(&bytes);
                let line_str = decoded.trim_end_matches(['\r', '\n']).to_string();
                lines.push(line_str);
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piece_table_basic_and_insert() {
        let mut pt = PieceTable::from_bytes(b"Hello World".to_vec());
        assert_eq!(pt.len(), 11);
        assert_eq!(pt.collect_bytes(), b"Hello World");

        // 中間插入：在 "Hello " 後面插入 "Beautiful " (offset 6)
        pt.insert(6, b"Beautiful ");
        assert_eq!(pt.len(), 21);
        assert_eq!(pt.collect_bytes(), b"Hello Beautiful World");

        // 開頭插入
        pt.insert(0, b">>> ");
        assert_eq!(pt.collect_bytes(), b">>> Hello Beautiful World");

        // 結尾插入
        let len = pt.len();
        pt.insert(len, b" !!!");
        assert_eq!(pt.collect_bytes(), b">>> Hello Beautiful World !!!");
    }

    #[test]
    fn test_piece_table_delete() {
        let mut pt = PieceTable::from_bytes(b"0123456789".to_vec());

        // 刪除中間 "345" (offset 3, length 3)
        pt.delete(3, 3);
        assert_eq!(pt.collect_bytes(), b"0126789");

        // 插入 "ABC" 到 offset 3
        pt.insert(3, b"ABC");
        assert_eq!(pt.collect_bytes(), b"012ABC6789");

        // 跨 Piece 刪除：從 "2" 到 "6" (offset 2, length 5: 包含 '2', 'A', 'B', 'C', '6')
        pt.delete(2, 5);
        assert_eq!(pt.collect_bytes(), b"01789");
    }

    #[test]
    fn test_piece_table_read_slice_and_stream_write() {
        let mut pt = PieceTable::from_bytes(b"Line 1\nLine 2\nLine 3\n".to_vec());
        pt.insert(7, b"Inserted ");

        let slice = pt.read_slice(0, 16);
        assert_eq!(slice, b"Line 1\nInserted ");

        let mut buffer = Vec::new();
        pt.write_to(&mut buffer).unwrap();
        assert_eq!(buffer, pt.collect_bytes());
    }

    #[test]
    fn test_line_index_build_and_fetch_lines() {
        let content = b"First line\r\nSecond line\nThird line";
        let pt = PieceTable::from_bytes(content.to_vec());
        let line_index = LineIndex::build(&pt);

        assert_eq!(line_index.line_count(), 3);
        assert_eq!(line_index.line_at_offset(0), 0);
        assert_eq!(line_index.line_at_offset(5), 0);
        assert_eq!(line_index.line_at_offset(12), 1); // "Second line" 開頭

        // 按需讀取行內容
        let fetched = line_index.fetch_lines(&pt, encoding_rs::UTF_8, 0, 3);
        assert_eq!(fetched, vec!["First line", "Second line", "Third line"]);

        // 按需讀取單一行
        let line2 = line_index.fetch_lines(&pt, encoding_rs::UTF_8, 1, 1);
        assert_eq!(line2, vec!["Second line"]);
    }
}
