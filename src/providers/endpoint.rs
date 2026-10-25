use super::{ProviderKind, DEFAULT_BASE_URL};
pub fn api_base(kind: ProviderKind, base: &str) -> String {
    let base = if base.trim().is_empty() {
        kind.base()
    } else {
        base.trim()
    }
    .trim_end_matches('/');
    if base.ends_with("/v1") {
        base.into()
    } else {
        format!("{base}/v1")
    }
}
pub fn chat_url(base: &str) -> String {
    format!("{}/chat/completions", api_base(ProviderKind::Vercel, base))
}
pub fn transcribe_url(base: &str) -> String {
    let base = if base.trim().is_empty() {
        DEFAULT_BASE_URL
    } else {
        base.trim()
    }
    .trim_end_matches('/')
    .trim_end_matches("/v1");
    format!("{base}/v4/ai/transcription-model")
}
pub fn validate_base(base: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(base).map_err(|_| "请输入完整 HTTPS 地址".to_string())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("接口必须使用 HTTPS，且不能包含凭据、查询或片段".into());
    }
    Ok(())
}
pub fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.trim().chars().collect();
    if chars.is_empty() {
        String::new()
    } else if chars.len() <= 8 {
        "••••••••".into()
    } else {
        format!(
            "{}••••••••{}",
            chars[..3].iter().collect::<String>(),
            chars[chars.len() - 4..].iter().collect::<String>()
        )
    }
}
