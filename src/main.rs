#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;

use std::cell::RefCell;
use std::rc::Rc;
use core::{
    char_col_to_visual_col, parse_goto_line_col, parse_goto_offset, DocumentBuffer, EditMode,
    NewLineFormat, SearchOptions, Selection, SUPPORTED_ENCODINGS, SUPPORTED_NEWLINES,
    SUPPORTED_SYNTAXES,
};
use slint::platform::Key;
use slint::{ModelRc, SharedString, Timer, TimerMode, VecModel};

slint::include_modules!();

/// 根據字型大小計算等寬字元格網寬度 (cell_width) 與行高 (line_height)
fn calc_editor_metrics(font_size: i32) -> (f32, f32) {
    let fs = font_size.clamp(10, 32) as f32;
    let cell_width = (fs * 0.55 * 2.0).round() / 2.0;
    let line_height = (fs * 1.35).round().max(18.0);
    (cell_width, line_height)
}

// ── 選單列模型 ─────────────────────────────────────────────────────
// 選單結構沿用原版 MadEdit（檔案 / 編輯 / 搜尋 / 檢視 / 工具 / 視窗 / 說明），
// 尚未實作的功能以停用樣式呈現，與原版的灰階項目一致。

/// 頂層選單標題（順序即選單列由左至右的排列順序）
const MENU_TITLES: [&str; 7] = [
    "檔案(F)",
    "編輯(E)",
    "搜尋(S)",
    "檢視(V)",
    "工具(T)",
    "視窗(W)",
    "說明(H)",
];

/// 選單項目種類
const MENU_ITEM: i32 = 0;
const MENU_SEPARATOR: i32 = 1;
const MENU_CHECK: i32 = 2;

/// 選單列標題的額外左右留白（像素）
const MENU_TITLE_PADDING: f32 = 22.0;

/// 估算文字寬度（像素）：中日韓全形字元約 13px、拉丁字元約 7px
fn text_width_px(text: &str) -> f32 {
    text.chars()
        .map(|c| if (c as u32) >= 0x2E80 { 13.0 } else { 7.0 })
        .sum()
}

/// 建立一般選單項目
fn menu_item(text: &str, shortcut: &str, action: &str) -> MenuItemDef {
    MenuItemDef {
        text: text.into(),
        shortcut: shortcut.into(),
        item_type: MENU_ITEM,
        enabled: true,
        checked: false,
        action: action.into(),
    }
}

/// 建立停用選單項目（對應原版尚未實作的功能）
fn menu_item_disabled(text: &str, shortcut: &str) -> MenuItemDef {
    MenuItemDef {
        text: text.into(),
        shortcut: shortcut.into(),
        item_type: MENU_ITEM,
        enabled: false,
        checked: false,
        action: "".into(),
    }
}

/// 建立可勾選選單項目
fn menu_check(text: &str, shortcut: &str, action: &str, checked: bool) -> MenuItemDef {
    MenuItemDef {
        text: text.into(),
        shortcut: shortcut.into(),
        item_type: MENU_CHECK,
        enabled: true,
        checked,
        action: action.into(),
    }
}

/// 建立選單分隔線
fn menu_separator() -> MenuItemDef {
    MenuItemDef {
        text: "".into(),
        shortcut: "".into(),
        item_type: MENU_SEPARATOR,
        enabled: false,
        checked: false,
        action: "".into(),
    }
}

/// 計算指定頂層選單標題在選單列中的水平位置
fn menu_title_x(title: &str) -> f32 {
    let mut x = 2.0;
    for t in MENU_TITLES.iter() {
        if *t == title {
            return x;
        }
        x += text_width_px(t) + MENU_TITLE_PADDING;
    }
    x
}

