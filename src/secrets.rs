use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySource {
    Missing,
    File(PathBuf),
}

pub fn env_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".env.local")
}

pub fn load_key() -> (String, KeySource) {
    let path = env_path();
    match read_key_file(&path) {
        Some(key) => (key, KeySource::File(path)),
        None => (String::new(), KeySource::Missing),
    }
}

pub fn save_key(key: &str) -> std::io::Result<PathBuf> {
    let path = env_path();
    let body = format!(
        "# 把 Vercel AI Gateway API Key 填在等号后面并保存。\n# 应用启动时会读这个文件；已经打开时，在设置里点「重新读取」。\nAI_GATEWAY_API_KEY={}\n",
        key.trim()
    );
    std::fs::write(&path, body)?;
    Ok(path)
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
    use super::parse_env;

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
