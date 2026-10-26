# AccessKit macOS KVO 修正回補

本目錄保留 `accesskit_macos 0.27.1` 的來源與 MIT／Apache-2.0 授權，供 MadEdit 透過根目錄的 `[patch.crates-io]` 使用。保留既有 AccessKit 無障礙支援；不修改本機 Cargo registry。

## 原因與上游來源

Touch Bar 的 AppKit 處理器會以 KVO 觀察第一回應者的 `nextResponder`／`touchBar`。舊版 `SubclassingAdapter::drop` 無條件把視圖類別還原，覆蓋 KVO 安裝的動態子類別，導致之後移除觀察者時拋出 `NSRangeException`，進而終止程式。

- 問題與可重現程式：[AccessKit #823](https://github.com/AccessKit/accesskit/issues/823)。
- 採用修正：[AccessKit PR #835](https://github.com/AccessKit/accesskit/pull/835)，2026-10-23 合併。
- 固定合併 commit：[`451885c1850ec1800cea1d0660bf8ba62e0f410d`](https://github.com/AccessKit/accesskit/commit/451885c1850ec1800cea1d0660bf8ba62e0f410d)。
- 發行版基底：crates.io `accesskit_macos 0.27.1`（eframe 0.37 所需），來源 commit `ce8164ba92995cfa86005b6259115e08c8244253`。原回補於 0.26.3，因 0.26.3 與 0.27.1 的 `src/subclass.rs` 完全相同，升級 egui 0.37 時直接移植。
- 2026-10-25 查核時，crates.io 最新版 `0.27.1` 仍早於此修正，直接採用該版無法修復此問題。

## 回補範圍

`src/subclass.rs` 完整取自上述固定上游 commit；其餘 `src/` 保持 `0.27.1` 原樣。此修正於 Drop 時釋放處理器與關聯物件，但只有在視圖仍使用 AccessKit 自己的子類別時才還原類別；KVO 子類別保留至 AppKit 正常移除觀察者。關聯物件釋放後的無障礙回呼會安全轉交原類別，並支援同一視圖重新建立 adapter，避免以保留整個 adapter 的方式隱藏記憶體洩漏。

`Cargo.toml` 保留 `0.27.1` 的相依版本，另登錄主執行緒測試。`tests/integration/{main,subclassing_adapter,util}.rs` 取自相同上游 commit；僅 `harness.rs` 改為直接在主執行緒依序執行，省去額外測試框架相依。第三方來源與授權註記保持原文。

## 驗證與移除條件

在 macOS 專案根目錄執行：

```bash
cargo test -p accesskit_macos --test integration
```

八項測試涵蓋無障礙初始查詢、處理器釋放、一般類別還原、KVO 觀察者在 Drop 後正常移除、Drop 後回呼與 adapter 重建。AppKit 必須在主執行緒執行，因此不能換成一般 Rust 測試 harness。

當 eframe／accesskit_winit 的相依版本可採用包含上述 commit 的正式 `accesskit_macos` 發行版後，先移除根目錄 patch 並更新 `Cargo.lock`，通過這組 KVO 測試與真實 Touch Bar Mac 的視窗關閉、開存檔及退出操作後，即可刪除本目錄。
