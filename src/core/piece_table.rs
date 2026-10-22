use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;

/// Piece 片段的資料來源（原始唯讀緩衝區或新增追加緩衝區）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// 檔案分頁快取的頁大小與最多保留頁數（共約 4 MB）
const PAGE_SIZE: usize = 64 * 1024;
const MAX_CACHED_PAGES: usize = 64;

/// 以檔案為後盾的唯讀原始資料：依需要以定位讀取（pread）取頁，並快取最近使用的頁
///
/// 不使用 mmap：在 Unix 上，對映中的檔案若被外部程式截短，存取超出新長度的頁面會收到
/// SIGBUS 而整個程式崩潰；定位讀取遇到截短只會讀到較短的資料，缺少的部分以 0 補齊。
pub struct FileSource {
    file: File,
    len: usize,
    cache: RefCell<HashMap<usize, Vec<u8>>>,
}

impl FileSource {
    fn read_page(&self, page: usize) -> Vec<u8> {
        let start = page * PAGE_SIZE;
        let want = PAGE_SIZE.min(self.len.saturating_sub(start));
        let mut buf = vec![0u8; want];
        let mut filled = 0;
        while filled < want {
            match read_at(&self.file, &mut buf[filled..], (start + filled) as u64) {
                Ok(0) | Err(_) => break, // 檔案被截短或讀取錯誤：其餘維持 0
                Ok(n) => filled += n,
            }
        }
        buf
    }

    /// 將 [offset, offset + out.len()) 的資料複製到 out
    fn copy_to(&self, offset: usize, out: &mut Vec<u8>, length: usize) {
        let mut pos = offset;
        let end = offset + length;
        let mut cache = self.cache.borrow_mut();
        while pos < end {
            let page = pos / PAGE_SIZE;
            if !cache.contains_key(&page) {
                if cache.len() >= MAX_CACHED_PAGES {
                    // 簡單淘汰：清空後重新累積，避免維護 LRU 順序的額外成本
                    cache.clear();
                }
                let data = self.read_page(page);
                cache.insert(page, data);
            }
            let data = &cache[&page];
            let in_page = pos - page * PAGE_SIZE;
            let take = (end - pos).min(data.len().saturating_sub(in_page));
            if take == 0 {
                break;
            }
            out.extend_from_slice(&data[in_page..in_page + take]);
            pos += take;
        }
        // 頁面不足（檔案被截短）時補 0，維持長度一致
        out.resize(out.len() + (end - pos), 0);
    }
}

#[cfg(unix)]
fn read_at(file: &File, buf: &mut [u8], offset: u64) -> std::io::Result<usize> {
    std::os::unix::fs::FileExt::read_at(file, buf, offset)
}

#[cfg(windows)]
fn read_at(file: &File, buf: &mut [u8], offset: u64) -> std::io::Result<usize> {
    std::os::windows::fs::FileExt::seek_read(file, buf, offset)
}

/// 原始緩衝區容器（記憶體位元組或檔案定位讀取）
pub enum OriginalBuffer {
    Bytes(Vec<u8>),
    File(FileSource),
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

impl PieceTable {
    /// 建立空的 PieceTable
    pub fn new() -> Self {
        Self::from_bytes(Vec::new())
    }

    fn init_with_original(original: OriginalBuffer, len: usize) -> Self {
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
            original,
            add_buffer: Vec::new(),
            pieces,
            total_len: len,
        }
    }

