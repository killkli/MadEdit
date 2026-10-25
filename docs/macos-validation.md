# macOS 移植驗證

驗證日期：2026-10-25。環境：Apple Silicon MacBook Pro（含 Touch Bar）、macOS 26.6.2、Rust 1.93.0。
正式產物為 `dist/MadEdit.app`，內含原生 arm64 執行檔、圖示與拼字字典；Mach-O 最低系統版本為 11.0。
最低版本是連結器資訊，不代表已在 macOS 11 實機測試。

## 自動化檢查

| 檢查 | 結果 |
| --- | --- |
| `cargo check --locked` | 通過，零警告 |
| `cargo test --locked` | 107 通過、1 項離屏截圖預設略過 |
| `cargo test -p accesskit_macos --locked --test integration` | 8 項通過，涵蓋 KVO 清理與無障礙處理器釋放 |
| `MADEDIT_SHOT_OUT=dist/validation/text.png MADEDIT_SHOT_FILE=README.md cargo test shot --locked -- --ignored` | 離屏繪製通過，已檢視中文與語法突顯 |
| `./scripts/test_macos_bundle.sh` | universal 測試執行檔、各架構最低系統版本、圖示／字典、簽章、失敗保護通過 |
| `./scripts/make_appbundle.sh` | 正式版建置、plist 與 ad-hoc 簽章驗證通過 |
| `python3 scripts/test_macos_runtime.py` | 文字、Hex、再次文字共三次原生啟動、截圖與退出皆回傳 0 |
| `DEST="$PWD/dist/validation/installed/MadEdit.app" ./scripts/install_macos.sh` | 安裝至隔離位置成功，副本簽章驗證通過 |

`otool -L` 確認正式執行檔只連結 macOS 系統 framework／動態函式庫，不依賴本機 Homebrew 路徑。
截圖與執行記錄保留於 `dist/validation/`，不納入版本控制。

## 原生互動

透過原生 App 的無障礙介面與鍵盤操作驗證：

- Launch Services 冷啟動與執行中 `open -a` 開檔；中文、空白及 `%25` 檔名正確，熱開檔新增分頁。
- ⌘O 開啟原生檔案面板，取消後回到編輯區；⌘⇧S 開啟原生另存面板並成功另存中文檔名。
- 中文多行貼上與 ⌘S 存檔；直接讀取輸出檔案確認內容一致。
- Option+3 切換 Hex 模式並正確繪製；文字與 Hex 原生截圖另由執行腳本保存。
- 未儲存時 ⌘Q／⌘W 顯示確認，取消保留修改；復原後 ⌘Q 正常退出，程序回傳 0。
- 使用 `MADEDIT_CONFIG_DIR` 隔離設定；退出後工作階段仍含原有文件與游標，測試修改未寫入原檔。

模擬輸入法預編輯／提交、列塊剪貼簿、多編碼往返、大檔案按需讀取、搜尋取代與存檔保護另由單元／互動測試覆蓋。

## 相依套件修正與範圍

原生退出曾重現 AccessKit 覆蓋 AppKit KVO 子類別而造成的 `NSRangeException`。
採用 [AccessKit #835](https://github.com/AccessKit/accesskit/pull/835) 的上游修正後，保留無障礙支援並通過同一台 Touch Bar Mac 的原生退出重測。
回補版本、授權與未來移除條件見 [vendor/README.md](../vendor/README.md)。

Intel 硬體及其他 macOS 版本尚未實機驗證。打包腳本支援本機 Intel 建置或指定 universal binary，測試中的 universal fixture 僅驗證打包邏輯。
目前使用本機 ad-hoc 簽章，未提供 Developer ID／Apple 公證發行。
