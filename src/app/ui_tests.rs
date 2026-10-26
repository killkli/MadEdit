//! 以 egui_kittest 模擬鍵盤與輸入法事件的整合測試（不開視窗、不需 GPU）

use eframe::egui::{Event, ImeEvent, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::commands::C;
use super::{App, Confirm};

fn harness() -> Harness<'static, App> {
    let mut h = Harness::builder().allow_missing_glyphs().with_size([1000.0, 700.0]).build_eframe(|cc| App::new(cc, Vec::new(), None));
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
    // 獨立視窗預設不跟隨捲動，切成並排顯示後才同步
    assert_eq!(h.state_mut().md_preview.as_ref().unwrap().synced_line, None);
    h.state_mut().md_preview.as_mut().unwrap().set_placement(super::mdpreview::Placement::Right);
    h.run_steps(5);
    {
        let pv = h.state_mut().md_preview.as_ref().expect("預覽未開啟");
        assert!(pv.anchors.len() > 100, "錨點數 {}", pv.anchors.len());
        // 錨點 y 應隨行號遞增
        assert!(pv.anchors.windows(2).all(|w| w[0].0 <= w[1].0 && w[0].1 <= w[1].1));
        assert_eq!(pv.synced_line, Some(0));
    }
    h.state_mut().tab_mut().view.top_row = 200;
    h.run_steps(5);
    assert_eq!(h.state_mut().md_preview.as_ref().unwrap().synced_line, Some(200));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Markdown 預覽：切成左右／上下並排時，面板固定為編輯區的一半，之後幾幀也不會彈回其他大小
#[test]
fn test_markdown_preview_docks_at_half_size() {
    use super::mdpreview::Placement;
    let dir = std::env::temp_dir().join(format!("madedit-mddock-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("dock.md");
    // 內含寬圖表，確認內容不會把面板撐大
    std::fs::write(&p, "# 標題\n\n```mermaid\nflowchart LR\n  A[很長很長的節點一] --> B[很長很長的節點二] --> C[很長很長的節點三] --> D[很長很長的節點四]\n```\n").unwrap();
    let mut h = Harness::builder().allow_missing_glyphs().with_size([1000.0, 700.0]).build_eframe(|cc| App::new(cc, vec![p.clone()], None));
    h.run_steps(3);
    h.state_mut().pending.push(super::C::PreviewMarkdown);
    h.run_steps(3);
    for placement in [Placement::Right, Placement::Bottom, Placement::Right] {
        h.state_mut().md_preview.as_mut().unwrap().set_placement(placement);
        h.run_steps(8);
        let pv = h.state_mut().md_preview.as_ref().unwrap();
        let (w, hgt) = (pv.rect.width(), pv.rect.height());
        if placement == Placement::Right {
            assert!((w - 500.0).abs() < 20.0, "左右並排寬度 {w}");
        } else {
            assert!(hgt > 200.0 && hgt < 400.0 && w > 900.0, "上下並排 {w}x{hgt}");
        }
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
