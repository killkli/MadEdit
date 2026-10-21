mod core;

use std::cell::RefCell;
use std::rc::Rc;
use core::{DocumentBuffer, EditMode, SUPPORTED_ENCODINGS};
use slint::platform::Key;
use slint::{ModelRc, SharedString, Timer, TimerMode, VecModel};

slint::include_modules!();

const CELL_WIDTH: f32 = 8.5;
const LINE_HEIGHT: f32 = 20.0;

/// 產生狀態列顯示字串，包含位元組長度、修改標記與無損解碼錯誤警告
fn format_status_info(doc: &DocumentBuffer) -> String {
    let modified_mark = if doc.is_modified { " *" } else { "" };
    let error_warn = if doc.has_decode_error {
        " [警告: 含無法無損解碼字元，請重新解碼或轉換編碼]"
    } else {
        ""
    };
    format!("Len: {} bytes{}{}", doc.raw_bytes.len(), modified_mark, error_warn)
}

/// 將核心 DocumentBuffer 的完整狀態同步至 Slint 主視窗屬性
fn sync_ui_from_doc(ui: &MainWindow, doc: &DocumentBuffer) {
    let mode_idx = match doc.edit_mode {
        EditMode::Text => 0,
        EditMode::Column => 1,
        EditMode::Hex => 2,
    };

    ui.set_file_path(doc.file_name().into());
    ui.set_edit_mode(mode_idx);
    ui.set_encoding_name(doc.encoding.name().into());
    ui.set_selected_encoding(doc.encoding.name().into());
    ui.set_newline_format(doc.newline_format.as_str().into());

    // 同步可見行自繪資料
    let visible_lines_data = doc.editor.generate_visible_lines(CELL_WIDTH);
    let slint_lines: Vec<VisibleLine> = visible_lines_data
        .into_iter()
        .map(|l| VisibleLine {
            line_number: l.line_number,
            text: l.text.into(),
            has_selection: l.has_selection,
            sel_start_x: l.sel_start_x,
            sel_width: l.sel_width,
            has_caret: l.has_caret,
            caret_x: l.caret_x,
        })
        .collect();
    ui.set_visible_lines(ModelRc::new(VecModel::from(slint_lines)));

    // 同步游標位置座標資訊
    let head = doc.editor.selection.head;
    ui.set_cursor_status(format!("Ln {}, Col {}", head.line + 1, head.col + 1).into());

    if doc.edit_mode == EditMode::Hex {
        let dump = doc.get_hex_dump();
        ui.set_hex_offset_text(dump.offsets.into());
        ui.set_hex_content_text(dump.hex.into());
        ui.set_hex_ascii_text(dump.ascii.into());
    }

    ui.set_status_info(format_status_info(doc).into());
}

