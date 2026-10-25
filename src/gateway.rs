use std::time::Instant;

use crate::polish::PolishOptions;

pub const DEFAULT_BASE_URL: &str = "https://ai-gateway.vercel.sh";
pub const TRANSCRIBE_MODEL: &str = "microsoft/mai-transcribe-2";
pub const POLISH_MODEL: &str = "deepseek/deepseek-v4.1-flash";

pub fn transcribe_url(base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        format!("{DEFAULT_BASE_URL}/v4/ai/transcription-model")
    } else {
        format!("{base}/v4/ai/transcription-model")
    }
}

pub fn chat_url(base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        format!("{DEFAULT_BASE_URL}/v1/chat/completions")
    } else {
        format!("{base}/v1/chat/completions")
    }
}

pub fn mask_key(key: &str) -> String {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.len() <= 8 {
        return "••••••••".to_string();
    }
    let prefix_len = if trimmed.starts_with("vck_") { 4 } else { 3.min(trimmed.len()) };
    let suffix_len = 4.min(trimmed.len().saturating_sub(prefix_len));
    format!(
        "{}••••••••{}",
        &trimmed[..prefix_len],
        &trimmed[trimmed.len() - suffix_len..]
    )
}

pub fn transcribe_with_model(
    base_url: &str,
    key: &str,
    wav: &[u8],
    phrases: &[String],
    model: &str,
) -> Result<String, String> {
    let model = if model.trim().is_empty() {
        TRANSCRIBE_MODEL
    } else {
        model.trim()
    };
    let client = client()?;
    let url = transcribe_url(base_url);
    // Azure accepts a phrase list, but a very long list can be rejected (or
    // degrade quality). If the request fails with a client error, step down to
    // smaller lists instead of dropping the whole dictionary on the floor.
    let mut caps = vec![phrases.len().min(crate::mining::ACTIVE_PHRASE_LIMIT), 50, 20, 0];
    caps.dedup();
    caps.retain(|cap| *cap == 0 || *cap <= phrases.len());
    let mut last_error = None;
    for cap in caps {
        let body = if cap == 0 {
            transcribe_body(wav, None)
        } else {
            transcribe_body(wav, Some(&phrases[..cap]))
        };
        let response = send_transcribe(&client, &url, key, model, body)?;
        let status = response.status();
        if status.is_success() {
            return transcript_text(&response.text().unwrap_or_default());
        }
        let text = response.text().unwrap_or_default();
        last_error = Some(error_message(status, &text));
        let retryable = status == reqwest::StatusCode::BAD_REQUEST
            || status == reqwest::StatusCode::PAYLOAD_TOO_LARGE
            || status == reqwest::StatusCode::UNPROCESSABLE_ENTITY;
        if !retryable {
            break;
        }
    }
    Err(last_error.unwrap_or_else(|| "转写失败".into()))
}

pub fn polish_with_options(
    base_url: &str,
    key: &str,
    transcript: &str,
    phrases: &[String],
    options: &PolishOptions,
    model: &str,
) -> Result<String, String> {
    let model = if model.trim().is_empty() {
        POLISH_MODEL
    } else {
        model.trim()
    };
    let (system, user) = crate::polish::messages_with_options(transcript, phrases, options);
    let tokens = crate::polish::max_output_tokens(transcript);
    let client = client()?;
    let url = chat_url(base_url);
    let response = send_chat(&client, &url, key, polish_body(model, &system, &user, tokens, true))?;
    if response.status() == reqwest::StatusCode::BAD_REQUEST {
        let retry = send_chat(
            &client,
            &url,
            key,
            polish_body(model, &system, transcript, tokens, false),
        )?;
        if retry.status().is_success() {
            return chat_text(&retry.text().unwrap_or_default());
        }
        return Err(error_message(retry.status(), &retry.text().unwrap_or_default()));
    }
    if !response.status().is_success() {
        return Err(error_message(
            response.status(),
            &response.text().unwrap_or_default(),
        ));
    }
    match chat_text(&response.text().unwrap_or_default()) {
        Ok(text) => Ok(text),
        Err(err) if tokens < 2000 => {
            let retry = send_chat(
                &client,
                &url,
                key,
                polish_body(model, &system, &user, 2000, true),
            )?;
            if retry.status().is_success() {
                return chat_text(&retry.text().unwrap_or_default());
            }
            Err(err)
        }
        Err(err) => Err(err),
    }
}

