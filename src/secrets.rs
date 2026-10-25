//! Keychain credentials with verified, backwards-compatible legacy migration.
use crate::providers::ProviderKind;
use std::path::PathBuf;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySource {
    Missing,
    Keychain,
    MigrationPending,
}
pub fn load_key() -> (String, KeySource) {
    match load_provider(ProviderKind::Vercel) {
        Ok(key) => {
            let source = if key.is_empty() {
                KeySource::Missing
            } else {
                KeySource::Keychain
            };
            (key, source)
        }
        Err(_) => (
            legacy_key().unwrap_or_default(),
            KeySource::MigrationPending,
        ),
    }
}
pub fn load_provider(kind: ProviderKind) -> Result<String, String> {
    if let Some(key) = crate::keychain::load(kind.id())? {
        return Ok(key);
    }
    if kind == ProviderKind::Vercel {
        if let Some(key) = legacy_key() {
            save_provider(kind, &key)?;
            return Ok(key);
        }
    }
    Ok(String::new())
}
fn legacy_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(dir) = crate::store::Store::data_dir() {
        paths.push(dir.join(".env.local"));
    }
    if std::env::var_os("NOTYPE_DATA_DIR").is_some() {
        return paths;
    }
    let developer = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".env.local");
    if !paths.contains(&developer) {
        paths.push(developer);
    }
    paths
}
fn legacy_key() -> Option<String> {
    if let Ok(store) = crate::store::Store::open() {
        let key = store.api_key();
        if !key.is_empty() {
            return Some(key);
        }
    }
    legacy_paths()
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok().and_then(|s| parse_env(&s)))
}
fn clear_legacy() -> Result<(), String> {
    // Remove only this application's key assignment, preserving unrelated environment variables.
    for path in legacy_paths() {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                if text
                    .lines()
                    .any(|l| l.trim().starts_with("AI_GATEWAY_API_KEY="))
                {
                    let text = text
                        .lines()
                        .filter(|l| !l.trim().starts_with("AI_GATEWAY_API_KEY="))
                        .collect::<Vec<_>>()
                        .join("\n");
                    crate::tasks::atomic_write(&path, text.as_bytes())?;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err("无法清理旧密钥文件；钥匙串副本已保存".into()),
        }
    }
    crate::store::Store::open()
        .map_err(|_| "无法打开旧密钥配置")?
        .set_api_key("")
        .map_err(|_| "无法清理旧数据库密钥".to_string())
}
pub fn save_provider(kind: ProviderKind, key: &str) -> Result<(), String> {
    crate::keychain::save(kind.id(), key.trim())?;
    if kind == ProviderKind::Vercel {
        clear_legacy()?;
    }
    Ok(())
}
pub fn clear_provider(kind: ProviderKind) -> Result<(), String> {
    if kind == ProviderKind::Vercel {
        clear_legacy()?;
    }
    crate::keychain::delete(kind.id())
}
pub fn parse_env(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(value) = line.strip_prefix("AI_GATEWAY_API_KEY=") else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'').trim();
        if value.is_empty() {
            return None;
        }
        return Some(value.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_key_and_ignores_comments() {
        let text = "# comment\nAI_GATEWAY_API_KEY=vck_test\n";
        assert_eq!(parse_env(text).as_deref(), Some("vck_test"));
    }

    #[test]
    fn empty_value_is_missing() {
        assert!(parse_env("AI_GATEWAY_API_KEY=\n").is_none());
        assert!(parse_env("AI_GATEWAY_API_KEY=\"\"\n").is_none());
    }
}
