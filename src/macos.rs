//! Finder／Dock 的開啟文件與結束程式原生事件橋接。
//!
//! winit 負責 NSApplication 的 delegate；這裡註冊開檔／結束 Apple Event 並改接
//! 原生 Quit 選單，保留視窗與輸入法機制。所有事件交給 App 處理，結束前才有機會
//! 詢問未儲存文件；啟動初期尚無 egui Context 時先排隊。

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Mutex;

use eframe::egui;
use objc2::rc::Retained;
use objc2::{MainThreadOnly, define_class, msg_send, sel};
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSApplication, NSApplicationDidFinishLaunchingNotification, NSApplicationWillFinishLaunchingNotification, NSMenu};
use objc2_foundation::{MainThreadMarker, NSAppleEventDescriptor, NSAppleEventManager, NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol};

// Apple Event 四字元代碼：核心事件、開啟文件、直接參數。
const CORE_EVENT: u32 = u32::from_be_bytes(*b"aevt");
const OPEN_DOCUMENTS: u32 = u32::from_be_bytes(*b"odoc");
const QUIT_APPLICATION: u32 = u32::from_be_bytes(*b"quit");
const DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");

struct OpenState {
    paths: Vec<PathBuf>,
    context: Option<egui::Context>,
    quit_requested: bool,
}

static OPEN_STATE: Mutex<OpenState> = Mutex::new(OpenState { paths: Vec::new(), context: None, quit_requested: false });

thread_local! {
    // NSAppleEventManager 不保證持有 handler；明確保留至主執行緒結束。
    static HANDLER: RefCell<Option<Retained<OpenDocumentsHandler>>> = const { RefCell::new(None) };
}

define_class!(
    // 安全性：NSObject 無額外繼承要求；所有 Objective-C 呼叫限定於主執行緒。
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    struct OpenDocumentsHandler;

    unsafe impl NSObjectProtocol for OpenDocumentsHandler {}

    impl OpenDocumentsHandler {
        // 原生選單的 ⌘Q 不經過 egui 鍵盤事件，必須先交給 App 的未存檔確認流程。
        #[unsafe(method(requestQuit:))]
        fn request_quit_action(&self, _sender: Option<&AnyObject>) {
            request_quit();
        }

        // Dock「結束」送出 Apple Event，與選單走相同的確認流程。
        #[unsafe(method(handleQuit:withReplyEvent:))]
        fn handle_quit(&self, _event: &NSAppleEventDescriptor, _reply: &NSAppleEventDescriptor) {
            request_quit();
        }

        // AppKit 在 finishLaunching 期間會安裝自己的核心 Apple Event 處理器。
        // will 通知在冷啟動的文件事件之前送達，did 通知則保證後續熱開檔仍用本處理器。
        #[unsafe(method(applicationLaunching:))]
        fn application_launching(&self, _notification: &NSNotification) {
            self.register();
        }

        // 安全性：簽名符合 NSAppleEventManager 的 handler selector（事件與回覆）。
        #[unsafe(method(handleOpenDocuments:withReplyEvent:))]
        fn handle_open_documents(&self, event: &NSAppleEventDescriptor, _reply: &NSAppleEventDescriptor) {
            let Some(list) = event.paramDescriptorForKeyword(DIRECT_OBJECT) else { return };
            let paths = paths_from_list(&list);
            if paths.is_empty() {
                return;
            }
            // 先放入佇列再喚醒繪製；不在 Cocoa 回呼中直接借用 App 的可變狀態。
            if let Ok(mut state) = OPEN_STATE.lock() {
                state.paths.extend(paths);
                if let Some(ctx) = &state.context {
                    ctx.request_repaint();
                }
            }
        }
    }
);

impl OpenDocumentsHandler {
    fn register(&self) {
        // 安全性：selector 的兩個參數皆為 NSAppleEventDescriptor，與上方實作一致。
        unsafe {
            NSAppleEventManager::sharedAppleEventManager().setEventHandler_andSelector_forEventClass_andEventID(
                self,
                sel!(handleOpenDocuments:withReplyEvent:),
                CORE_EVENT,
                OPEN_DOCUMENTS,
            );
            NSAppleEventManager::sharedAppleEventManager().setEventHandler_andSelector_forEventClass_andEventID(
                self,
                sel!(handleQuit:withReplyEvent:),
                CORE_EVENT,
                QUIT_APPLICATION,
            );
        }
        // winit 建立的原生 Quit 選單直接執行 terminate:，會繞過自繪介面的儲存確認。
        // App 建立時也會呼叫 register，確保 winit 完成建立選單後才改寫該項目。
        let app = NSApplication::sharedApplication(self.mtm());
        if let Some(menu) = app.mainMenu() {
            self.bridge_quit_menu(&menu);
        }
    }

