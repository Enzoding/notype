use std::sync::atomic::{AtomicBool, AtomicI64, AtomicPtr, AtomicU64, AtomicU8, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::store::TriggerMode;

const KEY_DOWN: u32 = 10;
const FLAGS_CHANGED: u32 = 12;
const KEYCODE_FIELD: u32 = 9;
const KEY_ESCAPE: i64 = 53;

pub const KEY_RIGHT_OPTION: i64 = 61;
pub const KEY_LEFT_OPTION: i64 = 58;
pub const KEY_RIGHT_COMMAND: i64 = 54;
pub const KEY_FN: i64 = 63;

const ALT: u64 = 0x0008_0000;
const SHIFT: u64 = 0x0002_0000;
const CONTROL: u64 = 0x0004_0000;
const COMMAND: u64 = 0x0010_0000;
const SECONDARY_FN: u64 = 0x0080_0000;

const DISABLED_TIMEOUT: u32 = 0xFFFF_FFFE;
const DISABLED_USER: u32 = 0xFFFF_FFFF;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyEvent {
    Toggle,
    HoldStart,
    HoldStop,
    Cancel,
}

pub const HOTKEY_OPTIONS: &[(i64, &str, &str)] = &[
    (KEY_RIGHT_OPTION, "右 Option", "右 ⌥"),
    (KEY_LEFT_OPTION, "左 Option", "左 ⌥"),
    (KEY_RIGHT_COMMAND, "右 Command", "右 ⌘"),
    (KEY_FN, "Fn / 地球键", "Fn"),
];

pub fn hotkey_label(code: i64) -> &'static str {
    HOTKEY_OPTIONS
        .iter()
        .find(|(c, _, _)| *c == code)
        .map(|(_, label, _)| *label)
        .unwrap_or("右 Option")
}

pub fn hotkey_badge(code: i64) -> &'static str {
    HOTKEY_OPTIONS
        .iter()
        .find(|(c, _, _)| *c == code)
        .map(|(_, _, badge)| *badge)
        .unwrap_or("右 ⌥")
}

static TAP: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
static TARGET_KEYCODE: AtomicI64 = AtomicI64::new(KEY_RIGHT_OPTION);
static TRIGGER_MODE: AtomicU8 = AtomicU8::new(0); // 0 = Toggle, 1 = Hold
static WAS_DOWN: AtomicBool = AtomicBool::new(false);
static LAST_MS: AtomicU64 = AtomicU64::new(0);
static CANCEL_ARMED: AtomicBool = AtomicBool::new(false);
static LAST_CANCEL_MS: AtomicU64 = AtomicU64::new(0);

/// While dictation or transcription is on screen, Esc is taken by this app.
pub fn set_cancel_armed(armed: bool) {
    CANCEL_ARMED.store(armed, Ordering::Relaxed);
}
static TX: OnceLock<Mutex<Option<mpsc::SyncSender<HotkeyEvent>>>> = OnceLock::new();

pub fn set_hotkey_code(code: i64) {
    TARGET_KEYCODE.store(code, Ordering::Relaxed);
    WAS_DOWN.store(false, Ordering::Relaxed);
}

pub fn set_trigger_mode(mode: TriggerMode) {
    let val = match mode {
        TriggerMode::Toggle => 0,
        TriggerMode::Hold => 1,
    };
    TRIGGER_MODE.store(val, Ordering::Relaxed);
    WAS_DOWN.store(false, Ordering::Relaxed);
}

pub fn is_accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

pub fn request_accessibility_permission() -> bool {
    unsafe {
        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue];
        let dict = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            std::ptr::addr_of!(kCFTypeDictionaryKeyCallBacks),
            std::ptr::addr_of!(kCFTypeDictionaryValueCallBacks),
        );
        if dict.is_null() {
            return AXIsProcessTrusted();
        }
        let trusted = AXIsProcessTrustedWithOptions(dict);
        CFRelease(dict as *mut std::ffi::c_void);
        trusted
    }
}

pub fn listen(initial_code: i64, initial_mode: TriggerMode) -> mpsc::Receiver<HotkeyEvent> {
    set_hotkey_code(initial_code);
    set_trigger_mode(initial_mode);
    let (tx, rx) = mpsc::sync_channel(16);
    TX.get_or_init(|| Mutex::new(None))
        .lock()
        .expect("hotkey sender")
        .replace(tx);
    std::thread::Builder::new()
        .name("notype-hotkey".into())
        .spawn(|| unsafe { run_loop() })
        .expect("hotkey thread");
    rx
}

