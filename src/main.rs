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

    // 同步原版 MadEdit 5 格狀態列資訊
    let head = doc.editor.selection.head;
    let total_lines = doc
        .line_index
        .line_count()
        .max(doc.editor.line_count())
        .max(1);
    let cur_line = head.line + 1;
    let cur_col = head.col + 1;

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
        ui.set_caret_status(block_status.clone().into());
        ui.set_status_line_col(block_status.into());
    } else {
        let line_col_status = format!("行 : {} / {}   列 : {}", cur_line, total_lines, cur_col);
        ui.set_caret_status(line_col_status.clone().into());
        ui.set_status_line_col(line_col_status.into());
    }

    let (char_off, byte_off, sel_chars, sel_bytes) = doc.cursor_and_selection_metrics();
    let decode_warn = if doc.has_decode_error {
        "  [⚠️ 含無法無損解碼字元]"
    } else {
        ""
    };
    ui.set_status_char_pos(
        format!(
            "字元位置 : {} ({:X}H) / {}{}",
            char_off,
            byte_off,
            doc.len(),
            decode_warn
        )
        .into(),
    );

    if sel_chars > 0 {
        ui.set_status_sel_len(format!("選取長度 : {} ({} B)", sel_chars, sel_bytes).into());
    } else {
        ui.set_status_sel_len("選取長度 : 0".into());
    }

    ui.set_status_enc_format(doc.encoding_status_code().into());
    ui.set_status_insert_mode(
        (if doc.editor.overwrite_mode {
            "覆寫"
        } else {
            "插入"
        })
        .into(),
    );

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
            let Some(path) = rfd::FileDialog::new().pick_file() else {
                if let Some(ui) = ui_handle.upgrade() {
                    ui.invoke_focus_editor();
                }
                return;
            };

            let mut d = doc_clone.borrow_mut();
            if let Err(e) = d.load_file(&path) {
                eprintln!("Failed to open file: {}", e);
                return;
            }

            if let Some(ui) = ui_handle.upgrade() {
                sync_ui_from_doc(&ui, &d);
                ui.invoke_focus_editor();
            }
        });
    }

    // 儲存檔案事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_save_file_clicked(move || {
            let target_path = {
                let d = doc_clone.borrow();
                d.file_path
                    .clone()
                    .or_else(|| rfd::FileDialog::new().save_file())
            };

            if let Some(path) = target_path {
                let mut d = doc_clone.borrow_mut();
                if let Err(e) = d.save_file(Some(path)) {
                    eprintln!("Failed to save file: {}", e);
                    if let Some(ui) = ui_handle.upgrade() {
                        ui.set_status_char_pos(format!("⚠️ 存檔拒絕: {}", e).into());
                        ui.invoke_focus_editor();
                    }
                    return;
                }
                if let Some(ui) = ui_handle.upgrade() {
                    sync_ui_from_doc(&ui, &d);
                    ui.invoke_focus_editor();
                }
            } else if let Some(ui) = ui_handle.upgrade() {
                ui.invoke_focus_editor();
            }
        });
    }

    // 重新載入檔案事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_reload_file(move || {
            let mut d = doc_clone.borrow_mut();
            if let Err(e) = d.reload_file() {
                eprintln!("Failed to reload file: {}", e);
                return;
            }
            if let Some(ui) = ui_handle.upgrade() {
                sync_ui_from_doc(&ui, &d);
                ui.invoke_focus_editor();
            }
        });
    }

    // 忽略外部檔案變更事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_ignore_external_change(move || {
            let mut d = doc_clone.borrow_mut();
            d.ignore_external_change();
            if let Some(ui) = ui_handle.upgrade() {
                sync_ui_from_doc(&ui, &d);
                ui.invoke_focus_editor();
            }
        });
    }

    // 編輯模式切換事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_mode_changed(move |mode_val| {
            let mode = EditMode::from(mode_val);
            let mut d = doc_clone.borrow_mut();
            d.set_edit_mode(mode);

            if let Some(ui) = ui_handle.upgrade() {
                sync_ui_from_doc(&ui, &d);
                ui.invoke_focus_editor();
            }
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
        main_window.on_reinterpret_encoding_clicked(move |enc_label| {
            let mut d = doc_clone.borrow_mut();
            if d.reinterpret_with_encoding(enc_label.as_str()) {
                if let Some(ui) = ui_handle.upgrade() {
                    sync_ui_from_doc(&ui, &d);
                }
            }
            if let Some(ui) = ui_handle.upgrade() {
                ui.invoke_focus_editor();
            }
        });
    }

    // 轉換編碼（Convert Encoding）事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_convert_encoding_clicked(move |enc_label| {
            let mut d = doc_clone.borrow_mut();
            if d.convert_to_encoding(enc_label.as_str()) {
                if let Some(ui) = ui_handle.upgrade() {
                    sync_ui_from_doc(&ui, &d);
                }
            }
            if let Some(ui) = ui_handle.upgrade() {
                ui.invoke_focus_editor();
            }
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
            if let Some(fmt) = NewLineFormat::from_label(nl_label.as_str()) {
                let mut d = doc_clone.borrow_mut();
                if d.convert_newline_format(fmt) {
                    if let Some(ui) = ui_handle.upgrade() {
                        sync_ui_from_doc(&ui, &d);
                    }
                }
            }
            if let Some(ui) = ui_handle.upgrade() {
                ui.invoke_focus_editor();
            }
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
            let query = ui.get_search_query();
            let opts = get_search_options(&ui);
            let mut d = doc_clone.borrow_mut();
            let res = d.find_next(query.as_str(), &opts);
            update_search_ui_feedback(&ui, res, query.as_str());
            sync_ui_from_doc(&ui, &d);
        });
    }

    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_find_prev_clicked(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let query = ui.get_search_query();
            let opts = get_search_options(&ui);
            let mut d = doc_clone.borrow_mut();
            let res = d.find_prev(query.as_str(), &opts);
            update_search_ui_feedback(&ui, res, query.as_str());
            sync_ui_from_doc(&ui, &d);
        });
    }

    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_replace_clicked(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let query = ui.get_search_query();
            let replace_with = ui.get_replace_text();
            let opts = get_search_options(&ui);
            let mut d = doc_clone.borrow_mut();
            if d.replace_current(query.as_str(), replace_with.as_str(), &opts) {
                let res = d.find_next(query.as_str(), &opts);
                update_search_ui_feedback(&ui, res, query.as_str());
            } else {
                let res = d.find_next(query.as_str(), &opts);
                update_search_ui_feedback(&ui, res, query.as_str());
            }
            sync_ui_from_doc(&ui, &d);
        });
    }

    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_replace_all_clicked(move || {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let query = ui.get_search_query();
            let replace_with = ui.get_replace_text();
            let opts = get_search_options(&ui);
            let mut d = doc_clone.borrow_mut();
            let count = d.replace_all(query.as_str(), replace_with.as_str(), &opts);
            ui.set_search_status_text(format!("已取代 {} 處", count).into());
            sync_ui_from_doc(&ui, &d);
        });
    }

    // 跳轉確認事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_goto_confirmed(move |input_text| {
            let Some(ui) = ui_handle.upgrade() else { return; };
            let mut d = doc_clone.borrow_mut();
            ui.set_show_goto_panel(false);

            if d.edit_mode == EditMode::Hex {
                if let Some(offset) = parse_goto_offset(input_text.as_str()) {
                    d.goto_offset(offset);
                }
            } else if let Some((line, col)) = parse_goto_line_col(input_text.as_str()) {
                d.goto_line_col(line, col);
            }

            sync_ui_from_doc(&ui, &d);
            ui.invoke_focus_editor();
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

    main_window.run()
}