    fn bridge_quit_menu(&self, menu: &NSMenu) {
        for item in menu.itemArray() {
            if item.action() == Some(sel!(terminate:)) {
                // 安全性：target 由 HANDLER 保留，requestQuit: 接受可為 nil 的 sender。
                unsafe {
                    item.setTarget(Some(self));
                    item.setAction(Some(sel!(requestQuit:)));
                }
            }
            if let Some(submenu) = item.submenu() {
                self.bridge_quit_menu(&submenu);
            }
        }
    }
}

/// 在建立 eframe 事件迴圈前呼叫，以保留 Finder 冷啟動時送來的文件。
pub fn install() {
    let mtm = MainThreadMarker::new().expect("macOS 文件事件必須在主執行緒安裝");
    // AppKit 預設事件處理器先完成初始化，再覆寫開啟文件這一種事件。
    let _app = NSApplication::sharedApplication(mtm);
    HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        // 安全性：正確呼叫 NSObject 初始化，且 handler 的 selector 簽名如上。
        let handler: Retained<OpenDocumentsHandler> = unsafe { msg_send![mtm.alloc::<OpenDocumentsHandler>(), init] };
        handler.register();
        // 安全性：通知 selector 接受一個 NSNotification；觀察者由 HANDLER 保留。
        // 必須在 NSApplication::run 前訂閱，才能攔到冷啟動送來的第一批文件。
        unsafe {
            let center = NSNotificationCenter::defaultCenter();
            for name in [NSApplicationWillFinishLaunchingNotification, NSApplicationDidFinishLaunchingNotification] {
                center.addObserver_selector_name_object(&handler, sel!(applicationLaunching:), Some(name), Some(&_app));
            }
        }
        *slot.borrow_mut() = Some(handler);
    });
}

/// App 建立後提供喚醒機制；啟動前已排隊的文件會在第一幀一起開啟。
pub fn attach_context(ctx: &egui::Context) {
    HANDLER.with(|slot| {
        if let Some(handler) = slot.borrow().as_ref() {
            handler.register();
        }
    });
    if let Ok(mut state) = OPEN_STATE.lock() {
        state.context = Some(ctx.clone());
    }
}

/// 由 App 的主迴圈取走待開文件，沿用一般開檔的編碼判斷與錯誤提示。
pub fn take_open_files() -> Vec<PathBuf> {
    OPEN_STATE.lock().map(|mut state| std::mem::take(&mut state.paths)).unwrap_or_default()
}

fn request_quit() {
    if let Ok(mut state) = OPEN_STATE.lock() {
        state.quit_requested = true;
        if let Some(ctx) = &state.context {
            ctx.request_repaint();
        }
    }
}

/// 由 App 將原生結束請求派送為 C::Exit，完成儲存／取消確認後再關閉視窗。
pub fn take_quit_requested() -> bool {
    OPEN_STATE.lock().map(|mut state| std::mem::take(&mut state.quit_requested)).unwrap_or(false)
}

fn paths_from_list(list: &NSAppleEventDescriptor) -> Vec<PathBuf> {
    // Apple Event 清單索引從 1 開始；fileURLValue 同時接受 Finder 的 alias descriptor，
    // 不自行拆解 file:// 字串，讓空白、百分比與中文檔名保持原貌。
    (1..=list.numberOfItems())
        .filter_map(|i| list.descriptorAtIndex(i)?.fileURLValue()?.path().map(|p| PathBuf::from(p.to_string())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2_foundation::{NSString, NSURL};

    #[test]
    fn open_document_list_preserves_unicode_and_multiple_files() {
        let paths = ["/tmp/中文 檔案%25.txt", "/tmp/second.bin"];
        let list = NSAppleEventDescriptor::listDescriptor();
        for (i, path) in paths.iter().enumerate() {
            let url = NSURL::fileURLWithPath(&NSString::from_str(path));
            list.insertDescriptor_atIndex(&NSAppleEventDescriptor::descriptorWithFileURL(&url), (i + 1) as isize);
        }
        assert_eq!(paths_from_list(&list), paths.map(PathBuf::from));
    }

    #[test]
    fn open_document_list_ignores_non_file_descriptors() {
        let list = NSAppleEventDescriptor::listDescriptor();
        list.insertDescriptor_atIndex(&NSAppleEventDescriptor::nullDescriptor(), 1);
        assert!(paths_from_list(&list).is_empty());
    }
}
