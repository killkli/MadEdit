# 02: 游標導覽、範圍選取與基礎文字編輯邏輯

**What to build:** 在核心提供完整的游標移動（上下左右、Home/End、PageUp/Down）、Shift 擴展選取、字元插入、換行、Backspace、Delete 與選取區文字刪除替換邏輯，並同步更新底層文件位元組。

**Blocked by:** 01: 核心 TextPosition、Selection 與 Character Cell Grid 視覺欄寬計算

**Status:** resolved

- [x] 支援游標在行內與跨行的四向移動，自動依目標行長度夾擠（clamp）欄位
- [x] 支援 Shift 導覽時更新 `head` 並維持 `anchor` 進行選取擴展
- [x] 支援一般字元插入、Enter 換行、Backspace/Delete 刪除字元；若有選取區則先刪除選取文字
- [x] 撰寫單元測試驗證多行編輯、選取替換與邊界保護
