use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const FLAGS_CHANGED: u32 = 12;
const KEYCODE_FIELD: u32 = 9;
const RIGHT_OPTION: i64 = 61;
const ALT: u64 = 0x0008_0000;
const SHIFT: u64 = 0x0002_0000;
const CONTROL: u64 = 0x0004_0000;
const COMMAND: u64 = 0x0010_0000;
const DISABLED_TIMEOUT: u32 = 0xFFFF_FFFE;
const DISABLED_USER: u32 = 0xFFFF_FFFF;

static TAP: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
static WAS_DOWN: AtomicBool = AtomicBool::new(false);
static LAST_MS: AtomicU64 = AtomicU64::new(0);
static TX: OnceLock<Mutex<Option<mpsc::SyncSender<()>>>> = OnceLock::new();

pub fn listen() -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::sync_channel(8);
    TX.get_or_init(|| Mutex::new(None))
        .lock()
        .expect("hotkey sender")
        .replace(tx);
    std::thread::Builder::new()
        .name("opentypeless-hotkey".into())
        .spawn(|| unsafe { run_loop() })
        .expect("hotkey thread");
    rx
}

unsafe fn run_loop() {
    unsafe {
        let mask = 1u64 << FLAGS_CHANGED;
        let tap = CGEventTapCreate(
            1,
            0,
            1,
            mask,
            tap_callback,
            std::ptr::null_mut(),
        );
        if tap.is_null() {
            eprintln!("热键监听没有启动。请在系统设置的辅助功能里允许 OpenTypeless。");
            return;
        }
        TAP.store(tap, Ordering::Relaxed);
        let source = CFMachPortCreateRunLoopSource(std::ptr::null_mut(), tap, 0);
        let run_loop = CFRunLoopGetCurrent();
        CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
        CFRunLoopRun();
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
        if event_type != FLAGS_CHANGED {
            return event;
        }
        let code = CGEventGetIntegerValueField(event, KEYCODE_FIELD);
        let flags = CGEventGetFlags(event);
        if code != RIGHT_OPTION {
            return event;
        }
        let down = flags & ALT != 0;
        let other = flags & (SHIFT | CONTROL | COMMAND) != 0;
        let was_down = WAS_DOWN.swap(down, Ordering::Relaxed);
        if down && !was_down && !other {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let last = LAST_MS.swap(now, Ordering::Relaxed);
            if now.saturating_sub(last) > 250 {
                if let Some(slot) = TX.get() {
                    if let Ok(guard) = slot.lock() {
                        if let Some(tx) = guard.as_ref() {
                            let _ = tx.try_send(());
                        }
                    }
                }
            }
        }
        event
    }
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
}
