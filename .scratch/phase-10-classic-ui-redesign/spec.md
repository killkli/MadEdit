# Spec: Phase 10 經典 MadEdit 風格介面重塑

> **整合說明**：本階段另有並行實作 `.scratch/phase-10-madedit-classic-ui-and-input-fix/spec.md`
> （焦點修復、深淺色主題、尺標、捲軸、控制字元、插入／覆寫模式、多語法）。
> 最終成果為兩者聯集：介面骨架採本規格的經典自繪路線，功能採另一份規格的完整實作。
> 決策理由詳見 [ADR 0010](../../docs/adr/0010-classic-ui-consolidation.md)。

## 1. 目標

將 MadEdit Rust 重製版的操作介面重塑為**貼近原版 MadEdit（C++ / wxWidgets）的經典 Windows 風格**，
包含經典選單列、原版圖示工具列、分頁列、六欄位狀態列，並讓選單與核心狀態（編輯模式、
選取範圍、游標位置、字元編碼等）保持雙向同步。

外部行為與既有 Phase 1～9 的核心能力（編輯模式、搜尋取代、書籤、跳轉、編碼轉換、
外部檔案變更監控、大檔引擎）完全沿用，**本階段僅重塑呈現層與選單模型**。

### 1.1 外觀基準來源

| 項目 | 路徑 | 用途 |
| --- | --- | --- |
| MadEdit-Mod 原始碼 | `../madedit-mod/src/MadEditFrame.cpp` | `CommandTable[]` 選單／工具列／快捷鍵定義（2221 行起） |
| MadEdit-Mod 圖示 | `../madedit-mod/images/*.xpm` | 38 個 16×16 工具列圖示來源 |
| 上游 MadEdit | `/tmp/madref/madedit-master/` | 上游原始碼與圖示比對 |

### 1.2 併入的進階功能（來自另一份 Phase 10 規格）

| 功能 | 實作位置 | 說明 |
| --- | --- | --- |
| 深色／淺色主題 | `ui/theme.slint`（`Theme.is-dark`）、`ui/appwindow.slint` | 預設跟隨系統配色，工具列可手動切換 |
| 水平欄位尺標 | `ui/appwindow.slint` | 與水平捲軸同步位移，顯示 0、10、20… 刻度 |
| 垂直／水平捲軸 | `ui/appwindow.slint` | 滑塊拖曳、方向按鈕微調、滾輪支援 |
| 控制字元顯示 | `src/core/editor.rs`（`apply_visible_whitespace`） | 空白顯示 `·`、Tab 顯示 `»`、行尾 `↵` |
| 插入／覆寫模式 | `src/core/editor.rs`（`overwrite_mode`） | `Insert` 鍵或工具列切換，狀態列顯示 `INS`／`OVR` |
| 焦點修復 | `ui/appwindow.slint`（`editor-focus` FocusScope） | 常駐 FocusScope 承接鍵盤事件，操作後自動回復焦點 |
| 多語法支援 | `src/core/syntax.rs` | 新增 C#、Java、Go、Shell、TOML |
| 命令列開檔 | `src/main.rs` | `MadEdit <檔案>` 於啟動時直接載入 |


## 2. 規格細節

### 2.1 經典樣式元件庫 (`ui/theme.slint`)
1. **配色**：`Theme` 全域結構以 `#f0f0f0`（視窗底色）、`#0a246a`（選取高亮）、`#808080`（凹陷邊框）
   等經典 Windows 系統色為基底，另定義 `text-color`、`muted-color` 等語意色。
2. **共用元件**：
   - `ClassicToolButton`：以 `TouchArea` + 矩形繪製的凸起／凹陷按鈕，支援 hover 與按下狀態。
   - `ClassicButton`、`ClassicLineEdit`（凹陷框線 + 原生 `TextInput`，`text <=> root.text` 雙向綁定）、
     `ClassicCheckBox`、`ToolbarSeparator`（垂直分隔線 + 左右淺色／深色雙線）。
3. **提示框**：自繪黃底黑框 `PopupWindow`（`close-policy: no-auto-close`），避免 Slint 內建
   `Tooltip` 僅接受 `@markdown` 字面值而無法傳入執行期字串的限制。

### 2.2 經典選單列 (`ui/menubar.slint`)
1. **資料模型**：
   - `MenuItemDef`：`text` / `shortcut` / `item-type`（一般、separator、核取）/ `enabled` / `checked` / `action`。
   - `MenuDef`：`title` / `width` / `x-offset` / `popup-width` / `popup-height` / `items`。
2. **元件**：
   - `ClassicMenuPopup`：渲染彈出選單，含分隔線、核取記號（√）、右側快捷鍵文字、停用項灰顯。
   - `ClassicMenuBar`：以**單一共用 `PopupWindow`** 搭配 `open-index` / `safe-index` / `open-menu`
     切換顯示的選單，避免為每個頂層選單各建一個彈出視窗。
   - 以 `popup.is-open` 連動標題高亮，避免點擊視窗外部後標題殘留選取樣式。
   - 滑鼠懸停於已開啟的選單列時可直接切換至相鄰選單（符合原版行為）。