unsafe fn run_loop() {
    let mask = (1u64 << KEY_DOWN) | (1u64 << FLAGS_CHANGED);
    // Explicitly ask macOS to prompt the user if not yet granted
    let _ = request_accessibility_permission();

    let mut warned = false;
    loop {
        let tap = unsafe { CGEventTapCreate(1, 0, 0, mask, tap_callback, std::ptr::null_mut()) };
        if !tap.is_null() {
            TAP.store(tap, Ordering::Relaxed);

            warned = false;
            unsafe {
                let source = CFMachPortCreateRunLoopSource(std::ptr::null_mut(), tap, 0);
                let run_loop = CFRunLoopGetCurrent();
                CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
                CGEventTapEnable(tap, true);
                CFRunLoopRun();
            }

            TAP.store(std::ptr::null_mut(), Ordering::Relaxed);
        } else {
            if !warned {
                eprintln!("热键监听等待辅助功能权限。请在系统设置的辅助功能里允许 Notype。");
                warned = true;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(800));
    }
}

fn flag_for_keycode(code: i64) -> (u64, u64) {
    match code {
        KEY_RIGHT_COMMAND => (COMMAND, ALT | SHIFT | CONTROL),
        KEY_FN => (SECONDARY_FN, ALT | SHIFT | CONTROL | COMMAND),
        _ => (ALT, SHIFT | CONTROL | COMMAND),
    }
}

extern "C" fn tap_callback(
    _proxy: *mut std::ffi::c_void,
    event_type: u32,
    event: *mut std::ffi::c_void,
    _user: *mut std::ffi::c_void,
) -> *mut std::ffi::c_void {
    unsafe {
        if event_type == DISABLED_TIMEOUT || event_type == DISABLED_USER {
            let tap = TAP.load(Ordering::Relaxed);
            if !tap.is_null() {
                CGEventTapEnable(tap, true);
            }
            return event;
        }
        if event_type == KEY_DOWN {
            let code = CGEventGetIntegerValueField(event, KEYCODE_FIELD);
            if code == KEY_ESCAPE && CANCEL_ARMED.load(Ordering::Relaxed) {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let last = LAST_CANCEL_MS.load(Ordering::Relaxed);
                if now.saturating_sub(last) > 250 {
                    LAST_CANCEL_MS.store(now, Ordering::Relaxed);
                    emit(HotkeyEvent::Cancel);
                }
                return std::ptr::null_mut();
            }
            return event;
        }
        if event_type != FLAGS_CHANGED {
            return event;
        }
        let code = CGEventGetIntegerValueField(event, KEYCODE_FIELD);
        let target_code = TARGET_KEYCODE.load(Ordering::Relaxed);
        if code != target_code {
            return event;
        }
        let flags = CGEventGetFlags(event);
        let (target_flag, forbidden_flags) = flag_for_keycode(target_code);
        let down = flags & target_flag != 0;
        let other = flags & forbidden_flags != 0;
        let was_down = WAS_DOWN.swap(down, Ordering::Relaxed);
        let is_hold = TRIGGER_MODE.load(Ordering::Relaxed) == 1;

        if is_hold {
            if down && !was_down && !other {
                emit(HotkeyEvent::HoldStart);
            } else if !down && was_down {
                emit(HotkeyEvent::HoldStop);
            }
        } else if down && !was_down && !other {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let last = LAST_MS.swap(now, Ordering::Relaxed);
            if now.saturating_sub(last) > 220 {
                emit(HotkeyEvent::Toggle);
            }
        }
        event
    }
}

fn emit(ev: HotkeyEvent) {
    if let Some(slot) = TX.get() {
        if let Ok(guard) = slot.lock() {
            if let Some(tx) = guard.as_ref() {
                let _ = tx.try_send(ev);
            }
        }
    }
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
    static kAXTrustedCheckOptionPrompt: *const std::ffi::c_void;
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events: u64,
        callback: extern "C" fn(
            *mut std::ffi::c_void,
            u32,
            *mut std::ffi::c_void,
            *mut std::ffi::c_void,
        ) -> *mut std::ffi::c_void,
        user_info: *mut std::ffi::c_void,
    ) -> *mut std::ffi::c_void;
    fn CGEventTapEnable(tap: *mut std::ffi::c_void, enable: bool);
    fn CGEventGetFlags(event: *mut std::ffi::c_void) -> u64;
    fn CGEventGetIntegerValueField(event: *mut std::ffi::c_void, field: u32) -> i64;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFRunLoopCommonModes: *const std::ffi::c_void;
    static kCFBooleanTrue: *const std::ffi::c_void;
    static kCFTypeDictionaryKeyCallBacks: std::ffi::c_void;
    static kCFTypeDictionaryValueCallBacks: std::ffi::c_void;
    fn CFRunLoopGetCurrent() -> *mut std::ffi::c_void;
    fn CFRunLoopAddSource(
        run_loop: *mut std::ffi::c_void,
        source: *mut std::ffi::c_void,
        mode: *const std::ffi::c_void,
    );
    fn CFRunLoopRun();
    fn CFMachPortCreateRunLoopSource(
        allocator: *mut std::ffi::c_void,
        port: *mut std::ffi::c_void,
        order: isize,
    ) -> *mut std::ffi::c_void;
    fn CFDictionaryCreate(
        allocator: *const std::ffi::c_void,
        keys: *const *const std::ffi::c_void,
        values: *const *const std::ffi::c_void,
        num_values: isize,
        key_callbacks: *const std::ffi::c_void,
        value_callbacks: *const std::ffi::c_void,
    ) -> *const std::ffi::c_void;
    fn CFRelease(value: *mut std::ffi::c_void);
}
