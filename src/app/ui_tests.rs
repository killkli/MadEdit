//! 以 egui_kittest 模擬鍵盤與輸入法事件的整合測試（不開視窗、不需 GPU）

use eframe::egui::{Event, ImeEvent, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::commands::C;
use super::{App, Confirm};

fn harness() -> Harness<'static, App> {
    let mut h = Harness::builder().with_size([1000.0, 700.0]).build_eframe(|cc| App::new(cc, Vec::new(), None));
    h.run_steps(3);
    h
}

fn type_text(h: &mut Harness<'static, App>, s: &str) {
    h.event(Event::Text(s.to_string()));
    h.run_steps(2);
}

fn text(h: &mut Harness<'static, App>) -> String {
    h.state_mut().doc().text()
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
    let mut h = Harness::builder().with_size([1000.0, 700.0]).build_eframe(|cc| App::new(cc, vec![p.clone()], None));
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

    let mut h = Harness::builder().with_size([1000.0, 700.0]).build_eframe(|cc| App::new(cc, vec![opened.clone()], None));
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