/// 依原版 MadEdit 的選單結構建構選單模型
fn build_menus(edit_mode: i32) -> Vec<MenuDef> {
    // ── 檔案(F) ──
    let file_menu = vec![
        menu_item("新增檔案", "Ctrl+N", "new"),
        menu_item("開啟舊檔...", "Ctrl+O", "open"),
        menu_separator(),
        menu_item("儲存檔案", "Ctrl+S", "save"),
        menu_item_disabled("另存新檔...", ""),
        menu_item_disabled("全部儲存", "Ctrl+Shift+S"),
        menu_separator(),
        menu_item("重新載入檔案", "Ctrl+R", "reload"),
        menu_separator(),
        menu_item_disabled("關閉檔案", "Ctrl+F4"),
        menu_item_disabled("關閉所有檔案", ""),
        menu_separator(),
        menu_item_disabled("設定/清除唯讀旗標", ""),
        menu_separator(),
        menu_item_disabled("頁面設定...", ""),
        menu_item_disabled("列印預覽...", ""),
        menu_item_disabled("列印...", "Ctrl+P"),
        menu_separator(),
        menu_item_disabled("最近使用的檔案", ""),
        menu_separator(),
        menu_item("離開程式", "", "quit"),
    ];

    // ── 編輯(E) ──
    let edit_menu = vec![
        menu_item_disabled("復原", "Ctrl+Z"),
        menu_item_disabled("重做", "Ctrl+Y"),
        menu_separator(),
        menu_item("剪下", "Ctrl+X", "cut"),
        menu_item("複製", "Ctrl+C", "copy"),
        menu_item("貼上", "Ctrl+V", "paste"),
        menu_item("刪除", "Del", "delete"),
        menu_separator(),
        menu_item("複製整行", "Ctrl+Shift+D", "duplicate-line"),
        menu_item("刪除整行", "Ctrl+Shift+K", "delete-line"),
        menu_item("整行上移", "Alt+Up", "move-line-up"),
        menu_item("整行下移", "Alt+Down", "move-line-down"),
        menu_separator(),
        menu_item("全選", "Ctrl+A", "select-all"),
        menu_separator(),
        menu_item_disabled("增加縮排", "Tab"),
        menu_item_disabled("減少縮排", "Shift+Tab"),
        menu_separator(),
        menu_item_disabled("插入定位字元", "Ctrl+`"),
        menu_item_disabled("插入日期與時間", "F7"),
        menu_separator(),
        menu_item_disabled("排序", ""),
        menu_item_disabled("進階...", ""),
    ];

    // ── 搜尋(S) ──
    let search_menu = vec![
        menu_item("尋找...", "Ctrl+F", "find"),
        menu_item("尋找下一個", "F3", "find-next"),
        menu_item("尋找上一個", "Shift+F3", "find-prev"),
        menu_item_disabled("快速尋找", "F8"),
        menu_separator(),
        menu_item("取代...", "Ctrl+H", "replace"),
        menu_separator(),
        menu_item_disabled("在檔案中尋找/取代...", "Ctrl+Shift+F"),
        menu_separator(),
        menu_item("移至行...", "Ctrl+G", "goto"),
        menu_item_disabled("移至位置...", "Ctrl+Shift+G"),
        menu_separator(),
        menu_item("切換/移除書籤", "Ctrl+F2", "bookmark-toggle"),
        menu_item("下一個書籤", "F2", "bookmark-next"),
        menu_item("上一個書籤", "Shift+F2", "bookmark-prev"),
        menu_item("清除所有書籤", "", "bookmark-clear"),
    ];

    // ── 檢視(V) ──
    let view_menu = vec![
        menu_check("文字模式", "Alt+1", "mode-text", edit_mode == 0),
        menu_check("欄位模式", "Alt+2", "mode-column", edit_mode == 1),
        menu_check("十六進位模式", "Alt+3", "mode-hex", edit_mode == 2),
        menu_separator(),
        menu_item("語法突顯...", "", "pick-syntax"),
        menu_item("換行格式...", "", "pick-newline"),
        menu_separator(),
        menu_item_disabled("顯示行號", "Ctrl+Alt+D"),
        menu_item_disabled("自動換行", "Ctrl+Q"),
        menu_item_disabled("永遠置頂", "Ctrl+F11"),
        menu_item_disabled("全螢幕模式", "F11"),
    ];

    // ── 工具(T) ──
    let tools_menu = vec![
        menu_item_disabled("選項...", ""),
        menu_item_disabled("語法突顯設定...", ""),
        menu_separator(),
        menu_item_disabled("檔案類型關聯...", ""),
        menu_separator(),
        menu_item("重新解碼...", "", "pick-reinterpret"),
        menu_item("轉換檔案編碼...", "", "pick-convert"),
        menu_separator(),
        menu_item_disabled("轉換中文簡繁...", ""),
    ];

    // ── 視窗(W) ──
    let window_menu = vec![
        menu_item_disabled("下一個視窗", "Ctrl+F6"),
        menu_item_disabled("上一個視窗", "Ctrl+Shift+F6"),
        menu_separator(),
        menu_item_disabled("視窗層級排列", ""),
        menu_item_disabled("視窗橫向排列", ""),
    ];

    // ── 說明(H) ──
    let help_menu = vec![
        menu_item("關於 MadEdit...", "", "about"),
    ];

    let groups = [
        file_menu,
        edit_menu,
        search_menu,
        view_menu,
        tools_menu,
        window_menu,
        help_menu,
    ];

    // 計算各選單的寬度、水平位置與彈出選單尺寸
    let mut x = 2.0f32;
    let mut menus = Vec::with_capacity(groups.len());
    for (title, items) in MENU_TITLES.iter().zip(groups.into_iter()) {
        let width = text_width_px(title) + MENU_TITLE_PADDING;

        // 彈出選單寬度：以最長的項目（文字 + 快捷鍵）為準
        let popup_width = items
            .iter()
            .filter(|i| i.item_type != MENU_SEPARATOR)
            .map(|i| {
                let shortcut = if i.shortcut.is_empty() {
                    0.0
                } else {
                    text_width_px(&i.shortcut) + 24.0
                };
                text_width_px(&i.text) + shortcut + 56.0
            })
            .fold(0.0f32, f32::max)
            .clamp(170.0, 340.0);

        // 彈出選單高度：一般項目 20px、分隔線 8px，再加上上下留白
        let popup_height: f32 = items
            .iter()
            .map(|i| if i.item_type == MENU_SEPARATOR { 8.0 } else { 20.0 })
            .sum::<f32>()
            + 4.0;

        menus.push(MenuDef {
            title: (*title).into(),
            width,
            x_offset: x,
            popup_width,
            popup_height,
            items: ModelRc::new(VecModel::from(items)),
        });
        x += width;
    }
    menus
}

/// 從 UI 狀態提取目前設定的搜尋選項
fn get_search_options(ui: &MainWindow) -> SearchOptions {
    SearchOptions::new(
        ui.get_search_match_case(),
        ui.get_search_whole_word(),
        ui.get_search_use_regex(),
    )
}

/// 更新 UI 中的搜尋狀態計數（如 "3/15" 或 "查無結果"）
fn update_search_status_ui(ui: &MainWindow, doc: &DocumentBuffer) {
    let query = ui.get_search_query();
    if query.is_empty() {
        ui.set_search_status_text("".into());
        return;
    }
    let opts = get_search_options(ui);
    let (cur, total) = doc.get_search_status(query.as_str(), &opts);
    if total == 0 {
        ui.set_search_status_text("查無結果".into());
    } else {
        ui.set_search_status_text(format!("{}/{}", cur, total).into());
    }
}

/// 統一更新搜尋結果 UI 反饋（DRY 萃取共用邏輯）
fn update_search_ui_feedback(ui: &MainWindow, res: Option<(usize, usize)>, query: &str) {
    if query.is_empty() {
        ui.set_search_status_text("".into());
    } else if let Some((cur, total)) = res {
        ui.set_search_status_text(format!("{}/{}", cur, total).into());
    } else {
        ui.set_search_status_text("查無結果".into());
    }
}

/// 若編輯器當前選取非空單行文字，自動填入搜尋關鍵字輸入框（DRY 萃取共用邏輯）
fn populate_search_query_from_selection(ui: &MainWindow, doc: &DocumentBuffer) {
    let selected = doc.editor.selected_text();
    if !selected.is_empty() && !selected.contains('\n') {
        ui.set_search_query(selected.into());
        update_search_status_ui(ui, doc);
    }
}

