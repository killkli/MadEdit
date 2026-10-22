Status: closed

# Issue 02: 列塊剪貼簿與多行同步輸入/刪除操作

## Description

實作列塊模式下的複製 (Copy)、剪下 (Cut)、貼上 (Paste)、多行同步輸入字元以及 Backspace / Delete 區塊刪除操作。

## Tasks

1. **矩形提取 (selected_text)**：當 `kind == SelectionKind::Block` 時，依序提取每行矩形內的文字，以 `\n` 連接。
2. **矩形刪除 (delete_selection)**：對選取範圍內的每一行，將落在視覺欄位內的字元切除；短行安全略過。
3. **多行同步打字 (insert_char / insert_text)**：若有矩形選取則先刪除選取區，隨後在各選取行對應的起點位置同步插入文字。
4. **列塊貼上 (paste_from_clipboard)**：若剪貼簿包含多行且當前為矩形選取，逐行對齊貼入對應行。
5. **單元測試**：包含複製剪下往返、矩形選取刪除、多行開頭對齊插入、短行安全防護測試。
