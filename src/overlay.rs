use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSWindow, NSWindowButton, NSWindowTitleVisibility};

const VOICE_TITLE: &str = "OpenTypeless Voice";

pub fn prepare_voice_window() {
    with_voice_window(|window| {
        quiet_panel(window);
        window.setAlphaValue(0.0);
    });
}

pub fn set_voice_visible(visible: bool) {
    with_voice_window(|window| {
        quiet_panel(window);
        window.setAlphaValue(if visible { 1.0 } else { 0.0 });
        if visible {
            window.orderFrontRegardless();
        }
    });
}

fn quiet_panel(window: &NSWindow) {
    window.setHidesOnDeactivate(false);
    window.setIgnoresMouseEvents(true);
    window.setTitlebarAppearsTransparent(true);
    window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
    for kind in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(button) = window.standardWindowButton(kind) {
            button.setHidden(true);
        }
    }
}

fn with_voice_window(apply: impl Fn(&NSWindow)) {
    let Some(marker) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(marker);
    for window in app.windows() {
        if window.title().to_string() == VOICE_TITLE {
            apply(&window);
        }
    }
}
