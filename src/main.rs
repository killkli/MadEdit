// Windows 發行版不顯示主控台視窗
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod core;
#[cfg(target_os = "macos")]
mod macos;

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;

fn main() -> eframe::Result {
    #[cfg(target_os = "macos")]
    macos::install();
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
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    // egui 預設向顯示卡要求固定的資源上限（OpenGL 後端用 WebGL2 標準值），
    // 舊版 Mesa 軟體繪製（如 Ubuntu 20.04 的 llvmpipe）達不到時會直接啟動失敗。
    // 這裡把要求值壓到顯示卡實際支援的範圍內；編輯器只畫 2D 介面，用不到那麼多。
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup {
        let base = setup.device_descriptor.clone();
        setup.device_descriptor = Arc::new(move |adapter| {
            let mut desc = base(adapter);
            desc.required_limits = desc.required_limits.or_worse_values_from(&adapter.limits());
            desc
        });
    }
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
