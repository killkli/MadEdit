# 01: Piece Table 核心資料結構

**What to build:** 在 `src/core/piece_table.rs` 實作 `PieceTable`，支援 Original Buffer 與 Add Buffer、Piece 節點管理，實作任意位移的隨機讀取、插入（insert）、刪除（delete）與全部位元組的迭代/串流輸出。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 定義 `BufferSource`（Original / Add）與 `Piece`（source, offset, length）
- [x] 實作 `PieceTable::new()`、`from_bytes()`、`len()`、`read_slice()`
- [x] 實作 `insert(offset, bytes)` 與 `delete(offset, length)`，支援跨片段的拆分與銜接
- [x] 撰寫單元測試覆蓋片段拆分、連續編輯與邊界極值
