//! 以 egui_kittest 模擬鍵盤與輸入法事件的整合測試（不開視窗、不需 GPU）

use eframe::egui::{Event, ImeEvent, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::commands::C;
use super::{App, Confirm};

fn harness() -> Harness<'static, App> {
    let mut h = Harness::builder().allow_missing_glyphs().with_size([1000.0, 700.0]).build_eframe(|cc| App::new(cc, Vec::new(), None));
    // 測試機的系統語言不一定是繁體中文，按鈕文字固定用原文
    h.state_mut().settings.ui_language = crate::core::settings::UiLanguage::ZhTw;
    h.run_steps(3);
    h
}

fn type_text(h: &mut Harness<'static, App>, s: &str) {
    h.event(Event::Text(s.to_string()));
    h.run_steps(2);
}

fn text(h: &mut Harness<'static, App>) -> String {
    h.state_mut().doc().editor.lines.join("\n")
}

#[test]
fn test_instance_request_restores_window_without_running_ui() {
    let dir = std::env::temp_dir().join(format!("madedit-hidden-instance-{}", std::process::id()));
    let instance = crate::single_instance::Instance::start(&dir, vec![]).unwrap().unwrap();
    let file = dir.join("external.txt");
    std::fs::write(&file, "external document").unwrap();
    let mut h = harness();
    h.state_mut().instance = Some(instance);
    assert!(crate::single_instance::Instance::start(&dir, vec![file]).unwrap().is_none());
    let ctx = eframe::egui::Context::default();
    let mut frame = eframe::Frame::_new_kittest();
    // 模擬最小化時只有邏輯回呼的情況，不能依賴 ui 才取出要求。
    let output = ctx.run_logic(&Default::default(), |ctx| {
        eframe::App::logic(h.state_mut(), ctx, &mut frame);
    });
    assert_eq!(text(&mut h), "external document");
    let commands: Vec<_> = output.viewport_commands.iter().flat_map(|(_, commands)| commands).collect();
    assert!(commands.iter().any(|cmd| matches!(cmd, eframe::egui::ViewportCommand::Minimized(false))));
    assert!(commands.iter().any(|cmd| matches!(cmd, eframe::egui::ViewportCommand::Focus)));
    drop(h.state_mut().instance.take());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn test_external_open_reuses_tabs_and_preserves_unsaved_text() {
    let dir = std::env::temp_dir().join(format!("madedit-external-open-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let first = dir.join("中文 空白.txt");
    let second = dir.join("second.txt");
    std::fs::write(&first, "first").unwrap();
    std::fs::write(&second, "second").unwrap();
    let mut h = harness();
    type_text(&mut h, "尚未儲存");
    let unsaved_id = h.state().doc().id;
    let ctx = eframe::egui::Context::default();
    h.state_mut().open_external_files(&ctx, vec![first.clone(), second.clone()]);
    assert_eq!(h.state().tabs.len(), 3);
    assert_eq!(h.state().doc().path.as_ref(), Some(&second.canonicalize().unwrap()));
    h.state_mut().open_external_files(&ctx, vec![first.clone(), first]);
    assert_eq!(h.state().tabs.len(), 3);
    assert_eq!(text(&mut h), "first");
    h.state_mut().open_external_files(&ctx, vec![]);
    assert_eq!(h.state().tabs.len(), 3);
    let unsaved = h.state().tabs.iter().find(|t| t.doc.id == unsaved_id).unwrap();
    assert_eq!(unsaved.doc.editor.lines.join("\n"), "尚未儲存");
    assert!(unsaved.doc.is_modified());
    std::fs::remove_dir_all(dir).unwrap();
}

/// 以指定文件量測開檔到並排預覽排版完成，以及輸入文字後的更新；不含原生視窗與 GPU 呈現。
/// 執行：MADEDIT_MD_BENCH_FILE=文件 cargo test --release benchmark_markdown_preview_ui -- --ignored --nocapture
#[test]
#[ignore]
fn benchmark_markdown_preview_ui() {
    use std::time::{Duration, Instant};
    use super::mdpreview::Placement;
    let path = std::path::PathBuf::from(std::env::var_os("MADEDIT_MD_BENCH_FILE").expect("請設定 MADEDIT_MD_BENCH_FILE"));
    let mut first = Vec::new();
    let mut edit = Vec::new();
    for sample in 0..12 {
        let start = Instant::now();
        let mut h = Harness::builder().allow_missing_glyphs().with_size([1400.0, 900.0]).build_eframe(|cc| {
            let mut app = App::new(cc, vec![path.clone()], None);
            app.run(&cc.egui_ctx, C::PreviewMarkdown);
            app.tab_mut().md_preview.as_mut().unwrap().set_placement(Placement::Right);
            app
        });
        h.run_steps(5);
        let elapsed = start.elapsed();
        let pv = h.state_mut().tab_mut().md_preview.as_ref().unwrap();
        assert!(!pv.anchors.is_empty());
        assert!((pv.rect.width() - 700.0).abs() < 20.0, "指定文件也須維持左右各半，預覽寬度 {}", pv.rect.width());
        if sample >= 3 {
            first.push(elapsed);
        }
        let version = h.state_mut().doc().version;
        h.event(Event::Text("測試內文".into()));
        let start = Instant::now();
        h.run_steps(3);
        let elapsed = start.elapsed();
        assert!(h.state_mut().doc().version > version, "量測的輸入事件必須實際修改文件");
        if sample >= 3 {
            edit.push(elapsed);
        }
        // 選用的離屏截圖在計時完成後才執行，不將 GPU 初始化算入效能樣本。
        if sample == 11 {
            if let Some(out) = std::env::var_os("MADEDIT_MD_BENCH_SHOT") {
                if let Ok(line) = std::env::var("MADEDIT_MD_BENCH_TOP_LINE").unwrap_or_default().parse::<usize>() {
                    h.state_mut().tab_mut().view.top_row = line;
                }
                h.run_steps(5);
                h.render().expect("預覽截圖失敗").save(out).unwrap();
            }
        }
    }
    let report = |name: &str, mut samples: Vec<Duration>| {
        samples.sort();
        println!("{name}: median={:.3} ms p90={:.3} ms", samples[samples.len() / 2].as_secs_f64() * 1000.0, samples[samples.len() * 9 / 10].as_secs_f64() * 1000.0);
    };
    println!("包含開檔、字型、圖表點陣化與預覽排版；每項 9 次有效樣本");
    report("首次開檔與預覽", first);
    report("輸入內文與更新預覽", edit);
}

/// 以含超長單行的文件量測開檔、輸入、移動游標、選取與捲動的畫面更新耗時；不含原生視窗與 GPU 呈現。
/// 執行：MADEDIT_LONGLINE_BENCH_FILE=文件 cargo test --release benchmark_long_line_ -- --ignored --nocapture
#[test]
#[ignore]
fn benchmark_long_line_editing() {
    use std::time::{Duration, Instant};

    use crate::core::settings::WrapMode;

    let path = std::path::PathBuf::from(std::env::var_os("MADEDIT_LONGLINE_BENCH_FILE").expect("請設定 MADEDIT_LONGLINE_BENCH_FILE"));

    fn samples(wrap: WrapMode, path: &std::path::Path) {
        let mut open = Vec::new();
        let mut typing = Vec::new();
        let mut caret = Vec::new();
        let mut select = Vec::new();
        let mut scroll = Vec::new();
        let mut click = Vec::new();
        for sample in 0..6 {
            let start = Instant::now();
            let mut h = Harness::builder().allow_missing_glyphs().with_size([1400.0, 900.0]).build_eframe(|cc| {
                let mut app = App::new(cc, vec![path.to_path_buf()], None);
                app.settings.wrap_mode = wrap;
                app
            });
            h.run_steps(3);
            if sample >= 1 {
                open.push(start.elapsed());
            }
            for _ in 0..8 {
                let t = Instant::now();
                h.event(Event::Text("x".into()));
                h.run_steps(1);
                if sample >= 1 {
                    typing.push(t.elapsed());
                }
            }
            let t = Instant::now();
            h.key_press(Key::End);
            h.run_steps(1);
            if sample >= 1 {
                caret.push(t.elapsed());
            }
            let t = Instant::now();
            h.key_press_modifiers(Modifiers::SHIFT, Key::End);
            h.run_steps(1);
            if sample >= 1 {
                select.push(t.elapsed());
            }
            let t = Instant::now();
            for _ in 0..5 {
                h.key_press(Key::PageDown);
                h.run_steps(1);
            }
            if sample >= 1 {
                scroll.push(t.elapsed());
            }
            let t = Instant::now();
            h.hover_at(eframe::egui::pos2(600.0, 400.0));
            h.drag_at(eframe::egui::pos2(600.0, 400.0));
            h.drop_at(eframe::egui::pos2(600.0, 400.0));
            h.run_steps(1);
            if sample >= 1 {
                click.push(t.elapsed());
            }
            assert!(h.state_mut().doc().version > 0, "量測的文件必須實際開啟並編輯");
        }
        let report = |name: &str, mut v: Vec<Duration>| {
            v.sort();
            println!("{name}: median={:.2} ms p90={:.2} ms", v[v.len() / 2].as_secs_f64() * 1000.0, v[v.len() * 9 / 10].as_secs_f64() * 1000.0);
        };
        println!("--- {wrap:?} ---");
        report("open+render", open);
        report("keystroke", typing);
        report("End caret", caret);
        report("Shift+End select", select);
        report("5x PageDown", scroll);
        report("click", click);
    }

    samples(WrapMode::NoWrap, &path);
    samples(WrapMode::ByWindow, &path);
}

/// 自動換行下超長行的視覺列、游標與編輯（涵蓋換行快取與逐行繪製重構）
#[test]
fn test_wrapped_long_line_rows_caret_and_edit() {
    use crate::core::settings::WrapMode;
    use crate::core::editor::TextPosition;
    let mut h = harness();
    h.state_mut().settings.wrap_mode = WrapMode::ByWindow;
    h.event(Event::Paste(format!("{}\n短行", "x".repeat(2000))));
    h.run_steps(3);
    let rows = {
        let tab = h.state_mut().tab_mut();
        tab.view.row_count(&tab.doc.editor)
    };
    assert!(rows > 3, "2000 字的行自動換行後必須佔多個視覺列，實際 {rows}");
    assert_eq!(h.state().doc().editor.lines.len(), 2, "換行不會產生新的邏輯行");

    // 行首往下：應走到同一邏輯行的下一個視覺列，而非第二行
    h.state_mut().doc_mut().editor.set_caret(0, 0);
    h.run_steps(2);
    h.key_press(Key::ArrowDown);
    h.run_steps(2);
    let head = h.state().doc().editor.selection.head;
    assert_eq!(head.line, 0, "第一個視覺列的下方仍是同一行");
    assert!(head.col > 0);

    // 行尾（第 2000 字）往下：應落到短行行尾（水平移動重設記憶欄位）
    h.state_mut().doc_mut().editor.set_caret(0, 2000);
    h.run_steps(2);
    h.key_press(Key::ArrowLeft);
    h.key_press(Key::ArrowRight);
    h.run_steps(2);
    h.key_press(Key::ArrowDown);
    h.run_steps(2);
    assert_eq!(h.state().doc().editor.selection.head, TextPosition::new(1, 2), "行尾的下方是短行行尾");

    // 在超長行中間輸入：換行快取增量更新，內容與游標正確
    h.state_mut().doc_mut().editor.set_caret(0, 1000);
    h.run_steps(1);
    type_text(&mut h, "abc");
    assert_eq!(h.state().doc().editor.lines[0].chars().count(), 2003);
    assert_eq!(h.state().doc().editor.selection.head, TextPosition::new(0, 1003));
}

#[test]
fn test_typing_enter_backspace_and_undo() {
    let mut h = harness();
    type_text(&mut h, "abc");
    h.key_press(Key::Enter);
    h.run_steps(2);
    type_text(&mut h, "xy");
    h.key_press(Key::Backspace);
    h.run_steps(2);
    assert_eq!(text(&mut h), "abc\nx");
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run_steps(2);
    assert_ne!(text(&mut h), "abc\nx", "復原應改變內容");
}

#[test]
fn test_ime_preedit_then_commit() {
    let mut h = harness();
    h.event(Event::Ime(ImeEvent::Preedit { text: "ㄋㄧˇ".into(), active_range_chars: None }));
    h.run_steps(2);
    assert_eq!(text(&mut h), "", "預編輯文字不應寫入文件");
    h.event(Event::Ime(ImeEvent::Commit("你".into())));
    h.run_steps(2);
    assert_eq!(text(&mut h), "你");
}

#[cfg(target_os = "macos")]
#[test]
fn test_macos_navigation_delete_and_undo() {
    let mut h = harness();
    h.event(Event::Paste("  hello world\n第二行".into()));
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, Key::ArrowUp);
    h.run_steps(2);
    assert_eq!(h.state_mut().doc().editor.selection.head, crate::core::editor::TextPosition::new(0, 0));
    h.key_press_modifiers(Modifiers::COMMAND, Key::ArrowRight);
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::ALT, Key::Backspace);
    h.run_steps(2);
    assert_eq!(text(&mut h), "  hello \n第二行");
    h.key_press_modifiers(Modifiers::COMMAND, Key::Backspace);
    h.run_steps(2);
    assert_eq!(text(&mut h), "\n第二行");
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run_steps(2);
    assert_eq!(text(&mut h), "  hello \n第二行");
    h.key_press_modifiers(Modifiers::COMMAND, Key::ArrowLeft);
    h.run_steps(2);
    assert_eq!(h.state_mut().doc().editor.selection.head.col, 0, "⌘← 必須到實際行首");
}

#[cfg(target_os = "macos")]
#[test]
fn test_macos_close_and_quit_preserve_unsaved_document() {
    let mut h = harness();
    type_text(&mut h, "未儲存");
    h.key_press_modifiers(Modifiers::COMMAND, Key::W);
    h.run_steps(2);
    assert!(matches!(h.state_mut().confirm, Some(Confirm::Close { exit: false, .. })));
    h.state_mut().confirm = None;
    h.key_press_modifiers(Modifiers::COMMAND, Key::Q);
    h.run_steps(2);
    assert!(matches!(h.state_mut().confirm, Some(Confirm::Close { exit: true, .. })));
    assert_eq!(text(&mut h), "未儲存");
    assert!(!h.state_mut().allow_close);
}

#[test]
fn test_find_bar_selects_match() {
    let mut h = harness();
    type_text(&mut h, "one two one");
    h.key_press_modifiers(Modifiers::COMMAND, Key::F);
    h.run_steps(3);
    assert!(h.state_mut().find.visible);
    h.state_mut().find.query = "two".into();
    h.state_mut().pending.push(C::FindNext);
    h.run_steps(3);
    assert_eq!(h.state_mut().doc().editor.selected_text(), "two");
}

#[test]
fn test_toggle_theme_switches_visuals() {
    let mut h = harness();
    let before = App::is_dark(&h.ctx);
    h.state_mut().pending.push(C::ToggleTheme);
    h.run_steps(3);
    assert_ne!(App::is_dark(&h.ctx), before);
    h.state_mut().pending.push(C::ToggleTheme);
    h.run_steps(3);
    assert_eq!(App::is_dark(&h.ctx), before);
}

#[test]
fn test_paste_event_inserts_text() {
    let mut h = harness();
    h.event(Event::Paste("貼上\n第二行".into()));
    h.run_steps(2);
    assert_eq!(text(&mut h), "貼上\n第二行");
}

#[test]
fn test_close_modified_document_asks_first() {
    let mut h = harness();
    type_text(&mut h, "x");
    h.state_mut().pending.push(C::Close);
    h.run_steps(2);
    assert!(matches!(h.state_mut().confirm, Some(Confirm::Close { .. })));
    assert_eq!(h.state_mut().tabs.len(), 1);
}

#[test]
fn test_quit_keeps_tabs_for_session_and_cancel() {
    let mut h = harness();
    let first = h.state_mut().doc().id;
    h.state_mut().pending.push(C::New);
    h.run_steps(2);
    type_text(&mut h, "保留修改");
    h.state_mut().pending.push(C::Exit);
    h.run_steps(2);
    assert_eq!(h.state_mut().tabs.len(), 2);
    assert_eq!(h.state_mut().tabs[0].doc.id, first);
    h.get_by_label("取消").click();
    h.run_steps(2);
    assert!(!h.state_mut().allow_close);
    assert_eq!(h.state_mut().tabs.len(), 2);
    h.state_mut().pending.push(C::Exit);
    h.run_steps(2);
    h.get_by_label("不儲存").click();
    h.run_steps(2);
    assert!(h.state_mut().allow_close);
    assert_eq!(h.state_mut().tabs.len(), 2, "結束前仍須保留分頁供工作階段記錄");
}

#[test]
fn test_close_save_with_second_confirm_still_closes() {
    // 關閉時選「儲存」而存檔需要再確認（檔案已被外部修改）：確認覆寫後仍要完成關閉
    let p = std::env::temp_dir().join(format!("madedit_ui_close_{}.txt", std::process::id()));
    std::fs::write(&p, "old").unwrap();
    let mut h = Harness::builder().allow_missing_glyphs().with_size([1000.0, 700.0]).build_eframe(|cc| App::new(cc, vec![p.clone()], None));
    h.run_steps(3);
    type_text(&mut h, "x");
    std::fs::write(&p, "changed outside").unwrap();
    h.state_mut().pending.push(C::Close);
    h.run_steps(2);
    h.get_by_label("儲存").click();
    h.run_steps(2);
    assert!(matches!(h.state_mut().confirm, Some(Confirm::OverwriteExternal { .. })));
    h.get_by_label("覆寫").click();
    h.run_steps(2);
    assert!(h.state_mut().confirm.is_none());
    assert!(h.state_mut().doc().path.is_none(), "分頁應已關閉");
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "xold");
    let _ = std::fs::remove_file(&p);
}

