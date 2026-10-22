# 02: LineIndex 行位移索引與按需行讀取

**What to build:** 在 `src/core/piece_table.rs` 實作 `LineIndex`，能高效掃描 Piece Table 中的換行符（`\n`）建立行位移表，並提供指定行範圍的位元組切片擷取與按需解碼。

**Blocked by:** 01: Piece Table 核心資料結構

**Status:** ready-for-agent

- [ ] 實作 `LineIndex::build_from_piece_table`，記錄每行的位元組起始位移
- [ ] 支援二分搜尋取得指定行號之 `(start_offset, end_offset)`
- [ ] 實作按需讀取指定行範圍位元組並解碼出 `Vec<String>`
- [ ] 撰寫單元測試覆蓋不同換行格式（CRLF / LF）與隨機行定位