/// 開啟並設定跳轉面板預設內容
fn open_goto_panel(ui: &MainWindow, doc: &DocumentBuffer) {
    ui.set_show_search_panel(false);
    ui.set_show_goto_panel(true);

    if doc.edit_mode == EditMode::Hex {
        let max_offset = doc.len();
        ui.set_goto_label_text(format!("移至位移 Offset (0x0 - 0x{:X}):", max_offset).into());
        ui.set_goto_input_text(format!("0x{:X}", doc.hex_start_offset).into());
    } else {
        let total_lines = doc.editor.line_count().max(1);
        let cur_line = doc.editor.selection.head.line + 1;
        ui.set_goto_label_text(format!("移至行 (1 - {}):", total_lines).into());
        ui.set_goto_input_text(format!("{}", cur_line).into());
    }
}

/// 產生狀態列顯示字串，包含位元組長度、修改標記與無損解碼錯誤警告
fn format_status_info(doc: &DocumentBuffer) -> String {
    let modified_mark = if doc.is_modified { " *" } else { "" };
    let error_warn = if doc.has_decode_error {
        " [警告: 含無法無損解碼字元，請重新解碼或轉換編碼]"
    } else {
        ""
    };
    format!("Len: {} bytes{}{}", doc.len(), modified_mark, error_warn)
}

/// 將核心 DocumentBuffer 的完整狀態同步至 Slint 主視窗屬性
fn sync_ui_from_doc(ui: &MainWindow, doc: &DocumentBuffer) {
    let mode_idx = match doc.edit_mode {
        EditMode::Text => 0,
        EditMode::Column => 1,
        EditMode::Hex => 2,
    };

    let (cell_width, line_height) = calc_editor_metrics(ui.get_editor_font_size());
    ui.set_cell_width(cell_width);
    ui.set_line_height(line_height);

    ui.set_file_path(doc.file_name().into());
    ui.set_edit_mode(mode_idx);
    ui.set_encoding_name(doc.encoding.name().into());
    ui.set_selected_encoding(doc.encoding.name().into());
    ui.set_newline_format(doc.newline_format.as_str().into());
    ui.set_selected_newline(doc.newline_format.as_str().into());
    ui.set_current_syntax(doc.current_syntax.as_str().into());

    // 同步可見行自繪資料（包含多色語法分段 spans、書籤標記與行尾換行座標 eol_x）
    let visible_lines_data = doc.generate_visible_lines(cell_width);
    let slint_lines: Vec<VisibleLine> = visible_lines_data
        .into_iter()
        .map(|l| {
            let slint_spans: Vec<HighlightSpan> = l
                .spans
                .into_iter()
                .map(|s| HighlightSpan {
                    text: s.text.into(),
                    color: slint::Color::from_argb_u8(
                        255,
                        s.color_rgb.0,
                        s.color_rgb.1,
                        s.color_rgb.2,
                    ),
                })
                .collect();

            VisibleLine {
                line_number: l.line_number,
                text: l.text.into(),
                spans: ModelRc::new(VecModel::from(slint_spans)),
                has_selection: l.has_selection,
                sel_start_x: l.sel_start_x,
                sel_width: l.sel_width,
                has_caret: l.has_caret,
                caret_x: l.caret_x,
                has_bookmark: l.has_bookmark,
                eol_x: l.eol_x,
            }
        })
        .collect();
    ui.set_visible_lines(ModelRc::new(VecModel::from(slint_lines)));

    // 同步原版 MadEdit 5 格狀態列資訊與捲軸 (Scrollbar) 狀態
    let head = doc.editor.selection.head;
    let total_lines = doc
        .line_index
        .line_count()
        .max(doc.editor.line_count())
        .max(1);
    let cur_line = head.line + 1;
    let cur_col = head.col + 1;

    ui.set_vscroll_first_line(doc.editor.viewport.first_visible_line as i32);
    ui.set_vscroll_total_lines(total_lines as i32);
    ui.set_vscroll_visible_lines(doc.editor.viewport.visible_line_count as i32);

    let max_cols = doc
        .editor
        .max_visual_columns()
        .max(doc.editor.viewport.first_visible_col + doc.editor.viewport.visible_col_count);
    ui.set_hscroll_first_col(doc.editor.viewport.first_visible_col as i32);
    ui.set_hscroll_total_cols(max_cols as i32);
    ui.set_hscroll_visible_cols(doc.editor.viewport.visible_col_count as i32);

    if doc.editor.selection.is_block() && !doc.editor.selection.is_empty() {
        let bounds = doc.editor.selection.visual_bounds(&doc.editor);
        let block_status = format!(
            "行 : {} / {}  列 : {} [{}x{}]",
            cur_line,
            total_lines,
            cur_col,
            bounds.line_count(),
            bounds.col_width()
        );
        ui.set_caret_status(block_status.into());
    } else {
        // 格式對應原版 MadEdit 狀態列第一欄：Ln:<行> /<總行數>  Col:<欄>
        let total_lines = doc.editor.line_count();
        ui.set_caret_status(
            format!("Ln:{} /{}  Col:{}", head.line + 1, total_lines, head.col + 1).into(),
        );
    }

    // 狀態列第二欄：字元位置 / 位元組位置 / 檔案大小
    //   char_pos 為字元層級索引；byte_offset 由 DocumentBuffer 依目前編碼計算
    let char_pos = doc.editor.char_pos(head);
    let (_, byte_offset, _, sel_bytes) = doc.cursor_and_selection_metrics();
    ui.set_status_charpos(format!("CharPos:{} (0x{:X}) /{}", char_pos, byte_offset, doc.len()).into());

    // 狀態列第三欄：選取字元數 / 位元組數
    let sel_chars = doc.editor.selection_char_count();
    ui.set_status_selsize(format!("SelSize:{} ({}B)", sel_chars, sel_bytes).into());

    // 狀態列第四欄：編碼 + BOM + 換行格式（沿用原版的 .BOM.DOS 樣式）
    ui.set_status_encoding(doc.encoding_status_code().into());

    // 狀態列第六欄：插入 / 覆寫模式（對應原版 MadEdit 狀態列最右欄）
    ui.set_status_insert(if doc.editor.overwrite_mode { "OVR" } else { "INS" }.into());

    // 選單列模型：依目前模式更新勾選狀態
    ui.set_menus(ModelRc::new(VecModel::from(build_menus(mode_idx))));

    if doc.edit_mode == EditMode::Hex {
        let dump = doc.get_hex_dump();
        ui.set_hex_offset_text(dump.offsets.into());
        ui.set_hex_content_text(dump.hex.into());
        ui.set_hex_ascii_text(dump.ascii.into());
    }

    ui.set_is_modified(doc.is_modified);
    ui.set_has_external_change(false);
    ui.set_status_info(format_status_info(doc).into());
}

