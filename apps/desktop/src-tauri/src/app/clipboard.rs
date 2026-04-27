use objc::rc::autoreleasepool;
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use rayon_features::{ClipboardAccess, ClipboardHistoryService};
use std::ffi::{c_char, CStr};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const NSPASTEBOARD_TYPE_STRING: &str = "public.utf8-plain-text";
const NSUTF8_STRING_ENCODING: usize = 4;
const CLIPBOARD_POLL_INTERVAL: Duration = Duration::from_millis(750);

#[derive(Clone, Copy, Default)]
pub struct MacOsClipboardAccess;

impl MacOsClipboardAccess {
    pub fn change_count(&self) -> Result<i64, String> {
        autoreleasepool(|| unsafe {
            let pasteboard = general_pasteboard();
            let count: i64 = msg_send![pasteboard, changeCount];
            Ok(count)
        })
    }
}

impl ClipboardAccess for MacOsClipboardAccess {
    fn read_text(&self) -> Result<Option<String>, String> {
        autoreleasepool(|| unsafe {
            let pasteboard = general_pasteboard();
            let pasteboard_type = nsstring(NSPASTEBOARD_TYPE_STRING);
            let value: *mut Object = msg_send![pasteboard, stringForType: pasteboard_type];
            if value.is_null() {
                return Ok(None);
            }

            Ok(Some(nsstring_to_string(value)))
        })
    }

    fn write_text(&self, text: &str) -> Result<(), String> {
        autoreleasepool(|| unsafe {
            let pasteboard = general_pasteboard();
            let _: i64 = msg_send![pasteboard, clearContents];
            let value = nsstring(text);
            let pasteboard_type = nsstring(NSPASTEBOARD_TYPE_STRING);
            let success: bool = msg_send![pasteboard, setString: value forType: pasteboard_type];
            if success {
                Ok(())
            } else {
                Err("failed to write text to macOS clipboard".into())
            }
        })
    }
}

pub fn spawn_clipboard_watcher(
    clipboard: Arc<ClipboardHistoryService>,
    access: Arc<MacOsClipboardAccess>,
) {
    thread::spawn(move || {
        let mut last_change_count = access.change_count().ok();
        if let Err(error) = clipboard.sync_current_clipboard() {
            eprintln!("failed to capture initial clipboard contents: {error}");
        }

        loop {
            thread::sleep(CLIPBOARD_POLL_INTERVAL);

            let change_count = match access.change_count() {
                Ok(change_count) => change_count,
                Err(error) => {
                    eprintln!("failed to poll macOS clipboard: {error}");
                    continue;
                }
            };

            if last_change_count == Some(change_count) {
                continue;
            }

            last_change_count = Some(change_count);
            if let Err(error) = clipboard.sync_current_clipboard() {
                eprintln!("failed to store clipboard history entry: {error}");
            }
        }
    });
}

unsafe fn general_pasteboard() -> *mut Object {
    let cls = class!(NSPasteboard);
    msg_send![cls, generalPasteboard]
}

unsafe fn nsstring(value: &str) -> *mut Object {
    let cls = class!(NSString);
    let string: *mut Object = msg_send![cls, alloc];
    let string: *mut Object = msg_send![
        string,
        initWithBytes: value.as_ptr()
        length: value.len()
        encoding: NSUTF8_STRING_ENCODING
    ];
    msg_send![string, autorelease]
}

unsafe fn nsstring_to_string(value: *mut Object) -> String {
    let utf8: *const c_char = msg_send![value, UTF8String];
    if utf8.is_null() {
        String::new()
    } else {
        CStr::from_ptr(utf8).to_string_lossy().into_owned()
    }
}