#[test]
fn test_open_missing_file_asks_to_create_new_file() {
    // 開啟不存在的檔案時要像 Notepad 一樣詢問是否建立新檔；
    // 選擇建立後分頁綁定原路徑，存檔會真的把檔案寫出來
    let p = std::env::temp_dir().join(format!("madedit_ui_create_{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&p);
    let mut h = Harness::builder().allow_missing_glyphs().with_size([1000.0, 700.0]).build_eframe(|cc| App::new(cc, vec![p.clone()], None));
    h.state_mut().settings.ui_language = crate::core::settings::UiLanguage::ZhTw;
    h.run_steps(3);
    assert!(matches!(h.state_mut().confirm, Some(Confirm::CreateNew { .. })), "檔案不存在時應詢問是否建立");
    h.get_by_label("建立").click();
    h.run_steps(2);
    assert!(h.state_mut().confirm.is_none());
    assert_eq!(h.state_mut().doc().path.as_deref(), Some(p.as_path()), "建立後分頁應綁定原路徑");
    h.state_mut().pending.push(C::Save);
    h.run_steps(3);
    assert!(p.exists(), "存檔後檔案應真的被建立");
    let _ = std::fs::remove_file(&p);
}

#[test]
fn test_open_missing_file_cancel_creates_nothing() {
    // 取消建立時不得新增分頁，也不得在磁碟上留下檔案
    let p = std::env::temp_dir().join(format!("madedit_ui_create_cancel_{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&p);
    let mut h = Harness::builder().allow_missing_glyphs().with_size([1000.0, 700.0]).build_eframe(|cc| App::new(cc, vec![p.clone()], None));
    h.state_mut().settings.ui_language = crate::core::settings::UiLanguage::ZhTw;
    h.run_steps(3);
    let before = h.state_mut().tabs.len();
    h.get_by_label("取消").click();
    h.run_steps(2);
    assert!(h.state_mut().confirm.is_none());
    assert_eq!(h.state_mut().tabs.len(), before, "取消建立不得新增分頁");
    assert!(!p.exists());
}

#[test]
fn test_mode_switch_keeps_content() {
    let mut h = harness();
    type_text(&mut h, "hello");
    h.state_mut().pending.push(C::HexMode);
    h.run_steps(2);
    h.state_mut().pending.push(C::TextMode);
    h.run_steps(2);
    assert_eq!(text(&mut h), "hello");
}

#[test]
fn test_switch_tab_by_number_shortcut() {
    let mut h = harness();
    h.state_mut().pending.push(C::New);
    h.state_mut().pending.push(C::New);
    h.run_steps(2);
    assert_eq!(h.state_mut().tabs.len(), 3);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Num1);
    h.run_steps(2);
    assert_eq!(h.state_mut().active, 0);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Num9);
    h.run_steps(2);
    assert_eq!(h.state_mut().active, 2, "第 9 個快捷鍵切至最後一個分頁");
    h.key_press_modifiers(Modifiers::COMMAND, Key::Num5);
    h.run_steps(2);
    assert_eq!(h.state_mut().active, 2, "分頁不存在時不動作");
}

#[cfg(unix)]
#[test]
fn test_replace_in_files_skips_opened_file_via_symlink_alias() {
    // 以符號連結別名指定資料夾時，已開啟的檔案仍須只在記憶體中取代、不寫入磁碟
    let base = std::env::temp_dir().join(format!("madedit_ui_fif_{}", std::process::id()));
    let real = base.join("real");
    let alias = base.join("alias");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&real).unwrap();
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let opened = real.join("opened.txt");
    let closed = real.join("closed.txt");
    std::fs::write(&opened, "old").unwrap();
    std::fs::write(&closed, "old").unwrap();

    let mut h = Harness::builder().allow_missing_glyphs().with_size([1000.0, 700.0]).build_eframe(|cc| App::new(cc, vec![opened.clone()], None));
    h.run_steps(3);
    h.state_mut().settings.find_in_files.directory = alias.to_string_lossy().into_owned();
    h.state_mut().start_find_in_files("old".into(), Some("new".into()));
    for _ in 0..200 {
        h.run_steps(1);
        if !h.state_mut().fif.running {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!h.state_mut().fif.running);
    assert_eq!(h.state_mut().doc().text(), "new");
    assert_eq!(std::fs::read_to_string(&opened).unwrap(), "old", "已開啟的檔案不可被背景寫入");
    assert_eq!(std::fs::read_to_string(&closed).unwrap(), "new");
    let _ = std::fs::remove_dir_all(&base);
}

/// Markdown 預覽：並排顯示時，編輯區捲動後預覽會記錄同步的行號，且錨點隨內容量得
#[test]
fn test_markdown_preview_follows_editor_scroll() {
    let dir = std::env::temp_dir().join(format!("madedit-mdsync-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("long.md");
    let body: String = (0..150).map(|i| format!("## 標題 {i}\n\n段落 {i}\n\n")).collect();
    std::fs::write(&p, body).unwrap();
    let mut h = Harness::builder().allow_missing_glyphs().with_size([1000.0, 700.0]).build_eframe(|cc| App::new(cc, vec![p.clone()], None));
    h.run_steps(3);
    h.state_mut().pending.push(super::C::PreviewMarkdown);
    h.run_steps(2);
    // 改為並排，檢查預覽的實際位置與來源錨點。
    h.state_mut().tab_mut().md_preview.as_mut().unwrap().set_placement(super::mdpreview::Placement::Right);
    h.run_steps(5);
    {
        let pv = h.state_mut().tab_mut().md_preview.as_ref().expect("預覽未開啟");
        assert!(pv.anchors.len() > 100, "錨點數 {}", pv.anchors.len());
        // 錨點 y 應隨行號遞增
        assert!(pv.anchors.windows(2).all(|w| w[0].0 <= w[1].0 && w[0].1 <= w[1].1));
        assert_eq!(pv.synced_line, Some(0));
    }
    h.state_mut().tab_mut().view.top_row = 200;
    h.run_steps(5);
    assert_eq!(h.state_mut().tab_mut().md_preview.as_ref().unwrap().synced_line, Some(200));
    let pv = h.state_mut().tab_mut().md_preview.as_ref().unwrap();
    let expected = pv.anchors.iter().find(|a| a.0 == 200).unwrap().1;
    assert!((pv.scroll_position.0 - expected.min(pv.scroll_position.1)).abs() < 1.0, "須實際捲到來源區塊，不能只更新同步行號");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_markdown_preview_scrolls_within_wrapped_editor_line() {
    use super::mdpreview::Placement;
    use crate::core::settings::WrapMode;
    let mut h = harness();
    h.state_mut().doc_mut().syntax = "Markdown".into();
    h.state_mut().doc_mut().editor.set_text(&format!("{}\n\n結尾\n", "long text ".repeat(1000)));
    h.state_mut().doc_mut().version += 1;
    h.state_mut().settings.wrap_mode = WrapMode::ByWindow;
    h.state_mut().pending.push(C::PreviewMarkdown);
    h.run_steps(3);
    h.state_mut().tab_mut().md_preview.as_mut().unwrap().set_placement(Placement::Right);
    h.run_steps(5);
    h.state_mut().tab_mut().view.top_row = 10;
    h.run_steps(5);
    let tab = h.state_mut().tab_mut();
    let (line, fraction) = tab.view.top_line(&tab.doc.editor);
    assert_eq!(line, 0);
    assert!(fraction > 0.0);
    assert!(tab.md_preview.as_ref().unwrap().scroll_position.0 > 10.0, "同一邏輯行內的換行也須推進預覽");
}

/// 直接送入滾輪事件，驗證實際互動與平滑捲動的後續幀，而非直接修改預覽狀態。
fn scroll_preview(h: &mut Harness<'static, App>, delta: f32) {
    let rect = h.state_mut().tab_mut().md_preview.as_ref().unwrap().rect;
    h.hover_at(eframe::egui::pos2(rect.center().x, rect.bottom() - 80.0));
    h.run_steps(1);
    h.event(Event::MouseWheel {
        unit: eframe::egui::MouseWheelUnit::Point,
        delta: eframe::egui::vec2(0.0, delta),
        phase: eframe::egui::TouchPhase::Move,
        source: eframe::egui::MouseWheelSource::Wheel,
        modifiers: Modifiers::NONE,
    });
    h.run_steps(30);
}

#[test]
fn test_markdown_preview_scrolls_editor_without_feedback_or_selection_change() {
    use super::mdpreview::Placement;
    use crate::core::editor::{Selection, TextPosition};
    for placement in [Placement::Right, Placement::Bottom, Placement::Tab] {
        let mut h = harness();
        h.state_mut().doc_mut().syntax = "Markdown".into();
        h.state_mut().doc_mut().editor.set_text(&(0..150).map(|i| format!("## Heading {i}\n\nParagraph {i}\n\n")).collect::<String>());
        h.state_mut().doc_mut().version += 1;
        let selection = Selection::new(TextPosition::new(0, 1), TextPosition::new(2, 3));
        h.state_mut().doc_mut().editor.selection = selection;
        h.state_mut().pending.push(C::PreviewMarkdown);
        h.run_steps(3);
        h.state_mut().tab_mut().md_preview.as_mut().unwrap().set_placement(placement);
        h.run_steps(8);
        scroll_preview(&mut h, -500.0);
        let tab = h.state_mut().tab_mut();
        let first = tab.view.top_row;
        assert!(first > 10, "預覽捲動後須帶動編輯區：{placement:?}, {first}");
        assert_eq!(tab.doc.editor.selection, selection, "同步不能移動游標或選取");
        let offset = tab.md_preview.as_ref().unwrap().scroll_position.0;
        h.run_steps(15);
        let tab = h.state_mut().tab_mut();
        assert_eq!(tab.view.top_row, first, "閒置後來源位置不能繼續來回變動");
        assert!((tab.md_preview.as_ref().unwrap().scroll_position.0 - offset).abs() < 0.1, "預覽不能被整行取整誤差拉回");
        // 接著改由編輯區捲動，確認主導權能立即交回。
        tab.view.top_row = first + 40;
        h.run_steps(5);
        assert_eq!(h.state_mut().tab_mut().md_preview.as_ref().unwrap().synced_line, Some(first + 40));
        h.get_by_label("同步捲動").click();
        h.run_steps(3);
        scroll_preview(&mut h, -400.0);
        assert_eq!(h.state_mut().tab_mut().view.top_row, first + 40, "停用同步後兩側須可各自閱讀");
    }
}

#[test]
fn test_markdown_preview_reverse_scrolls_within_wrapped_line() {
    use super::mdpreview::Placement;
    use crate::core::settings::WrapMode;
    let mut h = harness();
    h.state_mut().doc_mut().syntax = "Markdown".into();
    h.state_mut().doc_mut().editor.set_text(&format!("{}\n\nend\n", "long text ".repeat(1000)));
    h.state_mut().doc_mut().version += 1;
    h.state_mut().settings.wrap_mode = WrapMode::ByWindow;
    h.state_mut().pending.push(C::PreviewMarkdown);
    h.run_steps(3);
    h.state_mut().tab_mut().md_preview.as_mut().unwrap().set_placement(Placement::Right);
    h.run_steps(8);
    scroll_preview(&mut h, -700.0);
    let tab = h.state_mut().tab_mut();
    let (line, fraction) = tab.view.top_line(&tab.doc.editor);
    assert_eq!(line, 0);
    assert!(fraction > 0.0 && tab.view.top_row > 0, "長段落內須帶動來源的換行列");
    let before = tab.md_preview.as_ref().unwrap().scroll_position.0;
    h.run_steps(15);
    assert!((h.state_mut().tab_mut().md_preview.as_ref().unwrap().scroll_position.0 - before).abs() < 0.1);
}

/// Markdown 預覽：切成左右／上下並排時各佔一半，拖曳後須保留新的尺寸。
#[test]
fn test_markdown_preview_docks_at_half_size() {
    use super::mdpreview::Placement;
    let dir = std::env::temp_dir().join(format!("madedit-mddock-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("dock.md");
    // 內含寬圖表，確認內容不會把面板撐大
    let table = format!("\n| {} |\n| {} |\n| {} |\n", vec!["欄位"; 12].join(" | "), vec!["---"; 12].join(" | "), vec!["LongUnbrokenCellValue"; 12].join(" | "));
    std::fs::write(&p, format!("# 標題\n\n```mermaid\nflowchart LR\n  A[很長很長的節點一] --> B[很長很長的節點二] --> C[很長很長的節點三] --> D[很長很長的節點四]\n```\n{table}")).unwrap();
    let mut h = Harness::builder().allow_missing_glyphs().with_size([1000.0, 700.0]).build_eframe(|cc| App::new(cc, vec![p.clone()], None));
    h.run_steps(3);
    h.state_mut().pending.push(super::C::PreviewMarkdown);
    h.run_steps(3);
    let mut half_y = 0.0;
    for placement in [Placement::Right, Placement::Bottom, Placement::Right] {
        h.get_by_label(if placement == Placement::Right { "左右並排" } else { "上下並排" }).click();
        h.run_steps(8);
        // 模擬原生預覽關閉後，主視窗才收到新的可用尺寸；仍須重新平分。
        h.set_size(eframe::egui::vec2(900.0, 650.0));
        h.run_steps(4);
        let resized = h.get_by_role_and_label(eframe::egui::accesskit::Role::Splitter, "Resize panel").rect();
        if placement == Placement::Right { assert!((resized.center().x - 450.0).abs() < 1.0); }
        h.set_size(eframe::egui::vec2(1000.0, 700.0));
        h.run_steps(4);
        let handle = h.get_by_role_and_label(eframe::egui::accesskit::Role::Splitter, "Resize panel").rect();
        let pv = h.state_mut().tab_mut().md_preview.as_ref().unwrap();
        let (w, hgt) = (pv.rect.width(), pv.rect.height());
        if placement == Placement::Right {
            assert!((w - 500.0).abs() < 20.0, "左右並排寬度 {w}");
            assert!((handle.center().x - 500.0).abs() < 1.0, "分隔線應在正中央");
            half_y = handle.center().y;
        } else {
            assert!(hgt > 200.0 && hgt < 400.0 && w > 900.0, "上下並排 {w}x{hgt}");
            assert!((handle.center().y - half_y).abs() < 1.0, "上下並排分隔線應在可用高度中央");
        }
        // 第二次拖曳幅度不同，留下非一半的尺寸，再驗證切換擺放方式時確實重設。
        for direction in [1.0, -1.5] {
            let before = h.state_mut().tab_mut().md_preview.as_ref().unwrap().rect;
            let handle = h.get_by_role_and_label(eframe::egui::accesskit::Role::Splitter, "Resize panel").rect();
            // 從分隔線中央抓取，才能抓到與編輯區捲軸重疊的回歸問題。
            let start = handle.center();
            let end = start + direction * if placement == Placement::Right { eframe::egui::vec2(100.0, 0.0) } else { eframe::egui::vec2(0.0, 70.0) };
            h.hover_at(start);
            h.drag_at(start);
            h.run_steps(2);
            if direction > 0.0 {
                h.hover_at(end);
                h.run_steps(3);
                h.drop_at(end);
            } else {
                // Harness 的 event() 會將各事件拆成不同幀；直接寫入 RawInput，
                // 才能重現 Computer Use 的「移動與放開同幀」。
                h.input_mut().events.extend([
                    Event::PointerMoved(end),
                    Event::PointerButton { pos: end, button: eframe::egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE },
                ]);
            }
            h.run_steps(8);
            // 文件更新與圖表重新排版後，也不能覆蓋使用者調整的大小。
            h.state_mut().doc_mut().version += 1;
            h.run_steps(8);
            let after = h.state_mut().tab_mut().md_preview.as_ref().unwrap().rect;
            let (before_size, after_size, delta) = if placement == Placement::Right {
                (before.width(), after.width(), end.x - start.x)
            } else { (before.height(), after.height(), end.y - start.y) };
            assert!((before_size - after_size).abs() > 40.0, "拖曳應改變面板大小：{placement:?}, {before_size} → {after_size}");
            assert!((before_size - delta - after_size).abs() < 2.0, "放開後不能彈回：{placement:?}, 預期 {}，實際 {after_size}", before_size - delta);
        }
        let manual = h.state_mut().tab_mut().md_preview.as_ref().unwrap().rect;
        h.set_size(eframe::egui::vec2(1100.0, 760.0));
        h.run_steps(5);
        let resized = h.state_mut().tab_mut().md_preview.as_ref().unwrap().rect;
        let difference = if placement == Placement::Right { manual.width() - resized.width() } else { manual.height() - resized.height() };
        assert!(difference.abs() < 2.0, "使用者調整後，主視窗改變大小也不能強制改回一半");
        h.set_size(eframe::egui::vec2(1000.0, 700.0));
        h.run_steps(5);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_backspace_wrapped_selection_does_not_panic() {
    // 自動換行時刪除多行選取：同一幀內換行快取須先更新再繪製，否則以舊的斷行位置索引新內容而 panic
    use crate::core::editor::{Selection, TextPosition};
    use crate::core::settings::WrapMode;
    let mut h = Harness::builder().allow_missing_glyphs().with_size([400.0, 700.0]).build_eframe(|cc| App::new(cc, Vec::new(), None));
    h.run_steps(3);
    h.state_mut().settings.wrap_mode = WrapMode::ByWindow;
    let long = "word ".repeat(60);
    h.state_mut().doc_mut().editor.set_text(&format!("{long}\n{long}\n\nend"));
    h.state_mut().doc_mut().version += 1;
    h.run_steps(2);
    h.state_mut().doc_mut().editor.selection = Selection::new(TextPosition::new(0, 0), TextPosition::new(2, 0));
    h.run_steps(2);
    h.key_press(Key::Backspace);
    h.run_steps(2);
    assert_eq!(text(&mut h), "\nend");
}

/// Markdown 預覽屬於各自的分頁：切到別的分頁就不顯示，切回來仍保留原本的擺放方式
#[test]
fn test_markdown_preview_follows_active_tab() {
    use super::mdpreview::Placement;
    let dir = std::env::temp_dir().join(format!("madedit-mdtab-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (a, b) = (dir.join("a.md"), dir.join("b.md"));
    std::fs::write(&a, "# A\n").unwrap();
    std::fs::write(&b, "# B\n").unwrap();
    let mut h = Harness::builder().allow_missing_glyphs().with_size([1000.0, 700.0]).build_eframe(|cc| App::new(cc, vec![a, b], None));
    h.run_steps(3);
    h.state_mut().activate(0);
    h.state_mut().pending.push(C::PreviewMarkdown);
    h.run_steps(3);
    h.state_mut().tab_mut().md_preview.as_mut().unwrap().set_placement(Placement::Right);
    h.run_steps(3);
    h.state_mut().pending.push(C::NextWindow);
    h.run_steps(3);
    assert_eq!(h.state_mut().active, 1);
    assert!(h.state_mut().tab_mut().md_preview.is_none(), "另一個分頁不應顯示預覽");
    h.state_mut().pending.push(C::NextWindow);
    h.run_steps(3);
    let pv = h.state_mut().tab_mut().md_preview.as_ref().expect("切回後應恢復預覽");
    assert_eq!(pv.placement, Placement::Right);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 預覽分頁共享來源、支援切換與關閉，且不能新增原生視窗或修改隱藏的來源。
#[test]
fn test_markdown_preview_tab_reuses_source_and_never_opens_a_viewport() {
    let mut h = harness();
    h.state_mut().doc_mut().syntax = "Markdown".into();
    type_text(&mut h, "# Preview heading");
    let source_id = h.state().doc().id;
    let source_name = format!("{} *", h.state().doc().display_name());
    let preview_name = format!("{} — Markdown 預覽", h.state().doc().display_name());
    h.state_mut().pending.push(C::PreviewMarkdown);
    h.run_steps(5);
    assert!(h.state().is_preview_tab());
    assert_eq!(h.state().tabs.len(), 1, "預覽不得複製來源文件");
    assert_eq!(h.state().tab_pages(), vec![(0, false), (0, true)]);
    assert_eq!(h.output().viewport_output.len(), 1, "預覽不能建立額外原生視窗");
    h.get_by_label(&preview_name);
    h.get_by_label("Preview heading");
    // 預覽分頁不能讓貼上、輸入或復原修改隱藏中的來源文件。
    h.event(Event::Paste("should not edit".into()));
    type_text(&mut h, "should not edit");
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run_steps(2);
    assert_eq!(text(&mut h), "# Preview heading");
    h.get_by_label(&source_name).click();
    h.run_steps(3);
    assert!(!h.state().is_preview_tab());
    type_text(&mut h, " updated");
    h.get_by_label(&preview_name).click();
    h.run_steps(5);
    h.get_by_label("Preview heading updated");
    h.state_mut().pending.push(C::PreviewMarkdown);
    h.run_steps(3);
    assert_eq!(h.state().tab_pages().len(), 2, "重複開啟只選取既有預覽");
    h.key_press_modifiers(Modifiers::COMMAND, Key::Num1);
    h.run_steps(2);
    assert!(!h.state().is_preview_tab());
    h.key_press_modifiers(if cfg!(target_os = "macos") { Modifiers::CTRL } else { Modifiers::COMMAND }, Key::Tab);
    h.run_steps(2);
    assert!(h.state().is_preview_tab(), "切換分頁須包含預覽");
    h.state_mut().pending.push(C::Close);
    h.run_steps(3);
    assert!(!h.state().is_preview_tab());
    assert!(h.state_mut().tab_mut().md_preview.is_none());
    assert_eq!(h.state().doc().id, source_id);
    assert!(h.state().doc().is_modified());
    assert_eq!(text(&mut h), "# Preview heading updated");
    assert!(h.state().confirm.is_none(), "關閉預覽不應關閉或詢問儲存來源");
}

/// 在分頁與並排間切換，並確認關閉來源時不留下預覽，也不影響其他文件。
#[test]
fn test_markdown_preview_tab_layout_and_document_lifetime() {
    let mut h = harness();
    h.state_mut().doc_mut().syntax = "Markdown".into();
    type_text(&mut h, "# First");
    let first_id = h.state().doc().id;
    h.state_mut().pending.push(C::PreviewMarkdown);
    h.run_steps(4);
    h.get_by_label("左右並排").click();
    h.run_steps(5);
    assert!(!h.state().is_preview_tab());
    assert_eq!(h.state().tab_pages().len(), 1);
    h.get_by_label("獨立分頁").click();
    h.run_steps(5);
    assert!(h.state().is_preview_tab());
    assert_eq!(h.state().tab_pages().len(), 2);
    h.state_mut().pending.push(C::New);
    h.run_steps(3);
    h.state_mut().doc_mut().syntax = "Markdown".into();
    type_text(&mut h, "# Second");
    let second_id = h.state().doc().id;
    h.state_mut().pending.push(C::PreviewMarkdown);
    h.run_steps(4);
    assert_eq!(h.state().tab_pages().len(), 4);
    assert_eq!(h.output().viewport_output.len(), 1);
    // 關閉別份來源時，仍在閱讀的預覽要跟著原文件，而不是跟著陣列索引。
    h.state_mut().close_now(first_id);
    h.run_steps(3);
    assert!(h.state().is_preview_tab());
    assert_eq!(h.state().doc().id, second_id);
    h.get_by_label("Second");
    h.state_mut().activate(0);
    h.state_mut().close_now(second_id);
    h.run_steps(3);
    assert!(!h.state().is_preview_tab());
    assert_eq!(h.state().tab_pages().len(), 1, "關閉來源必須移除它的預覽分頁");
    assert!(h.state().tabs.iter().all(|t| t.md_preview.is_none()));
}

/// Markdown 預覽只對 Markdown 檔開啟：其他檔案即使以快捷鍵觸發也不會開
#[test]
fn test_markdown_preview_only_for_markdown() {
    let dir = std::env::temp_dir().join(format!("madedit-mdonly-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (txt, md) = (dir.join("a.txt"), dir.join("b.md"));
    std::fs::write(&txt, "# A\n").unwrap();
    std::fs::write(&md, "# B\n").unwrap();
    let mut h = Harness::builder().allow_missing_glyphs().with_size([1000.0, 700.0]).build_eframe(|cc| App::new(cc, vec![txt, md], None));
    h.run_steps(3);
    h.state_mut().activate(0);
    assert!(!h.state_mut().doc().is_markdown());
    h.state_mut().pending.push(C::PreviewMarkdown);
    h.run_steps(3);
    assert!(h.state_mut().tab_mut().md_preview.is_none(), "純文字檔不應開啟預覽");
    h.state_mut().activate(1);
    assert!(h.state_mut().doc().is_markdown());
    h.state_mut().pending.push(C::PreviewMarkdown);
    h.run_steps(3);
    assert!(h.state_mut().tab_mut().md_preview.is_some(), "Markdown 檔應開啟預覽");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 另存檔名去除前後空白與結尾句點() {
    use std::path::PathBuf;
    let t = |s: &str| App::trim_file_name(PathBuf::from(s));
    assert_eq!(t("/a/ b.txt"), PathBuf::from("/a/b.txt"));
    assert_eq!(t("/a/b.txt "), PathBuf::from("/a/b.txt"));
    assert_eq!(t("/a/b. "), PathBuf::from("/a/b"));
    assert_eq!(t("/a/ b . . "), PathBuf::from("/a/b"));
    assert_eq!(t("/a/.hidden"), PathBuf::from("/a/.hidden"));
    assert_eq!(t("/a /b.txt"), PathBuf::from("/a /b.txt"));
}

/// 含全形字的超長行水平捲動：視窗起點的位元組格線會落在多位元組字元中間，不得 panic
#[test]
fn test_cjk_long_line_horizontal_scroll() {
    let mut h = harness();
    // 一長串全形字（字串）：視窗起點會落在多位元組字元中間
    let line = format!("let s = \"{}\";", "中文內容測試".repeat(4_000));
    {
        let doc = h.state_mut().doc_mut();
        doc.syntax = "Rust".into();
        doc.syntax_cache.clear();
    }
    h.event(Event::Paste(line.clone()));
    h.run_steps(3);
    let chars = h.state().doc().editor.line_char_count(0);
    assert_eq!(chars, line.chars().count());
    // 逐段把游標移到行內各處：畫面捲動會推進可見視窗，沿途每個視窗都要能切片與解析
    for step in 0..=32 {
        let col = chars * step / 32;
        h.state_mut().doc_mut().editor.set_caret(0, col);
        h.state_mut().tab_mut().view.ensure_visible = true;
        h.run_steps(1);
        assert_eq!(h.state().doc().editor.selection.head.col, col);
    }
    // 回到行首：視窗反向移動也不得 panic
    h.state_mut().doc_mut().editor.set_caret(0, 0);
    h.state_mut().tab_mut().view.ensure_visible = true;
    h.run_steps(2);
    assert_eq!(h.state().doc().editor.lines.len(), 1, "水平捲動不會產生新行");
}