/// Ask the polish model to extract dictionary candidates from final
/// transcripts. The returned string is the raw model content (JSON); parsing
/// and validation live in `mining`.
pub fn extract_glossary(
    base_url: &str,
    key: &str,
    model: &str,
    prompt: &str,
) -> Result<String, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("尚未配置 API Key".into());
    }
    let model = if model.trim().is_empty() {
        POLISH_MODEL
    } else {
        model.trim()
    };
    let client = client()?;
    let url = chat_url(base_url);
    let body = serde_json::json!({
        "model": model,
        "temperature": 0.0,
        "max_tokens": 1400,
        "messages": [
            {"role": "user", "content": prompt}
        ],
        "reasoning": {"enabled": false}
    });
    let response = send_chat(&client, &url, key, body)?;
    if !response.status().is_success() {
        return Err(error_message(
            response.status(),
            &response.text().unwrap_or_default(),
        ));
    }
    chat_content(&response.text().unwrap_or_default())
}

pub fn test_connection(base_url: &str, key: &str, model: &str) -> Result<u128, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("尚未配置 API Key".into());
    }
    let model = if model.trim().is_empty() {
        POLISH_MODEL
    } else {
        model.trim()
    };
    let mut builder = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15));
    builder = configure_proxy(builder);
    let client = builder
        .build()
        .map_err(|err| format!("网络客户端错误：{err}"))?;
    let started = Instant::now();
    let url = chat_url(base_url);
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 8,
        "temperature": 0.0,
        "messages": [
            {"role": "user", "content": "Reply OK"}
        ]
    });
    let response = send_chat(&client, &url, key, body)?;
    let elapsed_ms = started.elapsed().as_millis();
    if response.status().is_success() {
        Ok(elapsed_ms)
    } else {
        Err(error_message(
            response.status(),
            &response.text().unwrap_or_default(),
        ))
    }
}

fn polish_body(
    model: &str,
    system: &str,
    user: &str,
    tokens: u32,
    disable_reasoning: bool,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": model,
        "temperature": 0.2,
        "max_tokens": tokens,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ]
    });
    if disable_reasoning {
        body["reasoning"] = serde_json::json!({"enabled": false});
    }
    body
}

fn client() -> Result<reqwest::blocking::Client, String> {
    let mut builder = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(90));
    builder = configure_proxy(builder);
    builder
        .build()
        .map_err(|err| format!("无法创建网络客户端：{err}"))
}

fn configure_proxy(mut builder: reqwest::blocking::ClientBuilder) -> reqwest::blocking::ClientBuilder {
    // If environment variables already specify a proxy (e.g. in terminal), reqwest respects them.
    // However, macOS GUI applications launched from Finder / launchd do not inherit shell env vars.
    // In that case, automatically detect and apply the active macOS system proxy:
    if std::env::var("https_proxy").is_err()
        && std::env::var("HTTPS_PROXY").is_err()
        && std::env::var("all_proxy").is_err()
        && std::env::var("ALL_PROXY").is_err()
    {
        if let Some(proxy_url) = detect_macos_system_proxy() {
            if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                builder = builder.proxy(proxy);
            }
        }
    }
    builder
}

pub fn detect_macos_system_proxy() -> Option<String> {
    let output = std::process::Command::new("scutil")
        .arg("--proxy")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_scutil_proxy(&text)
}

pub fn parse_scutil_proxy(text: &str) -> Option<String> {
    let mut https_enabled = false;
    let mut https_host = String::new();
    let mut https_port = 0u16;

    let mut socks_enabled = false;
    let mut socks_host = String::new();
    let mut socks_port = 0u16;

    let mut http_enabled = false;
    let mut http_host = String::new();
    let mut http_port = 0u16;

    for line in text.lines() {
        let line = line.trim();
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim();
            let val = v.trim();
            match key {
                "HTTPSEnable" => https_enabled = val == "1",
                "HTTPSProxy" => https_host = val.to_string(),
                "HTTPSPort" => https_port = val.parse().unwrap_or(0),
                "HTTPEnable" => http_enabled = val == "1",
                "HTTPProxy" => http_host = val.to_string(),
                "HTTPPort" => http_port = val.parse().unwrap_or(0),
                "SOCKSEnable" => socks_enabled = val == "1",
                "SOCKSProxy" => socks_host = val.to_string(),
                "SOCKSPort" => socks_port = val.parse().unwrap_or(0),
                _ => {}
            }
        }
    }

    if https_enabled && !https_host.is_empty() && https_port > 0 {
        return Some(format!("http://{https_host}:{https_port}"));
    }
    if http_enabled && !http_host.is_empty() && http_port > 0 {
        return Some(format!("http://{http_host}:{http_port}"));
    }
    if socks_enabled && !socks_host.is_empty() && socks_port > 0 {
        return Some(format!("socks5h://{socks_host}:{socks_port}"));
    }
    None
}