// ── 功能動作（選單列 / 工具列共用）────────────────────────────────

/// 開啟舊檔對話框並載入檔案
fn do_open_file(ui: &MainWindow, doc: &mut DocumentBuffer) {
    let Some(path) = rfd::FileDialog::new().pick_file() else {
        return;
    };
    if let Err(e) = doc.load_file(&path) {
        eprintln!("Failed to open file: {}", e);
        return;
    }
    sync_ui_from_doc(ui, doc);
}

/// 儲存目前檔案（尚未命名時另存新檔）
fn do_save_file(ui: &MainWindow, doc: &mut DocumentBuffer) {
    let target_path = doc
        .file_path
        .clone()
        .or_else(|| rfd::FileDialog::new().save_file());

    if let Some(path) = target_path {
        if let Err(e) = doc.save_file(Some(path)) {
            eprintln!("Failed to save file: {}", e);
            ui.set_status_info(format!("存檔拒絕: {}", e).into());
            return;
        }
    }
    sync_ui_from_doc(ui, doc);
}

/// 重新載入磁碟上的檔案內容
fn do_reload_file(ui: &MainWindow, doc: &mut DocumentBuffer) {
    if let Err(e) = doc.reload_file() {
        eprintln!("Failed to reload file: {}", e);
        return;
    }
    sync_ui_from_doc(ui, doc);
}

/// 忽略外部檔案變更通知
fn do_ignore_external_change(ui: &MainWindow, doc: &mut DocumentBuffer) {
    doc.ignore_external_change();
    sync_ui_from_doc(ui, doc);
}

/// 切換編輯模式（文字 / 欄位 / 十六進位）
fn do_set_mode(ui: &MainWindow, doc: &mut DocumentBuffer, mode: EditMode) {
    doc.set_edit_mode(mode);
    sync_ui_from_doc(ui, doc);
}

/// 尋找下一個匹配項
fn do_find_next(ui: &MainWindow, doc: &mut DocumentBuffer) {
    let query = ui.get_search_query();
    let opts = get_search_options(ui);
    let res = doc.find_next(query.as_str(), &opts);
    update_search_ui_feedback(ui, res, query.as_str());
    sync_ui_from_doc(ui, doc);
}

/// 尋找上一個匹配項
fn do_find_prev(ui: &MainWindow, doc: &mut DocumentBuffer) {
    let query = ui.get_search_query();
    let opts = get_search_options(ui);
    let res = doc.find_prev(query.as_str(), &opts);
    update_search_ui_feedback(ui, res, query.as_str());
    sync_ui_from_doc(ui, doc);
}

/// 取代目前匹配項
fn do_replace(ui: &MainWindow, doc: &mut DocumentBuffer) {
    let query = ui.get_search_query();
    let replace_with = ui.get_replace_text();
    let opts = get_search_options(ui);
    doc.replace_current(query.as_str(), replace_with.as_str(), &opts);
    update_search_status_ui(ui, doc);
    sync_ui_from_doc(ui, doc);
}

/// 取代全部匹配項
fn do_replace_all(ui: &MainWindow, doc: &mut DocumentBuffer) {
    let query = ui.get_search_query();
    let replace_with = ui.get_replace_text();
    let opts = get_search_options(ui);
    let count = doc.replace_all(query.as_str(), replace_with.as_str(), &opts);
    ui.set_search_status_text(format!("已取代 {} 處", count).into());
    sync_ui_from_doc(ui, doc);
}

/// 依輸入內容跳轉至指定行 / 欄位或十六進位位移
fn do_goto_confirmed(ui: &MainWindow, doc: &mut DocumentBuffer, input_text: &str) {
    ui.set_show_goto_panel(false);

    if doc.edit_mode == EditMode::Hex {
        if let Some(offset) = parse_goto_offset(input_text) {
            doc.goto_offset(offset);
        }
    } else if let Some((line, col)) = parse_goto_line_col(input_text) {
        doc.goto_line_col(line, col);
    }

    sync_ui_from_doc(ui, doc);
}

/// 切換語法突顯種類
fn do_syntax_changed(ui: &MainWindow, doc: &mut DocumentBuffer, name: &str) {
    doc.set_syntax(name);
    sync_ui_from_doc(ui, doc);
}

/// 切換換行格式
fn do_newline_changed(ui: &MainWindow, doc: &mut DocumentBuffer, label: &str) {
    if let Some(fmt) = NewLineFormat::from_label(label) {
        doc.convert_newline_format(fmt);
        sync_ui_from_doc(ui, doc);
    }
}

/// 以指定編碼重新解讀檔案內容（不解碼轉換）
fn do_reinterpret(ui: &MainWindow, doc: &mut DocumentBuffer, encoding: &str) {
    if doc.reinterpret_with_encoding(encoding) {
        sync_ui_from_doc(ui, doc);
    }
}

/// 將檔案內容轉換為指定編碼
fn do_convert(ui: &MainWindow, doc: &mut DocumentBuffer, encoding: &str) {
    if doc.convert_to_encoding(encoding) {
        sync_ui_from_doc(ui, doc);
    }
}

