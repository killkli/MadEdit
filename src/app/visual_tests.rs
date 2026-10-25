//! 離屏繪製截圖（供無法開啟實體視窗的環境檢查畫面）
//!
//! 預設略過，執行方式：
//! `MADEDIT_SHOT_OUT=out.png MADEDIT_SHOT_FILE=檔案 MADEDIT_SHOT_CMDS=menuDarkTheme,menuHexMode cargo test shot -- --ignored`

use super::commands::C;
use super::App;

#[test]
#[ignore]
fn shot() {
    let out = std::env::var("MADEDIT_SHOT_OUT").unwrap_or_else(|_| "shot.png".into());
    let files: Vec<std::path::PathBuf> = std::env::var("MADEDIT_SHOT_FILE").ok().into_iter().map(Into::into).collect();
    let cmds = std::env::var("MADEDIT_SHOT_CMDS").unwrap_or_default();
    let mut harness = egui_kittest::Harness::builder()
        .allow_missing_glyphs()
        .with_size([1100.0, 720.0])
        .wgpu()
        .build_eframe(|cc| App::new(cc, files, None));
    harness.run_steps(3);
    for id in cmds.split(',').filter(|s| !s.is_empty()) {
        let c = C::from_id(id).unwrap_or_else(|| panic!("未知命令 {id}"));
        harness.state_mut().pending.push(c);
        harness.run_steps(3);
    }
    if let Ok(text) = std::env::var("MADEDIT_SHOT_TYPE") {
        harness.event(eframe::egui::Event::Text(text));
        harness.run_steps(3);
    }
    harness.render().expect("繪製失敗").save(&out).expect("無法儲存截圖");
}
