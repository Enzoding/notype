use super::transport::{client, error_message, send_with_retry};
use super::{api_base, transcribe_url, ProviderKind, TRANSCRIBE_MODEL};

static MAX_SAFE_PHRASES: std::sync::Mutex<Option<usize>> = std::sync::Mutex::new(None);

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
        TRANSCRIBE_MODEL
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
    // degrade quality). Step down safely and cache the safe capacity.
    let safe_limit = MAX_SAFE_PHRASES.lock().ok().and_then(|g| *g).unwrap_or(25);
    let initial_cap = phrases.len().min(safe_limit);
    let mut caps = vec![initial_cap, 10, 0];
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
            if let Ok(mut guard) = MAX_SAFE_PHRASES.lock() {
                if guard.is_none() || guard.is_some_and(|g| g < cap) {
                    *guard = Some(cap.max(25));
                }
            }
            return transcript_text(&response.text().unwrap_or_default());
        }
        let text = response.text().unwrap_or_default();
        last_error = Some(error_message(status, &text));
        let retryable = status == reqwest::StatusCode::BAD_REQUEST
            || status == reqwest::StatusCode::UNPROCESSABLE_ENTITY;
        if retryable {
            if let Ok(mut guard) = MAX_SAFE_PHRASES.lock() {
                *guard = Some(cap.saturating_sub(1));
            }
        } else {
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
    /// Pins the Vercel AI Gateway transcription wire contract (headers + JSON
    /// body) so protocol drift is caught without a paid request.
    #[test]
    fn vercel_request_sends_protocol_headers_and_body_on_the_wire() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut data = Vec::new();
            let mut buf = [0; 4096];
            while let Ok(n) = stream.read(&mut buf) {
                if n == 0 {
                    break;
                }
                data.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&data);
                if let Some((head, body)) = text.split_once("\r\n\r\n") {
                    let length = head
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if body.len() >= length {
                        break;
                    }
                }
            }
            let body = r#"{"text":"ok"}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            String::from_utf8(data).unwrap()
        });
        let client = reqwest::blocking::Client::new();
        let body = transcribe_body(&[1, 2, 3], Some(&["Notype".into()]));
        let response = send_transcribe(
            &client,
            &format!("http://{address}/v4/ai/transcription-model"),
            "test-key",
            "model/x",
            body,
        )
        .unwrap();
        assert_eq!(transcript_text(&response.text().unwrap()).unwrap(), "ok");
        let request = server.join().unwrap();
        let lower = request.to_ascii_lowercase();
        assert!(request.starts_with("POST /v4/ai/transcription-model "));
        assert!(lower.contains("authorization: bearer test-key"));
        assert!(lower.contains("ai-gateway-protocol-version: 0.0.1"));
        assert!(lower.contains("ai-transcription-model-specification-version: 4"));
        assert!(lower.contains("ai-model-id: model/x"));
        let json: serde_json::Value =
            serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(json["audio"], "AQID");
        assert_eq!(json["mediaType"], "audio/wav");
        assert_eq!(
            json["providerOptions"]["azure"]["phraseList"]["phrases"][0],
            "Notype"
        );
    }
}
