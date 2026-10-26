//! macOS installer helper to mount DMG, replace .app bundle, and restart.
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn running_app_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut current = exe.as_path();
    while let Some(parent) = current.parent() {
        if parent.extension().and_then(|s| s.to_str()) == Some("app") {
            return Some(parent.to_path_buf());
        }
        current = parent;
    }
    None
}

pub fn install_and_restart(dmg_path: &Path) -> Result<(), String> {
    if !dmg_path.exists() {
        return Err("安装包文件不存在".into());
    }

    let target_app =
        running_app_bundle().unwrap_or_else(|| PathBuf::from("/Applications/Notype.app"));
    let target_app_str = target_app.to_string_lossy().to_string();
    let dmg_path_str = dmg_path.to_string_lossy().to_string();
    let pid = std::process::id();

    let script = format!(
        r#"#!/bin/bash
set -e
PID={pid}
DMG_PATH="{dmg_path_str}"
TARGET_APP="{target_app_str}"
LOG_FILE="/tmp/notype_updater.log"

exec > "$LOG_FILE" 2>&1
echo "==> Notype Updater started at $(date)"
echo "Waiting for process $PID to exit..."
while kill -0 "$PID" 2>/dev/null; do
    sleep 0.2
done
echo "Process $PID exited."

MOUNT_DIR="/tmp/notype_update_mount_$$"
mkdir -p "$MOUNT_DIR"
echo "Attaching DMG to $MOUNT_DIR..."
hdiutil attach -nobrowse -readonly -mountpoint "$MOUNT_DIR" "$DMG_PATH"

SOURCE_APP="$MOUNT_DIR/Notype.app"
if [ ! -d "$SOURCE_APP" ]; then
    echo "ERROR: Notype.app not found in DMG"
    hdiutil detach "$MOUNT_DIR" -force || true
    rm -rf "$MOUNT_DIR"
    exit 1
fi

echo "Replacing $TARGET_APP..."
if rm -rf "$TARGET_APP" 2>/dev/null && cp -R "$SOURCE_APP" "$TARGET_APP" 2>/dev/null; then
    xattr -cr "$TARGET_APP" 2>/dev/null || true
    echo "App replaced successfully."
else
    echo "Warning: Direct replacement failed, falling back to opening DMG..."
    hdiutil detach "$MOUNT_DIR" -force || true
    rm -rf "$MOUNT_DIR"
    open "$DMG_PATH"
    exit 0
fi

hdiutil detach "$MOUNT_DIR" -force || true
rm -rf "$MOUNT_DIR"

echo "Launching updated app..."
open "$TARGET_APP"
echo "Update finished."
"#
    );

    let script_path = PathBuf::from("/tmp/notype_update_restart.sh");
    std::fs::write(&script_path, script).map_err(|e| format!("写入更新脚本失败: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script_path)
            .map_err(|e| e.to_string())?
            .permissions();
        perms.set_mode(0o755);
        let _ = std::fs::set_permissions(&script_path, perms);
    }

    Command::new("/bin/bash")
        .arg(&script_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("启动更新进程失败: {e}"))?;

    std::process::exit(0);
}
