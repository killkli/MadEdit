mod core;

use std::cell::RefCell;
use std::rc::Rc;
use core::{DocumentBuffer, EditMode, SUPPORTED_ENCODINGS};
use slint::{ModelRc, SharedString, VecModel};

slint::include_modules!();

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
    ui.set_text_content(doc.text.clone().into());

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

    // 儲存檔案事件綁定（若檔案含無損解碼錯誤則拒絕存檔並在狀態列提示）
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

    // 文字內容變更事件綁定
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_text_changed(move |new_text| {
            let mut d = doc_clone.borrow_mut();
            d.update_text(new_text.to_string());
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_status_info(format_status_info(&d).into());
            }
        });
    }

    main_window.run()
}
