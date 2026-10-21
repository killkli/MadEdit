mod core;

use std::cell::RefCell;
use std::rc::Rc;
use core::{DocumentBuffer, EditMode};

slint::include_modules!();

fn sync_ui_from_doc(ui: &MainWindow, doc: &DocumentBuffer) {
    let mode_idx = match doc.edit_mode {
        EditMode::Text => 0,
        EditMode::Column => 1,
        EditMode::Hex => 2,
    };

    ui.set_file_path(doc.file_name().into());
    ui.set_edit_mode(mode_idx);
    ui.set_encoding_name(doc.encoding.name().into());
    ui.set_newline_format(doc.newline_format.as_str().into());
    ui.set_text_content(doc.text.clone().into());

    if doc.edit_mode == EditMode::Hex {
        let dump = doc.get_hex_dump();
        ui.set_hex_offset_text(dump.offsets.into());
        ui.set_hex_content_text(dump.hex.into());
        ui.set_hex_ascii_text(dump.ascii.into());
    }

    let modified_mark = if doc.is_modified { " *" } else { "" };
    ui.set_status_info(format!("Len: {} bytes{}", doc.raw_bytes.len(), modified_mark).into());
}

fn main() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new()?;
    let doc = Rc::new(RefCell::new(DocumentBuffer::new()));

    // Initial UI state
    sync_ui_from_doc(&main_window, &doc.borrow());

    // Open file
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

    // Save file
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
                    return;
                }
                if let Some(ui) = ui_handle.upgrade() {
                    sync_ui_from_doc(&ui, &d);
                }
            }
        });
    }

    // Mode changed
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

    // Text changed
    {
        let ui_handle = main_window.as_weak();
        let doc_clone = Rc::clone(&doc);
        main_window.on_text_changed(move |new_text| {
            let mut d = doc_clone.borrow_mut();
            d.update_text(new_text.to_string());
            if let Some(ui) = ui_handle.upgrade() {
                let modified_mark = if d.is_modified { " *" } else { "" };
                ui.set_status_info(format!("Len: {} bytes{}", d.raw_bytes.len(), modified_mark).into());
            }
        });
    }

    main_window.run()
}
