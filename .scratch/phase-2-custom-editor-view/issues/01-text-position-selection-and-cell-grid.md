# 01: 核心 TextPosition、Selection 與 Character Cell Grid 視覺欄寬計算

**What to build:** 建立 `TextPosition`、`Selection`、`Viewport` 模型，並實作等寬字元格網視覺欄寬計算（ASCII 佔 1 欄、CJK 全形佔 2 欄、Tab 展開對齊 4 欄倍數），提供像素座標換算介面。

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] 定義 `TextPosition`（line, col）與 `Selection`（anchor, head），支援方向正規化（起點與終點）
- [x] 實作字元欄寬與 Tab 展開演算法（`char_visual_width`、`char_col_to_visual_col`、`visual_col_to_char_col`）
- [x] 撰寫單元測試覆蓋 ASCII、中文字元、Tab 混排與邊界極值