    /// 從記憶體位元組序列建立 PieceTable
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        let len = bytes.len();
        Self::init_with_original(OriginalBuffer::Bytes(bytes), len)
    }

    /// 從實體檔案建立 PieceTable（不載入記憶體，依需要讀取）
    pub fn open_file(file: File) -> std::io::Result<Self> {
        let len = file.metadata()?.len() as usize;
        Ok(Self::init_with_original(
            OriginalBuffer::File(FileSource {
                file,
                len,
                cache: RefCell::new(HashMap::new()),
            }),
            len,
        ))
    }

    /// 目前總位元組長度
    pub fn len(&self) -> usize {
        self.total_len
    }

    /// 將指定來源區段附加到 out
    fn append_source(&self, source: BufferSource, offset: usize, length: usize, out: &mut Vec<u8>) {
        match source {
            BufferSource::Add => out.extend_from_slice(&self.add_buffer[offset..offset + length]),
            BufferSource::Original => match &self.original {
                OriginalBuffer::Bytes(b) => out.extend_from_slice(&b[offset..offset + length]),
                OriginalBuffer::File(f) => f.copy_to(offset, out, length),
            },
        }
    }

    /// 隨機讀取指定位移與長度的位元組片段
    pub fn read_slice(&self, offset: usize, length: usize) -> Vec<u8> {
        let actual_len = length.min(self.total_len.saturating_sub(offset));
        let mut out = Vec::with_capacity(actual_len);
        if actual_len == 0 {
            return out;
        }
        let end_offset = offset + actual_len;
        let mut curr_offset = 0;
        for piece in &self.pieces {
            let piece_end = curr_offset + piece.length;
            if piece_end > offset && curr_offset < end_offset {
                let slice_start = offset.saturating_sub(curr_offset);
                let slice_end = (end_offset - curr_offset).min(piece.length);
                self.append_source(
                    piece.source,
                    piece.offset + slice_start,
                    slice_end - slice_start,
                    &mut out,
                );
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
        let offset = offset.min(self.total_len);
        let new_piece = Piece {
            source: BufferSource::Add,
            offset: self.add_buffer.len(),
            length: data.len(),
        };
        self.add_buffer.extend_from_slice(data);

        let mut curr_offset = 0;
        let mut insert_idx = self.pieces.len();
        let mut split: Option<(Piece, Piece)> = None;
        for (i, piece) in self.pieces.iter().enumerate() {
            let piece_end = curr_offset + piece.length;
            if offset == curr_offset {
                insert_idx = i;
                break;
            } else if offset > curr_offset && offset < piece_end {
                let left_len = offset - curr_offset;
                split = Some((
                    Piece { length: left_len, ..*piece },
                    Piece {
                        source: piece.source,
                        offset: piece.offset + left_len,
                        length: piece.length - left_len,
                    },
                ));
                insert_idx = i;
                break;
            }
            curr_offset = piece_end;
        }

        if let Some((left, right)) = split {
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
        let mut new_pieces = Vec::with_capacity(self.pieces.len() + 1);
        let mut curr_offset = 0;
        for piece in &self.pieces {
            let piece_start = curr_offset;
            let piece_end = curr_offset + piece.length;
            if piece_end <= del_start || piece_start >= del_end {
                new_pieces.push(*piece);
            } else {
                if piece_start < del_start {
                    new_pieces.push(Piece { length: del_start - piece_start, ..*piece });
                }
                if piece_end > del_end {
                    new_pieces.push(Piece {
                        source: piece.source,
                        offset: piece.offset + (del_end - piece_start),
                        length: piece_end - del_end,
                    });
                }
            }
            curr_offset = piece_end;
        }
        self.pieces = new_pieces;
        self.total_len -= actual_len;
    }

    /// 將全部 Piece 循序收集為一個位元組向量（僅建議在中小型檔案使用）
    pub fn collect_bytes(&self) -> Vec<u8> {
        self.read_slice(0, self.total_len)
    }

    /// 串流寫出所有片段至指定的 Writer（大檔分段讀寫，不整份載入記憶體）
    pub fn write_to<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        const CHUNK: usize = 4 * 1024 * 1024;
        let mut buf = Vec::with_capacity(CHUNK);
        for piece in &self.pieces {
            let mut done = 0;
            while done < piece.length {
                let take = CHUNK.min(piece.length - done);
                buf.clear();
                self.append_source(piece.source, piece.offset + done, take, &mut buf);
                writer.write_all(&buf)?;
                done += take;
            }
        }
        Ok(())
    }

    /// 在 [from, to) 範圍內搜尋位元組樣式，回傳第一個匹配位移（分塊讀取，適用大檔）
    pub fn find(&self, pattern: &[u8], from: usize, to: usize) -> Option<usize> {
        if pattern.is_empty() {
            return None;
        }
        const CHUNK: usize = 1024 * 1024;
        let to = to.min(self.total_len);
        let mut pos = from;
        while pos + pattern.len() <= to {
            let len = (CHUNK + pattern.len() - 1).min(to - pos);
            let data = self.read_slice(pos, len);
            if let Some(i) = data.windows(pattern.len()).position(|w| w == pattern) {
                return Some(pos + i);
            }
            if pos + len >= to {
                break;
            }
            pos += CHUNK;
        }
        None
    }

    /// 在 [0, before) 範圍內反向搜尋位元組樣式，回傳最後一個匹配位移
    pub fn rfind(&self, pattern: &[u8], before: usize) -> Option<usize> {
        if pattern.is_empty() {
            return None;
        }
        const CHUNK: usize = 1024 * 1024;
        let mut end = before.min(self.total_len);
        while end >= pattern.len() {
            let start = end.saturating_sub(CHUNK + pattern.len() - 1);
            let data = self.read_slice(start, end - start);
            if let Some(i) = data.windows(pattern.len()).rposition(|w| w == pattern) {
                return Some(start + i);
            }
            if start == 0 {
                break;
            }
            end = start + pattern.len() - 1;
        }
        None
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

        pt.insert(6, b"Beautiful ");
        assert_eq!(pt.collect_bytes(), b"Hello Beautiful World");
        pt.insert(0, b">>> ");
        assert_eq!(pt.collect_bytes(), b">>> Hello Beautiful World");
        let len = pt.len();
        pt.insert(len, b" !!!");
        assert_eq!(pt.collect_bytes(), b">>> Hello Beautiful World !!!");
    }

    #[test]
    fn test_piece_table_delete() {
        let mut pt = PieceTable::from_bytes(b"0123456789".to_vec());
        pt.delete(3, 3);
        assert_eq!(pt.collect_bytes(), b"0126789");
        pt.insert(3, b"ABC");
        assert_eq!(pt.collect_bytes(), b"012ABC6789");
        pt.delete(2, 5);
        assert_eq!(pt.collect_bytes(), b"01789");
    }

    #[test]
    fn test_piece_table_read_slice_and_stream_write() {
        let mut pt = PieceTable::from_bytes(b"Line 1\nLine 2\nLine 3\n".to_vec());
        pt.insert(7, b"Inserted ");
        assert_eq!(pt.read_slice(0, 16), b"Line 1\nInserted ");
        let mut buffer = Vec::new();
        pt.write_to(&mut buffer).unwrap();
        assert_eq!(buffer, pt.collect_bytes());
    }

    #[test]
    fn test_file_backed_source_survives_truncation() {
        let path = std::env::temp_dir().join(format!("madedit_pt_{}.bin", std::process::id()));
        let data: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&path, &data).unwrap();
        let pt = PieceTable::open_file(File::open(&path).unwrap()).unwrap();
        assert_eq!(pt.len(), data.len());
        assert_eq!(pt.read_slice(70_000, 10), &data[70_000..70_010]);
        // 資料以 251 為週期重複，第一個匹配位於 150000 % 251
        assert_eq!(pt.find(&data[150_000..150_004], 0, pt.len()), Some(150_000 % 251));

        // 外部截短檔案後讀取不應崩潰，缺少的資料以 0 補齊
        std::fs::write(&path, &data[..1000]).unwrap();
        let tail = pt.read_slice(190_000, 16);
        assert_eq!(tail.len(), 16);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_find_and_rfind_across_chunks() {
        let mut data = vec![0u8; 3 * 1024 * 1024];
        data[1024 * 1024 - 2..1024 * 1024 + 2].copy_from_slice(b"ABCD");
        data[2 * 1024 * 1024 + 5..2 * 1024 * 1024 + 9].copy_from_slice(b"ABCD");
        let pt = PieceTable::from_bytes(data);
        assert_eq!(pt.find(b"ABCD", 0, pt.len()), Some(1024 * 1024 - 2));
        assert_eq!(pt.find(b"ABCD", 1024 * 1024, pt.len()), Some(2 * 1024 * 1024 + 5));
        assert_eq!(pt.rfind(b"ABCD", pt.len()), Some(2 * 1024 * 1024 + 5));
        assert_eq!(pt.rfind(b"ABCD", 2 * 1024 * 1024), Some(1024 * 1024 - 2));
    }
}
