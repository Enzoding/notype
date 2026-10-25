//! Best-effort reading of the focused text field through the macOS
//! Accessibility API. Used only *after* a paste has already completed, from a
//! background thread, so it can never slow down dictation.
//!
//! Many apps expose `AXValue` for their focused text field (native AppKit,
//! most Electron apps, web inputs). Some terminal emulators and canvas-based
//! editors do not; in that case we simply return `None` and no correction is
//! learned.

use std::ffi::c_void;
use std::ptr::NonNull;

use objc2_core_foundation::{CFRetained, CFString, CFType};

#[derive(Clone, Debug)]
pub struct FocusedText {
    pub pid: i32,
    pub text: String,
}

/// Read the focused text element. Returns `None` if Accessibility is not
/// trusted, the focused element has no plain-text value, or the value is too
/// large to be a normal input field.
pub fn focused_text() -> Option<FocusedText> {
    if !crate::hotkey::is_accessibility_trusted() {
        return None;
    }
    unsafe { focused_text_inner() }
}

unsafe fn focused_text_inner() -> Option<FocusedText> {
    let system_ptr = unsafe { AXUIElementCreateSystemWide() };
    let system =
        unsafe { CFRetained::<CFType>::from_raw(NonNull::new(system_ptr.cast::<CFType>())?) };
    unsafe {
        AXUIElementSetMessagingTimeout(CFRetained::as_ptr(&system).as_ptr().cast::<c_void>(), 0.5);
    }

    let focused = unsafe { copy_attr(&system, "AXFocusedUIElement")? };
    unsafe {
        AXUIElementSetMessagingTimeout(CFRetained::as_ptr(&focused).as_ptr().cast::<c_void>(), 0.5);
    }

    let role = unsafe { copy_string(&focused, "AXRole") }.unwrap_or_default();
    let subrole = unsafe { copy_string(&focused, "AXSubrole") }.unwrap_or_default();
    if role.contains("Secure") || subrole.contains("Secure") {
        return None;
    }
    // Only text-bearing roles are useful. If an app reports a generic group we
    // still try `AXValue`, because some web views do exactly that.
    if !(role.contains("Text") || role.contains("ComboBox") || role == "AXGroup") {
        return None;
    }

    let text = unsafe { copy_string(&focused, "AXValue") }?;
    if text.len() > 20_000 {
        return None;
    }
    let text = text.trim_end_matches('\u{0}').to_string();
    if text.trim().is_empty() {
        return None;
    }

    let mut pid: i32 = 0;
    unsafe {
        AXUIElementGetPid(
            CFRetained::as_ptr(&focused).as_ptr().cast::<c_void>(),
            &mut pid,
        );
    }

    let _ = role;
    Some(FocusedText { pid, text })
}

unsafe fn copy_attr(element: &CFType, name: &'static str) -> Option<CFRetained<CFType>> {
    let attr = CFString::from_static_str(name);
    let mut value: *mut c_void = std::ptr::null_mut();
    let err = unsafe {
        AXUIElementCopyAttributeValue(
            (element as *const CFType).cast_mut().cast::<c_void>(),
            CFRetained::as_ptr(&attr).as_ptr().cast::<c_void>(),
            &mut value,
        )
    };
    if err != 0 || value.is_null() {
        return None;
    }
    let ptr = NonNull::new(value.cast::<CFType>())?;
    Some(unsafe { CFRetained::from_raw(ptr) })
}

unsafe fn copy_string(element: &CFType, name: &'static str) -> Option<String> {
    let value = unsafe { copy_attr(element, name) }?;
    value.downcast::<CFString>().ok().map(|s| s.to_string())
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateSystemWide() -> *mut c_void;
    fn AXUIElementCopyAttributeValue(
        element: *mut c_void,
        attribute: *const c_void,
        value: *mut *mut c_void,
    ) -> i32;
    fn AXUIElementSetMessagingTimeout(element: *mut c_void, timeout: f32) -> i32;
    fn AXUIElementGetPid(element: *mut c_void, pid: *mut i32) -> i32;
}