fn send_transcribe(
    client: &reqwest::blocking::Client,
    url: &str,
    key: &str,
    model: &str,
    body: serde_json::Value,
) -> Result<reqwest::blocking::Response, String> {
    client
        .post(url)
        .bearer_auth(key)
        .header("ai-gateway-protocol-version", "0.0.1")
        .header("ai-transcription-model-specification-version", "4")
        .header("ai-model-id", model)
        .json(&body)
        .send()
        .map_err(|err| format!("转写请求失败：{err}"))
}

fn send_chat(
    client: &reqwest::blocking::Client,
    url: &str,
    key: &str,
    body: serde_json::Value,
) -> Result<reqwest::blocking::Response, String> {
    client
        .post(url)
        .bearer_auth(key)
        .json(&body)
        .send()
        .map_err(|err| format!("请求失败：{err}"))
}

fn transcribe_body(wav: &[u8], phrases: Option<&[String]>) -> serde_json::Value {
    let mut body = serde_json::json!({
        "audio": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, wav),
        "mediaType": "audio/wav"
    });
    if let Some(phrases) = phrases {
        if !phrases.is_empty() {
            body["providerOptions"] = serde_json::json!({
                "azure": {
                    "transcribeStyle": "clean",
                    "phraseList": { "phrases": phrases }
                }
            });
        }
    }
    body
}

fn transcript_text(raw: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| "转写结果不是 JSON".to_string())?;
    let text = value
        .get("text")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if text.is_empty() {
        Err("没有识别到内容".to_string())
    } else {
        Ok(text)
    }
}

fn chat_text(raw: &str) -> Result<String, String> {
    let content = chat_content(raw)?;
    let text = crate::polish::parse_polished(&content);
    if text.is_empty() {
        Err("订正没有返回正文".to_string())
    } else {
        Ok(text)
    }
}

fn chat_content(raw: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| "模型结果不是 JSON".to_string())?;
    let content = value
        .pointer("/choices/0/message/content")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .trim();
    if content.is_empty() {
        Err("模型没有返回内容".to_string())
    } else {
        Ok(content.to_string())
    }
}

fn error_message(status: reqwest::StatusCode, body: &str) -> String {
    let body = body.chars().take(280).collect::<String>();
    format!("Gateway 返回 {status}：{body}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scutil_proxy_https_enabled() {
        let scutil_out = "\
<dictionary> {
  HTTPEnable : 1
  HTTPPort : 7897
  HTTPProxy : 127.0.0.1
  HTTPSEnable : 1
  HTTPSPort : 7897
  HTTPSProxy : 127.0.0.1
  SOCKSEnable : 1
  SOCKSPort : 7897
  SOCKSProxy : 127.0.0.1
}";
        assert_eq!(
            parse_scutil_proxy(scutil_out),
            Some("http://127.0.0.1:7897".into())
        );
    }

    #[test]
    fn parses_scutil_proxy_disabled() {
        let scutil_out = "\
<dictionary> {
  HTTPEnable : 0
  HTTPSEnable : 0
  SOCKSEnable : 0
}";
        assert_eq!(parse_scutil_proxy(scutil_out), None);
    }

    #[test]
    fn builds_urls_properly() {
        assert_eq!(
            transcribe_url("https://ai-gateway.vercel.sh"),
            "https://ai-gateway.vercel.sh/v4/ai/transcription-model"
        );
        assert_eq!(
            chat_url("https://ai-gateway.vercel.sh/"),
            "https://ai-gateway.vercel.sh/v1/chat/completions"
        );
        assert_eq!(
            chat_url("https://api.mygateway.com/v1"),
            "https://api.mygateway.com/v1/v1/chat/completions"
        );
    }

    #[test]
    fn masks_api_key_safely() {
        assert_eq!(mask_key(""), "");
        assert_eq!(mask_key("12345"), "••••••••");
        assert_eq!(mask_key("vck_1234567890abcdef"), "vck_••••••••cdef");
        assert_eq!(mask_key("sk-proj-1234567890abcdef"), "sk-••••••••cdef");
    }
}
