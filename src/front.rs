pub fn front_bundle_id() -> String {
    unsafe { bundle_id() }.unwrap_or_else(|| "unknown".to_string())
}

unsafe fn bundle_id() -> Option<String> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSWorkspace;
    let marker = MainThreadMarker::new()?;
    let workspace = NSWorkspace::sharedWorkspace();
    let app = workspace.frontmostApplication()?;
    let _ = marker;
    app.bundleIdentifier().map(|value| value.to_string())
}