### 2.3 主視窗版面 (`ui/appwindow.slint`)
1. **版面順序**（自上而下）：經典選單列 → 圖示工具列 → 分頁列 → 搜尋／跳轉／外部變更橫幅
   → 編輯區（行號欄 + 文字區） → 六欄位狀態列。
2. **工具列**：6 組共 21 個原版圖示（檔案、編輯、搜尋、模式、書籤、輔助），使用 `@image-url`
   內嵌 `assets/icons/*.png`；復原／重做等未實作圖標以停用樣式呈現。
3. **編輯區**：維持既有自繪字元格網渲染，行號欄為凹陷面板，書籤以藍色圓點標記。
4. **狀態列**：六欄位 `Ln` / `CharPos` / `SelSize` / `編碼.DOS` / `唯讀` / `INS`，欄位間以凹陷分隔線區隔。
5. **通用選擇面板**：語法突顯、字元編碼、換行格式、重新解碼、轉換編碼改以選單項目觸發，
   開啟同一個可捲動的 `PopupWindow` 選擇面板（避免巢狀彈出選單複雜度）。

### 2.4 選單模型與動作派送 (`src/main.rs`)
1. **頂層選單**：依原版 `CommandTable` 建立 7 個選單——
   `檔案(F)` / `編輯(E)` / `搜尋(S)` / `檢視(V)` / `工具(T)` / `視窗(W)` / `說明(H)`。
2. **模型建構**：`build_menus(edit_mode)` 產生 `Vec<MenuDef>`，並計算各選單標題寬度、
   `x-offset`、彈出選單寬度與高度；`text_width_px` 以中日韓全形 13px、拉丁 7px 估算寬度。
3. **項目種類**：
   - `menu_item`（一般）、`menu_item_disabled`（對應原版尚未實作的功能，如列印、另存新檔、復原／重做）、
     `menu_check`（核取項，用於編輯模式）、`menu_separator`（分隔線）。
4. **動作派送**：`callback menu-action(string)` 接收動作識別碼，由 `handle_menu_action` 對應既有 helper
   （`do_open_file`、`do_save_file`、`do_find` 等），避免重複實作既有功能。
5. **狀態列同步**：`sync_ui_from_doc` 同時輸出六欄位狀態列內容與最新選單模型，
   確保編輯模式切換時選單的核取記號即時更新。

### 2.5 核心狀態擴充 (`src/core/editor.rs`)
1. `EditorState::char_pos(pos)`：回傳游標自文件開頭起算的 0-based 字元索引，
   每行結尾計入一個換行字元；行號超出範圍時夾限至最後一行。
2. `EditorState::selection_char_count()`：回傳選取範圍字元數（不含換行字元）。
3. 對應單元測試：`test_char_pos_counts_newlines`、`test_selection_char_count`。

### 2.6 圖示資產 (`assets/`)
1. `assets/icons-src/*.xpm`：38 個原版 XPM 圖示（唯一真實來源），附 `README.md` 說明來源與 GPL 授權。
2. `assets/icons/*.png`：由 `assets/make_toolbar_icons.py` 產生（純標準函式庫 `zlib` + `struct`，
   最近鄰放大 2 倍，32×30），**不得手動編輯**。
3. 授權：原版 MadEdit / MadEdit-Mod 圖示受 GNU GPL 授權，本重製版為衍生作品，
   散布時須遵循相同條款並保留來源說明。

## 3. 驗收條件

- [x] `cargo check --offline` 零警告通過。
- [x] `cargo test --offline` 59 項全數通過（含狀態列計算與深色主題語法測試）。
- [x] 選單列可開啟、懸停切換、點擊外部收起，且選單項目停用／核取狀態正確反映核心狀態。
- [x] 工具列 21 個圖示正確顯示，復原／重做為停用樣式。
- [x] 搜尋取代面板、指定行號面板、外部變更橫幅在重寫後運作正常。
- [x] 十六進位模式三欄（Offset / Hex / ASCII）配色正確，且與文字區互斥顯示。
- [x] 六欄位狀態列顯示正確：`Ln`、`CharPos`（含位元組位置）、`SelSize`（含位元組數）、編碼、唯讀、`INS`／`OVR`。
- [x] 深色與淺色主題皆可正常顯示（選單列、工具列、狀態列、編輯區、捲軸無撞色）。
- [x] 水平欄位尺標與水平捲軸位移同步；右側垂直捲軸與底部水平捲軸可拖曳與按鈕微調。
- [x] 控制字元顯示（空白 `·`、Tab `»`、行尾 `↵`）與深色主題語法配色正確。
- [x] 鍵盤輸入正常（FocusScope 承接 `key-pressed`），開檔後焦點自動回到編輯區。
