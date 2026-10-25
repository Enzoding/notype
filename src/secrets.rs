use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySource {
    Missing,
    Database,
    File(PathBuf),
}

pub fn env_path() -> PathBuf {
    if let Ok(dir) = crate::store::Store::data_dir() {
        let app_env = dir.join(".env.local");
        if app_env.exists() {
            return app_env;
        }
    }
    let manifest_env = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".env.local");
    if manifest_env.exists() {
        return manifest_env;
    }
    if let Ok(dir) = crate::store::Store::data_dir() {
        return dir.join(".env.local");
    }
    manifest_env
}

pub fn load_key() -> (String, KeySource) {
    // 1. Check SQLite database (primary storage for user-entered keys in open-source app)
    if let Ok(store) = crate::store::Store::open() {
        let db_key = store.api_key();
        if !db_key.is_empty() {
            return (db_key, KeySource::Database);
        }
    }
    // 2. Check ~/Library/Application Support/Notype/.env.local
    if let Ok(dir) = crate::store::Store::data_dir() {
        let app_env = dir.join(".env.local");
        if app_env.exists() {
            if let Some(key) = read_key_file(&app_env) {
                if let Ok(store) = crate::store::Store::open() {
                    let _ = store.set_api_key(&key);
                }
                return (key, KeySource::File(app_env));
            }
        }
    }
    // 3. Check developer CARGO_MANIFEST_DIR/.env.local (convenient for local dev)
    let manifest_env = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".env.local");
    if manifest_env.exists() {
        if let Some(key) = read_key_file(&manifest_env) {
            if let Ok(store) = crate::store::Store::open() {
                let _ = store.set_api_key(&key);
            }
            return (key, KeySource::File(manifest_env));
        }
    }
    (String::new(), KeySource::Missing)
}

pub fn save_key(key: &str) -> std::io::Result<PathBuf> {
    let key_trimmed = key.trim();
    // 1. Always save directly into SQLite database
    if let Ok(store) = crate::store::Store::open() {
        let _ = store.set_api_key(key_trimmed);
    }
    // 2. Also write to .env.local for file-based tools / compatibility
    let body = format!(
        "# Notype API Key 配置文件\nAI_GATEWAY_API_KEY={}\n",
        key_trimmed
    );
    let target = if let Ok(dir) = crate::store::Store::data_dir() {
        let _ = std::fs::create_dir_all(&dir);
        let app_env = dir.join(".env.local");
        let _ = std::fs::write(&app_env, &body);
        let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".env.local");
        if manifest_path.exists() {
            let _ = std::fs::write(&manifest_path, &body);
        }
        app_env
    } else {
        let path = env_path();
        std::fs::write(&path, &body)?;
        path
    };
    Ok(target)
}

pub fn clear_key() {
    if let Ok(store) = crate::store::Store::open() {
        let _ = store.set_api_key("");
    }
    if let Ok(dir) = crate::store::Store::data_dir() {
        let app_env = dir.join(".env.local");
        if app_env.exists() {
            let _ = std::fs::remove_file(app_env);
        }
    }
}

pub fn read_key_file(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    parse_env(&text)
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
