use super::transport::{client, error_message, send_with_retry};
use super::{api_base, transcribe_url, ProviderKind, TRANSCRIBE_MODEL};

pub fn transcribe_with_model(
    kind: ProviderKind,
    base_url: &str,
    key: &str,
    wav: &[u8],
    phrases: &[String],
    model: &str,
) -> Result<String, String> {
    super::validate_base(if base_url.trim().is_empty() {
        kind.base()
    } else {
        base_url
    })?;
    let model = if model.trim().is_empty() {
        if kind == ProviderKind::OpenRouter {
            "openai/whisper-large-v3"
        } else {
            TRANSCRIBE_MODEL
        }
    } else {
        model.trim()
    };
    let client = client()?;
    if kind == ProviderKind::OpenRouter {
        let body = openrouter_body(model, wav);
        let response = send_with_retry(|| {
            client
                .post(format!("{}/audio/transcriptions", api_base(kind, base_url)))
                .bearer_auth(key)
                .json(&body)
                .send()
        })?;
        if !response.status().is_success() {
            return Err(error_message(response.status(), ""));
        }
        return transcript_text(&response.text().unwrap_or_default());
    }
    let url = transcribe_url(base_url);
    // Azure accepts a phrase list, but a very long list can be rejected (or
    // degrade quality). If the request fails with a client error, step down to
    // smaller lists instead of dropping the whole dictionary on the floor.
    let mut caps = vec![
        phrases.len().min(crate::mining::ACTIVE_PHRASE_LIMIT),
        50,
        20,
        0,
    ];
    let mut seen = std::collections::HashSet::new();
    caps.retain(|cap| (*cap == 0 || *cap <= phrases.len()) && seen.insert(*cap));
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
            || status == reqwest::StatusCode::UNPROCESSABLE_ENTITY;
        if !retryable {
            break;
        }
    }
    Err(last_error.unwrap_or_else(|| "转写失败".into()))
}

fn openrouter_body(model: &str, wav: &[u8]) -> serde_json::Value {
    serde_json::json!({"model":model,"input_audio":{"data":base64::Engine::encode(&base64::engine::general_purpose::STANDARD,wav),"format":"wav"}})
}
fn send_transcribe(
    client: &reqwest::blocking::Client,
    url: &str,
    key: &str,
    model: &str,
    body: serde_json::Value,
) -> Result<reqwest::blocking::Response, String> {
    send_with_retry(|| {
        client
            .post(url)
            .bearer_auth(key)
            .header("ai-gateway-protocol-version", "0.0.1")
            .header("ai-transcription-model-specification-version", "4")
            .header("ai-model-id", model)
            .json(&body)
            .send()
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn openrouter_request_uses_stt_contract_without_vercel_options() {
        let body = openrouter_body("openai/whisper-large-v3", &[1, 2, 3]);
        assert_eq!(body["input_audio"]["data"], "AQID");
        assert_eq!(body["input_audio"]["format"], "wav");
        assert_eq!(body["model"], "openai/whisper-large-v3");
        assert!(body.get("providerOptions").is_none());
    }
    #[test]
    fn rejects_empty_or_malformed_response() {
        assert!(transcript_text("{}").is_err());
        assert!(transcript_text("bad").is_err());
        assert_eq!(transcript_text(r#"{"text":"  hello  "}"#).unwrap(), "hello");
    }
    #[test]
    fn vercel_hotwords_are_provider_specific() {
        let body = transcribe_body(&[1, 2], Some(&["Notype".into()]));
        assert_eq!(
            body["providerOptions"]["azure"]["phraseList"]["phrases"][0],
            "Notype"
        );
    }
}
