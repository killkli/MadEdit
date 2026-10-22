# Issue 2: Slint 編輯器焦點輸入修復與原版 MadEdit 四大經典 UI 實作

## 狀態
closed

## 目標
1. 修正 `ui/appwindow.slint` 中的 `editor-focus := FocusScope`，確保點擊編輯區或工具列操作後皆能正常打字輸入。
2. 支援深淺色主題一鍵切換（預設跟隨系統 `Palette.color-scheme`），消除所有文字撞色問題。
3. 實作主選單列、緊湊工具列、檔案分頁標籤列、水平欄位尺標 (Column Ruler)、游標行外框高亮、控制字元渲染與底部多格狀態列。
