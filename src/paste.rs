use std::thread;
use std::time::Duration;

pub enum InsertResult {
    Pasted,
    Copied,
}

pub struct ClipboardRestore {
    previous: Option<String>,
}

impl ClipboardRestore {
    pub fn apply(self) {
        match self.previous {
            Some(text) => {
                let _ = write_clipboard(&text);
            }
            None => {
                let _ = clear_clipboard();
            }
        }
    }
}

pub fn copy_to_clipboard(text: &str) -> bool {
    write_clipboard(text).is_ok()
}

pub fn read_from_clipboard() -> Option<String> {
    read_clipboard()
}

pub fn insert_text(text: &str) -> (InsertResult, Option<ClipboardRestore>) {
    if secure_input() {
        let _ = write_clipboard(text);
        return (InsertResult::Copied, None);
    }
    let previous = read_clipboard();
    if write_clipboard(text).is_err() {
        return (InsertResult::Copied, None);
    }
    post_command_v();
    (InsertResult::Pasted, Some(ClipboardRestore { previous }))
}

fn read_clipboard() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    clipboard.get_text().ok()
}

fn write_clipboard(text: &str) -> Result<(), ()> {
    let mut clipboard = arboard::Clipboard::new().map_err(|_| ())?;
    clipboard.set_text(text).map_err(|_| ())
}

fn clear_clipboard() -> Result<(), ()> {
    let mut clipboard = arboard::Clipboard::new().map_err(|_| ())?;
    clipboard.clear().map_err(|_| ())
}

fn secure_input() -> bool {
    unsafe { IsSecureEventInputEnabled() }
}

fn post_command_v() {
    unsafe {
        let source = CGEventSourceCreate(1);
        let down = CGEventCreateKeyboardEvent(source, 9, true);
        let up = CGEventCreateKeyboardEvent(source, 9, false);
        CGEventSetFlags(down, 0x0010_0000);
        CGEventSetFlags(up, 0x0010_0000);
        CGEventPost(0, down);
        thread::sleep(Duration::from_millis(20));
        CGEventPost(0, up);
        if !down.is_null() {
            CFRelease(down);
        }
        if !up.is_null() {
            CFRelease(up);
        }
        if !source.is_null() {
            CFRelease(source);
        }
    }
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn IsSecureEventInputEnabled() -> bool;
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceCreate(state_id: u32) -> *mut std::ffi::c_void;
    fn CGEventCreateKeyboardEvent(
        source: *mut std::ffi::c_void,
        keycode: u16,
        key_down: bool,
    ) -> *mut std::ffi::c_void;
    fn CGEventSetFlags(event: *mut std::ffi::c_void, flags: u64);
    fn CGEventPost(tap: u32, event: *mut std::ffi::c_void);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: *mut std::ffi::c_void);
}
