use std::path::PathBuf;

/// 建置腳本：於 Windows 平台將應用程式圖示（assets/madedit-icon.ico）嵌入 .exe 資源，
/// 讓檔案總管與工作列顯示專屬圖示；其他平台為空操作（no-op）。
fn main() {
    embed_windows_icon();
}

/// 產生 Windows 資源檔（.rc）並交由 embed-resource 編譯嵌入執行檔。
///
/// 僅在目標平台為 Windows 時才執行；圖示嵌入失敗只發出警告而不中斷建置，
/// 以免缺少 RC.EXE / windres 的環境無法編譯專案。
fn embed_windows_icon() {
    // 非 Windows 平台直接略過（embed-resource 本身也會回報 NotWindows）
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let icon_path = manifest_dir.join("assets").join("madedit-icon.ico");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let rc_path = out_dir.join("madedit.rc");

    // 資源檔需要的是絕對路徑，才能不受建置工作目錄影響
    println!("cargo:rerun-if-changed={}", icon_path.display());
    std::fs::write(&rc_path, format!("1 ICON \"{}\"\n", icon_path.display())).unwrap();

    // .manifest_optional()：圖示屬於裝飾性資源，缺失時不視為建置失敗
    if let Err(err) = embed_resource::compile(rc_path.to_str().unwrap(), embed_resource::NONE)
        .manifest_optional()
    {
        println!(
            "cargo:warning=無法嵌入 Windows 圖示資源（視窗圖示仍會正常顯示）: {:?}",
            err
        );
    }
}