fn main() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new()?;
    let doc = Rc::new(RefCell::new(DocumentBuffer::new()));

    // 初始化支援編碼清單與畫面狀態
    let encoding_items: Vec<SharedString> = SUPPORTED_ENCODINGS
        .iter()
        .map(|&s| SharedString::from(s))
        .collect();
    main_window.set_encoding_list(ModelRc::new(VecModel::from(encoding_items)));
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

    // 開啟舊檔事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_open_file_clicked(move || {
            let Some(path) = rfd::FileDialog::new().pick_file() else {
                return;
            };

            let mut d = doc_clone.borrow_mut();
            if let Err(e) = d.load_file(&path) {
                eprintln!("Failed to open file: {}", e);
                return;
            }

            if let Some(ui) = ui_handle.upgrade() {
                sync_ui_from_doc(&ui, &d);
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
                d.file_path.clone().or_else(|| rfd::FileDialog::new().save_file())
            };

            if let Some(path) = target_path {
                let mut d = doc_clone.borrow_mut();
                if let Err(e) = d.save_file(Some(path)) {
                    eprintln!("Failed to save file: {}", e);
                    if let Some(ui) = ui_handle.upgrade() {
                        ui.set_status_info(format!("存檔拒絕: {}", e).into());
                    }
                    return;
                }
                if let Some(ui) = ui_handle.upgrade() {
                    sync_ui_from_doc(&ui, &d);
                }
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
            }
        });
    }

    // 重新解碼（Reinterpret Encoding）事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_reinterpret_encoding_clicked(move |enc_name| {
            let mut d = doc_clone.borrow_mut();
            if d.reinterpret_with_encoding(enc_name.as_str()) {
                if let Some(ui) = ui_handle.upgrade() {
                    sync_ui_from_doc(&ui, &d);
                }
            }
        });
    }

    // 轉換編碼（Convert Encoding）事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_convert_encoding_clicked(move |enc_name| {
            let mut d = doc_clone.borrow_mut();
            if d.convert_to_encoding(enc_name.as_str()) {
                if let Some(ui) = ui_handle.upgrade() {
                    sync_ui_from_doc(&ui, &d);
                }
            }
        });
    }

    // 自繪編輯器鍵盤按鍵事件
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_key_pressed(move |key_text, ctrl, shift| {
            let mut d = doc_clone.borrow_mut();
            let key_str = key_text.as_str();

            // 快捷鍵處理
            if ctrl {
                match key_str {
                    "a" | "A" => {
                        d.editor.select_all();
                    }
                    "c" | "C" => {
                        let text = d.editor.selected_text();
                        if !text.is_empty() {
                            // 可擴充寫入系統剪貼簿
                        }
                    }
                    "x" | "X" => {
                        let text = d.editor.selected_text();
                        if !text.is_empty() {
                            d.editor.delete_selection();
                            d.sync_from_editor();
                        }
                    }
                    _ => return false,
                }
                if let Some(ui) = ui_handle.upgrade() {
                    sync_ui_from_doc(&ui, &d);
                }
                return true;
            }

            // 導覽鍵處理
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
                d.editor.move_home(shift);
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
            } else if key_str.starts_with(char::from(Key::Backspace)) || key_str == "\u{0008}" {
                d.editor.delete_backspace();
                d.sync_from_editor();
                true
            } else if key_str.starts_with(char::from(Key::Delete)) || key_str == "\u{007f}" {
                d.editor.delete_forward();
                d.sync_from_editor();
                true
            } else if key_str.starts_with(char::from(Key::Return)) || key_str == "\n" || key_str == "\r" {
                d.editor.insert_newline();
                d.sync_from_editor();
                true
            } else if key_str.starts_with(char::from(Key::Tab)) || key_str == "\t" {
                d.editor.insert_char('\t');
                d.sync_from_editor();
                true
            } else if !key_str.is_empty() && !key_str.chars().any(|c| c.is_control()) {
                d.editor.insert_text(key_str);
                d.sync_from_editor();
                true
            } else {
                false
            };

            if handled {
                if let Some(ui) = ui_handle.upgrade() {
                    ui.set_caret_visible(true); // 按鍵時立即顯示游標
                    sync_ui_from_doc(&ui, &d);
                }
            }
            handled
        });
    }

    // 自繪編輯器滑鼠點擊定位
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_mouse_clicked(move |x, y, _is_drag| {
            let mut d = doc_clone.borrow_mut();
            let pos = d.editor.pos_from_pixel(x, y, CELL_WIDTH, LINE_HEIGHT);
            d.editor.set_caret(pos.line, pos.col);
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_caret_visible(true);
                sync_ui_from_doc(&ui, &d);
            }
        });
    }

    // 自繪編輯器滑鼠拖曳選取
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_mouse_dragged(move |x, y| {
            let mut d = doc_clone.borrow_mut();
            let pos = d.editor.pos_from_pixel(x, y, CELL_WIDTH, LINE_HEIGHT);
            d.editor.selection.head = pos;
            if let Some(ui) = ui_handle.upgrade() {
                sync_ui_from_doc(&ui, &d);
            }
        });
    }

    // 自繪編輯器滾輪捲動
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_scrolled(move |delta_y| {
            let mut d = doc_clone.borrow_mut();
            if delta_y > 0.0 {
                d.editor.viewport.first_visible_line =
                    d.editor.viewport.first_visible_line.saturating_sub(3);
            } else if delta_y < 0.0 {
                let max_first = d.editor.lines.len().saturating_sub(1);
                d.editor.viewport.first_visible_line =
                    (d.editor.viewport.first_visible_line + 3).min(max_first);
            }
            if let Some(ui) = ui_handle.upgrade() {
                sync_ui_from_doc(&ui, &d);
            }
        });
    }

    main_window.run()
}
