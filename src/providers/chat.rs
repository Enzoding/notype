use super::transport::{client, configure_proxy, error_message, send_chat};
use super::{chat_url, POLISH_MODEL};
use crate::polish::PolishOptions;
use std::time::Instant;

pub fn polish_with_options(
    base_url: &str,
    key: &str,
    transcript: &str,
    phrases: &[String],
    options: &PolishOptions,
    model: &str,
) -> Result<String, String> {
    super::validate_base(if base_url.trim().is_empty() {
        super::DEFAULT_BASE_URL
    } else {
        base_url
    })?;
    let model = if model.trim().is_empty() {
        POLISH_MODEL
    } else {
        model.trim()
    };
    let (system, user) = crate::polish::messages_with_options(transcript, phrases, options);
    let tokens = crate::polish::max_output_tokens(transcript);
    let client = client()?;
    let url = chat_url(base_url);
    let response = send_chat(
        &client,
        &url,
        key,
        polish_body(model, &system, &user, tokens, true),
    )?;
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
        return Err(error_message(
            retry.status(),
            &retry.text().unwrap_or_default(),
        ));
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
    super::validate_base(if base_url.trim().is_empty() {
        super::DEFAULT_BASE_URL
    } else {
        base_url
    })?;
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
    super::validate_base(if base_url.trim().is_empty() {
        super::DEFAULT_BASE_URL
    } else {
        base_url
    })?;
    let key = key.trim();
    if key.is_empty() {
        return Err("尚未配置 API Key".into());
    }
    let model = if model.trim().is_empty() {
        POLISH_MODEL
    } else {
        model.trim()
    };
    let mut builder =
        reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(15));
    builder = builder
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(10));
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
    if value
        .pointer("/choices/0/finish_reason")
        .and_then(|v| v.as_str())
        == Some("length")
    {
        return Err("润色结果被渠道截断，保留完整原文供重试".into());
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_empty_and_malformed_chat_results() {
        assert!(chat_text(
            r#"{"choices":[{"finish_reason":"length","message":{"content":"partial"}}]}"#
        )
        .is_err());
        assert!(chat_text("bad").is_err());
        assert!(chat_text(r#"{"choices":[]}"#).is_err());
        assert_eq!(
            chat_text(r#"{"choices":[{"message":{"content":"synthetic text"}}]}"#).unwrap(),
            "synthetic text"
        );
    }
}
