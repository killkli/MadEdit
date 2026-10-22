# 03: DocumentBuffer 整合、memmap2 大檔開啟與回歸測試

**What to build:** 引入 `memmap2`，將 `PieceTable` 與 `LineIndex` 整合進 `DocumentBuffer`，取代原有的全檔複製與整檔解碼；更新 Hex Dump 與 UI 介面，並通過 10MB+ 大檔與現有編碼/自繪編輯器回歸測試。

**Blocked by:** 02: LineIndex 行位移索引與按需行讀取

**Status:** ready-for-agent

- [ ] 在 `Cargo.toml` 引入 `memmap2 = "0.9"`
- [ ] `DocumentBuffer::load_file` 使用 `memmap2::Mmap` 零拷貝載入 Original Buffer
- [ ] 保持 Phase 1（多編碼切換與防毀損保護）與 Phase 2（可見行虛擬渲染與編輯）功能正常運作
- [ ] 新增大檔案秒開與隨機存取單元測試，`cargo check` 與 `cargo test` 零警告通過
- [ ] 同步更新 `AGENTS.md` 第 5 節