/// 開啟通用選擇面板（語法 / 編碼 / 換行格式）
fn open_picker(
    ui: &MainWindow,
    title: &str,
    options: ModelRc<SharedString>,
    current: &str,
    action: &str,
    x: f32,
) {
    ui.set_picker_title(title.into());
    ui.set_picker_options(options);
    ui.set_picker_current(current.into());
    ui.set_picker_action(action.into());
    ui.set_picker_x(x);
    ui.set_picker_open(true);
    // 由 Slint 端負責實際彈出面板
    ui.invoke_show_picker();
}

/// 處理選單列 / 工具列送出的動作識別碼
///
/// 格式為「命令」或「命令|參數」（參數用於選擇面板的選項）。
fn handle_menu_action(ui: &MainWindow, doc: &mut DocumentBuffer, action: &str) {
    let (cmd, arg) = match action.split_once('|') {
        Some((c, a)) => (c, Some(a)),
        None => (action, None),
    };

    match cmd {
        "new" => {
            *doc = DocumentBuffer::new();
            sync_ui_from_doc(ui, doc);
        }
        "open" => do_open_file(ui, doc),
        "save" => do_save_file(ui, doc),
        "reload" => do_reload_file(ui, doc),

        "cut" => {
            doc.editor.cut_to_clipboard();
            doc.sync_from_editor();
            sync_ui_from_doc(ui, doc);
        }
        "copy" => {
            doc.editor.copy_to_clipboard();
        }
        "paste" => {
            doc.editor.paste_from_clipboard();
            doc.sync_from_editor();
            sync_ui_from_doc(ui, doc);
        }
        "delete" => {
            doc.editor.delete_forward();
            doc.sync_from_editor();
            sync_ui_from_doc(ui, doc);
        }
        "select-all" => {
            doc.editor.select_all();
            sync_ui_from_doc(ui, doc);
        }
        "duplicate-line" => {
            doc.duplicate_lines();
            sync_ui_from_doc(ui, doc);
        }
        "delete-line" => {
            doc.delete_lines();
            sync_ui_from_doc(ui, doc);
        }
        "move-line-up" => {
            doc.move_lines_up();
            sync_ui_from_doc(ui, doc);
        }
        "move-line-down" => {
            doc.move_lines_down();
            sync_ui_from_doc(ui, doc);
        }

        "bookmark-toggle" => {
            doc.toggle_bookmark();
            sync_ui_from_doc(ui, doc);
        }
        "bookmark-next" => {
            doc.next_bookmark();
            sync_ui_from_doc(ui, doc);
        }
        "bookmark-prev" => {
            doc.prev_bookmark();
            sync_ui_from_doc(ui, doc);
        }
        "bookmark-clear" => {
            doc.clear_all_bookmarks();
            sync_ui_from_doc(ui, doc);
        }

        "find" => {
            ui.set_show_search_panel(true);
            ui.set_show_replace(false);
            populate_search_query_from_selection(ui, doc);
        }
        "find-next" => do_find_next(ui, doc),
        "find-prev" => do_find_prev(ui, doc),
        "replace" => {
            ui.set_show_search_panel(true);
            ui.set_show_replace(true);
            populate_search_query_from_selection(ui, doc);
        }
        "goto" => open_goto_panel(ui, doc),

        "mode-text" => do_set_mode(ui, doc, EditMode::Text),
        "mode-column" => do_set_mode(ui, doc, EditMode::Column),
        "mode-hex" => do_set_mode(ui, doc, EditMode::Hex),

        "pick-syntax" => open_picker(
            ui,
            "語法突顯",
            ui.get_syntax_list(),
            &ui.get_current_syntax(),
            "syntax",
            menu_title_x("檢視(V)"),
        ),
        "pick-newline" => open_picker(
            ui,
            "換行格式",
            ui.get_newline_list(),
            &ui.get_selected_newline(),
            "newline",
            menu_title_x("檢視(V)"),
        ),
        "pick-reinterpret" => open_picker(
            ui,
            "重新解碼（選擇編碼）",
            ui.get_encoding_list(),
            &ui.get_selected_encoding(),
            "reinterpret",
            menu_title_x("工具(T)"),
        ),
        "pick-convert" => open_picker(
            ui,
            "轉換檔案編碼（選擇編碼）",
            ui.get_encoding_list(),
            &ui.get_selected_encoding(),
            "convert",
            menu_title_x("工具(T)"),
        ),

        "syntax" => {
            if let Some(name) = arg {
                do_syntax_changed(ui, doc, name);
            }
        }
        "newline" => {
            if let Some(label) = arg {
                do_newline_changed(ui, doc, label);
            }
        }
        "reinterpret" => {
            if let Some(enc) = arg {
                do_reinterpret(ui, doc, enc);
            }
        }
        "convert" => {
            if let Some(enc) = arg {
                do_convert(ui, doc, enc);
            }
        }

        "about" => {
            ui.set_status_info("MadEdit 0.1.0 — Rust / Slint 重製版".into());
        }
        "quit" => {
            // 結束事件迴圈（回傳值僅表示是否成功退出）
            let _ = slint::quit_event_loop();
        }
        _ => {}
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new()?;
    let doc = Rc::new(RefCell::new(DocumentBuffer::new()));

    // 同步系統初始深淺色與控制字元顯示設定
    {
        let mut d = doc.borrow_mut();
        d.is_dark_mode = main_window.get_is_dark_mode();
        d.show_control_chars = main_window.get_show_control_chars();
    }

    // 初始化支援編碼清單與畫面狀態
    let encoding_items: Vec<SharedString> = SUPPORTED_ENCODINGS
        .iter()
        .map(|&s| SharedString::from(s))
        .collect();
    main_window.set_encoding_list(ModelRc::new(VecModel::from(encoding_items)));

    // 初始化支援語法清單
    let syntax_items: Vec<SharedString> = SUPPORTED_SYNTAXES
        .iter()
        .map(|&s| SharedString::from(s))
        .collect();
    main_window.set_syntax_list(ModelRc::new(VecModel::from(syntax_items)));

    // 初始化支援換行格式清單
    let newline_items: Vec<SharedString> = SUPPORTED_NEWLINES
        .iter()
        .map(|&s| SharedString::from(s))
        .collect();
    main_window.set_newline_list(ModelRc::new(VecModel::from(newline_items)));
    sync_ui_from_doc(&main_window, &doc.borrow());
    // 開發用：啟動後自動展開指定選單，方便截圖檢查彈出選單外觀
    // 游標閃爍計時器（每 500ms 切換閃爍狀態）
    let caret_timer = Timer::default();
    {
        let ui_handle = main_window.as_weak();
        caret_timer.start(
            TimerMode::Repeated,
            std::time::Duration::from_millis(500),
            move || {
                if let Some(ui) = ui_handle.upgrade() {
                    let visible = ui.get_caret_visible();
                    ui.set_caret_visible(!visible);
                }
            },
        );
    }

    // 外部檔案變更定時檢查計時器（每 1500ms 檢查一次磁碟 mtime）
    let file_watcher_timer = Timer::default();
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        file_watcher_timer.start(
            TimerMode::Repeated,
            std::time::Duration::from_millis(1500),
            move || {
                let has_change = doc_clone.borrow().check_external_change();
                if has_change {
                    let _ = ui_handle.upgrade_in_event_loop(|ui| {
                        if !ui.get_has_external_change() {
                            ui.set_has_external_change(true);
                        }
                    });
                }
            },
        );
    }

    // 開啟舊檔事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_open_file_clicked(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            do_open_file(&ui, &mut d);
        });
    }

    // 儲存檔案事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_save_file_clicked(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            do_save_file(&ui, &mut d);
        });
    }

    // 重新載入檔案事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_reload_file(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            do_reload_file(&ui, &mut d);
        });
    }

    // 忽略外部檔案變更事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_ignore_external_change(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            do_ignore_external_change(&ui, &mut d);
        });
    }

    // 編輯模式切換事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_mode_changed(move |mode_val| {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            do_set_mode(&ui, &mut d, EditMode::from(mode_val));
        });
    }

    // 深淺色主題切換事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_toggle_theme(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            d.is_dark_mode = !d.is_dark_mode;
            ui.set_is_dark_mode(d.is_dark_mode);
            sync_ui_from_doc(&ui, &d);
            ui.invoke_focus_editor();
        });
    }

    // 控制字元顯示切換事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_toggle_control_chars(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            d.show_control_chars = !d.show_control_chars;
            ui.set_show_control_chars(d.show_control_chars);
            sync_ui_from_doc(&ui, &d);
            ui.invoke_focus_editor();
        });
    }

    // 插入/覆寫模式切換事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_toggle_overwrite_mode(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            d.editor.toggle_overwrite_mode();
            sync_ui_from_doc(&ui, &d);
            ui.invoke_focus_editor();
        });
    }

    // 字級大小切換事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_font_size_changed(move |size_str| {
            let Some(ui) = ui_handle.upgrade() else { return; };
            if let Ok(sz) = size_str.trim().parse::<i32>() {
                ui.set_editor_font_size(sz);
                let d = doc_clone.borrow();
                sync_ui_from_doc(&ui, &d);
            }
            ui.invoke_focus_editor();
        });
    }

    // 選單列動作回呼綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_toggle_search_panel(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let next = !ui.get_show_search_panel();
            ui.set_show_search_panel(next);
            if next {
                let d = doc_clone.borrow();
                populate_search_query_from_selection(&ui, &d);
            } else {
                ui.invoke_focus_editor();
            }
        });
    }
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_toggle_replace_panel(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            ui.set_show_search_panel(true);
            ui.set_show_replace(true);
            let d = doc_clone.borrow();
            populate_search_query_from_selection(&ui, &d);
        });
    }
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_toggle_goto_panel(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            if ui.get_show_goto_panel() {
                ui.set_show_goto_panel(false);
                ui.invoke_focus_editor();
            } else {
                let d = doc_clone.borrow();
                open_goto_panel(&ui, &d);
            }
        });
    }
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_toggle_bookmark_clicked(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            d.toggle_bookmark();
            sync_ui_from_doc(&ui, &d);
            ui.invoke_focus_editor();
        });
    }
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_next_bookmark_clicked(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            d.next_bookmark();
            sync_ui_from_doc(&ui, &d);
            ui.invoke_focus_editor();
        });
    }

    // 重新解碼（Reinterpret Encoding）事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_reinterpret_encoding_clicked(move |enc_name| {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            do_reinterpret(&ui, &mut d, enc_name.as_str());
        });
    }

    // 轉換編碼（Convert Encoding）事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_convert_encoding_clicked(move |enc_name| {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            do_convert(&ui, &mut d, enc_name.as_str());
        });
    }

    // 語法突顯切換（Syntax Changed）事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_syntax_changed(move |syn_name| {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            do_syntax_changed(&ui, &mut d, syn_name.as_str());
        });
    }

    // 語法突顯切換事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_syntax_changed(move |syntax_name| {
            let mut d = doc_clone.borrow_mut();
            d.set_syntax(syntax_name.as_str());
            if let Some(ui) = ui_handle.upgrade() {
                sync_ui_from_doc(&ui, &d);
                ui.invoke_focus_editor();
            }
        });
    }

    // 換行格式轉換事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_newline_format_changed(move |nl_label| {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            do_newline_changed(&ui, &mut d, nl_label.as_str());
        });
    }

    // 選單列 / 工具列動作派發
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_menu_action(move |action| {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            handle_menu_action(&ui, &mut d, action.as_str());
        });
    }

    // 搜尋與取代面板事件綁定
    {
        let ui_handle = main_window.as_weak();
        main_window.on_close_search_panel(move || {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_show_search_panel(false);
                ui.invoke_focus_editor();
            }
        });
    }

    {
        let ui_handle = main_window.as_weak();
        main_window.on_toggle_replace_clicked(move || {
            if let Some(ui) = ui_handle.upgrade() {
                let cur = ui.get_show_replace();
                ui.set_show_replace(!cur);
            }
        });
    }

    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_search_query_changed(move |_| {
            if let Some(ui) = ui_handle.upgrade() {
                let d = doc_clone.borrow();
                update_search_status_ui(&ui, &d);
            }
        });
    }

    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_search_options_changed(move || {
            if let Some(ui) = ui_handle.upgrade() {
                let d = doc_clone.borrow();
                update_search_status_ui(&ui, &d);
            }
        });
    }

    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_find_next_clicked(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            do_find_next(&ui, &mut d);
        });
    }

    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_find_prev_clicked(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            do_find_prev(&ui, &mut d);
        });
    }

    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_replace_clicked(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            do_replace(&ui, &mut d);
        });
    }

    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_replace_all_clicked(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            do_replace_all(&ui, &mut d);
        });
    }

    // 跳轉確認事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_goto_confirmed(move |input_text| {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            do_goto_confirmed(&ui, &mut d, input_text.as_str());
        });
    }

    // 跳轉取消事件綁定
    {
        let ui_handle = main_window.as_weak();
        main_window.on_goto_cancelled(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            ui.set_show_goto_panel(false);
            ui.invoke_focus_editor();
        });
    }

    // 自繪編輯器鍵盤按鍵事件
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_key_pressed(move |key_text, ctrl, shift, alt| {
            let mut d = doc_clone.borrow_mut();
            let key_str = key_text.as_str();

            // 書籤快捷鍵處理 (Ctrl+Shift+F2, Ctrl+F2, Shift+F2, F2)
            if key_str.starts_with(char::from(Key::F2)) {
                if ctrl && shift {
                    d.clear_all_bookmarks();
                } else if ctrl {
                    d.toggle_bookmark();
                } else if shift {
                    d.prev_bookmark();
                } else {
                    d.next_bookmark();
                }
                if let Some(ui) = ui_handle.upgrade() {
                    sync_ui_from_doc(&ui, &d);
                }
                return true;
            }

            // 快捷鍵處理
            if ctrl {
                if shift {
                    match key_str {
                        "d" | "D" => {
                            d.duplicate_lines();
                        }
                        "k" | "K" => {
                            d.delete_lines();
                        }
                        _ => return false,
                    }
                } else {
                    match key_str {
                        "a" | "A" => {
                            d.editor.select_all();
                        }
                        "c" | "C" => {
                            d.editor.copy_to_clipboard();
                        }
                        "x" | "X" => {
                            d.editor.cut_to_clipboard();
                            d.sync_from_editor();
                        }
                        "v" | "V" => {
                            d.editor.paste_from_clipboard();
                            d.sync_from_editor();
                        }
                        "s" | "S" => {
                            drop(d);
                            if let Some(ui) = ui_handle.upgrade() {
                                ui.invoke_save_file_clicked();
                            }
                            return true;
                        }
                        "o" | "O" => {
                            drop(d);
                            if let Some(ui) = ui_handle.upgrade() {
                                ui.invoke_open_file_clicked();
                            }
                            return true;
                        }
                        "f" | "F" => {
                            if let Some(ui) = ui_handle.upgrade() {
                                ui.set_show_search_panel(true);
                                populate_search_query_from_selection(&ui, &d);
                            }
                        }
                        "h" | "H" => {
                            if let Some(ui) = ui_handle.upgrade() {
                                ui.set_show_search_panel(true);
                                ui.set_show_replace(true);
                                populate_search_query_from_selection(&ui, &d);
                            }
                        }
                        "g" | "G" => {
                            if let Some(ui) = ui_handle.upgrade() {
                                if ui.get_show_goto_panel() {
                                    ui.set_show_goto_panel(false);
                                    ui.invoke_focus_editor();
                                } else {
                                    open_goto_panel(&ui, &d);
                                }
                            }
                        }
                        _ => return false,
                    }
                }
                if let Some(ui) = ui_handle.upgrade() {
                    sync_ui_from_doc(&ui, &d);
                }
                return true;
            }

            // Alt+方向鍵 行上下移動
            if alt && !ctrl && !shift {
                let moved = if key_str.starts_with(char::from(Key::UpArrow)) {
                    d.move_lines_up()
                } else if key_str.starts_with(char::from(Key::DownArrow)) {
                    d.move_lines_down()
                } else {
                    false
                };
                if moved {
                    if let Some(ui) = ui_handle.upgrade() {
                        sync_ui_from_doc(&ui, &d);
                    }
                    return true;
                }
            }

            // 導覽鍵與編輯鍵處理
            let handled = if key_str.starts_with(char::from(Key::LeftArrow)) {
                d.editor.move_left(shift);
                true
            } else if key_str.starts_with(char::from(Key::RightArrow)) {
                d.editor.move_right(shift);
                true
            } else if key_str.starts_with(char::from(Key::UpArrow)) {
                d.editor.move_up(shift);
                true
            } else if key_str.starts_with(char::from(Key::DownArrow)) {
                d.editor.move_down(shift);
                true
            } else if key_str.starts_with(char::from(Key::Home)) {
                d.move_home_smart(shift);
                true
            } else if key_str.starts_with(char::from(Key::End)) {
                d.editor.move_end(shift);
                true
            } else if key_str.starts_with(char::from(Key::PageUp)) {
                d.editor.move_page_up(shift);
                true
            } else if key_str.starts_with(char::from(Key::PageDown)) {
                d.editor.move_page_down(shift);
                true
            } else if key_str.starts_with(char::from(Key::Insert)) {
                d.editor.toggle_overwrite_mode();
                true
            } else if key_str.starts_with(char::from(Key::Backspace)) || key_str == "\u{0008}" {
                d.editor.delete_backspace();
                d.sync_from_editor();
                true
            } else if key_str.starts_with(char::from(Key::Delete)) || key_str == "\u{007f}" {
                d.editor.delete_forward();
                d.sync_from_editor();
                true
            } else if key_str.starts_with(char::from(Key::Return))
                || key_str == "\n"
                || key_str == "\r"
            {
                d.editor.insert_newline();
                d.sync_from_editor();
                true
            } else if key_str.starts_with(char::from(Key::Tab)) || key_str == "\t" {
                d.editor.insert_char('\t');
                d.sync_from_editor();
                true
            } else if key_str.starts_with(char::from(Key::F3)) {
                if let Some(ui) = ui_handle.upgrade() {
                    let query = ui.get_search_query();
                    let opts = get_search_options(&ui);
                    let res = if shift {
                        d.find_prev(query.as_str(), &opts)
                    } else {
                        d.find_next(query.as_str(), &opts)
                    };
                    update_search_ui_feedback(&ui, res, query.as_str());
                }
                true
            } else if key_str.starts_with(char::from(Key::Escape)) {
                if let Some(ui) = ui_handle.upgrade() {
                    if ui.get_show_goto_panel() {
                        ui.set_show_goto_panel(false);
                        ui.invoke_focus_editor();
                        true
                    } else if ui.get_show_search_panel() {
                        ui.set_show_search_panel(false);
                        ui.invoke_focus_editor();
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else if !key_str.is_empty() && !key_str.chars().any(|c| c.is_control()) {
                d.editor.insert_text(key_str);
                d.sync_from_editor();
                true
            } else {
                false
            };

            if handled && shift && (alt || d.edit_mode == EditMode::Column) {
                let anchor = d.editor.selection.anchor;
                let head = d.editor.selection.head;
                let anchor_line_text = d
                    .editor
                    .lines
                    .get(anchor.line)
                    .map(|s| s.as_str())
                    .unwrap_or("");
                let head_line_text = d
                    .editor
                    .lines
                    .get(head.line)
                    .map(|s| s.as_str())
                    .unwrap_or("");
                let a_vcol = char_col_to_visual_col(anchor_line_text, anchor.col);
                let h_vcol = char_col_to_visual_col(head_line_text, head.col);
                d.editor.selection = Selection::block(anchor, head, a_vcol, h_vcol);
            }

            if handled {
                if let Some(ui) = ui_handle.upgrade() {
                    ui.set_caret_visible(true); // 按鍵時立即顯示游標
                    sync_ui_from_doc(&ui, &d);
                }
            }
            handled
        });
    }

    // 自繪編輯器滑鼠點擊定位（支援列塊模式與 Alt 鍵矩形定位）
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_mouse_clicked(move |x, y, alt| {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let (cell_width, line_height) = calc_editor_metrics(ui.get_editor_font_size());
            let mut d = doc_clone.borrow_mut();
            let is_block_mode = d.edit_mode == EditMode::Column || alt;
            let target_vcol = (x / cell_width).max(0.0).round() as usize;
            let pos = d.editor.pos_from_pixel(x, y, cell_width, line_height);
            if is_block_mode {
                d.editor.selection = Selection::block(pos, pos, target_vcol, target_vcol);
            } else {
                d.editor.set_caret(pos.line, pos.col);
            }
            ui.set_caret_visible(true);
            sync_ui_from_doc(&ui, &d);
        });
    }

    // 自繪編輯器滑鼠拖曳選取（支援列塊模式與 Alt 鍵矩形選取）
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_mouse_dragged(move |x, y, alt| {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let (cell_width, line_height) = calc_editor_metrics(ui.get_editor_font_size());
            let mut d = doc_clone.borrow_mut();
            let is_block_mode =
                d.edit_mode == EditMode::Column || alt || d.editor.selection.is_block();
            let target_vcol = (x / cell_width).max(0.0).round() as usize;
            let pos = d.editor.pos_from_pixel(x, y, cell_width, line_height);
            if is_block_mode {
                d.editor.update_block_selection(pos, target_vcol);
            } else {
                d.editor.selection.head = pos;
            }
            sync_ui_from_doc(&ui, &d);
        });
    }

    // 自繪編輯器滾輪捲動
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_scrolled(move |delta_y| {
            let mut d = doc_clone.borrow_mut();
            if delta_y > 0.0 {
                d.editor.scroll_by(-3);
            } else if delta_y < 0.0 {
                d.editor.scroll_by(3);
            }
            if let Some(ui) = ui_handle.upgrade() {
                sync_ui_from_doc(&ui, &d);
            }
        });
    }

    // 垂直捲軸滑塊跳轉與拖曳
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_vscroll_to(move |target_line| {
            let mut d = doc_clone.borrow_mut();
            d.editor.scroll_to_line(target_line.max(0) as usize);
            if let Some(ui) = ui_handle.upgrade() {
                sync_ui_from_doc(&ui, &d);
            }
        });
    }

    // 垂直捲軸上下按鈕微調
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_vscroll_by(move |delta_lines| {
            let mut d = doc_clone.borrow_mut();
            d.editor.scroll_by(delta_lines);
            if let Some(ui) = ui_handle.upgrade() {
                sync_ui_from_doc(&ui, &d);
            }
        });
    }

    // 水平捲軸滑塊跳轉與拖曳
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_hscroll_to(move |target_col| {
            let mut d = doc_clone.borrow_mut();
            d.editor.scroll_to_col(target_col.max(0) as usize);
            if let Some(ui) = ui_handle.upgrade() {
                sync_ui_from_doc(&ui, &d);
            }
        });
    }

    // 水平捲軸左右按鈕與橫向滾輪微調
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_hscroll_by(move |delta_cols| {
            let mut d = doc_clone.borrow_mut();
            d.editor.scroll_horizontal_by(delta_cols);
            if let Some(ui) = ui_handle.upgrade() {
                sync_ui_from_doc(&ui, &d);
            }
        });
    }

    // 命令列指定的檔案：於啟動時直接開啟（對應原版 MadEdit 的行為）
    {
        let startup_path = std::env::args()
            .skip(1)
            .find(|arg| !arg.starts_with('-') && std::path::Path::new(arg).is_file());
        if let Some(path) = startup_path {
            let mut d = doc.borrow_mut();
            if let Err(e) = d.load_file(std::path::Path::new(&path)) {
                eprintln!("Failed to open file {}: {}", path, e);
            } else {
                sync_ui_from_doc(&main_window, &d);
            }
        }
    }

    main_window.run()
}
