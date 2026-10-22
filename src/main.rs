// Windows 發行版不顯示主控台視窗
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod core;

use std::path::PathBuf;

use eframe::egui;

fn main() -> eframe::Result {
    // 命令列：MadEdit [檔案...] [--screenshot <輸出.png>]（截圖供自動化測試使用）
    let mut files = Vec::new();
    let mut screenshot = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--screenshot" {
            screenshot = args.next().map(PathBuf::from);
        } else {
            files.push(PathBuf::from(a));
        }
    }

    let size = core::settings::Settings::load().window_size.unwrap_or((1200.0, 780.0));
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("MadEdit")
        .with_inner_size([size.0.max(400.0), size.1.max(300.0)])
        .with_min_inner_size([400.0, 300.0])
        .with_drag_and_drop(true);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/madedit-icon.png")) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("MadEdit", options, Box::new(|cc| Ok(Box::new(app::App::new(cc, files, screenshot)))))
}

#[cfg(test)]
mod tests {
    /// 內嵌的視窗圖示（原版調色盤格式 PNG）必須能被 eframe 解碼
    #[test]
    fn test_window_icon_decodes() {
        let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/madedit-icon.png")).unwrap();
        assert_eq!((icon.width, icon.height), (48, 48));
    }
}
