pub const TRANSCRIBE_MODEL: &str = "microsoft/mai-transcribe-2";
pub const POLISH_MODEL: &str = "deepseek/deepseek-v4.1-flash";

const TRANSCRIBE_URL: &str = "https://ai-gateway.vercel.sh/v4/ai/transcription-model";
const CHAT_URL: &str = "https://ai-gateway.vercel.sh/v1/chat/completions";

pub fn transcribe(key: &str, wav: &[u8], phrases: &[String]) -> Result<String, String> {
    let client = client()?;
    let with_phrases = transcribe_body(wav, Some(phrases));
    let response = send_transcribe(&client, key, with_phrases)?;
    if response.status().is_success() {
        return transcript_text(&response.text().unwrap_or_default());
    }
    if !phrases.is_empty() {
        let plain = send_transcribe(&client, key, transcribe_body(wav, None))?;
        if plain.status().is_success() {
            return transcript_text(&plain.text().unwrap_or_default());
        }
        return Err(error_message(plain.status(), &plain.text().unwrap_or_default()));
    }
    Err(error_message(
        response.status(),
        &response.text().unwrap_or_default(),
    ))
}

pub fn polish(key: &str, transcript: &str, bundle_id: &str, phrases: &[String]) -> Result<String, String> {
    let (system, user) = crate::polish::messages(transcript, bundle_id, phrases);
    let tokens = crate::polish::max_output_tokens(transcript);
    let client = client()?;
    let response = send_chat(&client, key, polish_body(&system, &user, tokens, true))?;
    if response.status() == reqwest::StatusCode::BAD_REQUEST {
        let retry = send_chat(&client, key, polish_body(&system, transcript, tokens, false))?;
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
            let retry = send_chat(&client, key, polish_body(&system, &user, 2000, true))?;
            if retry.status().is_success() {
                return chat_text(&retry.text().unwrap_or_default());
            }
            Err(err)
        }
        Err(err) => Err(err),
    }
}

fn polish_body(system: &str, user: &str, tokens: u32, disable_reasoning: bool) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": POLISH_MODEL,
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
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .map_err(|err| format!("无法创建网络客户端：{err}"))
}

fn send_transcribe(
    client: &reqwest::blocking::Client,
    key: &str,
    body: serde_json::Value,
) -> Result<reqwest::blocking::Response, String> {
    client
        .post(TRANSCRIBE_URL)
        .bearer_auth(key)
        .header("ai-gateway-protocol-version", "0.0.1")
        .header("ai-transcription-model-specification-version", "4")
        .header("ai-model-id", TRANSCRIBE_MODEL)
        .json(&body)
        .send()
        .map_err(|err| format!("转写请求失败：{err}"))
}

fn send_chat(
    client: &reqwest::blocking::Client,
    key: &str,
    body: serde_json::Value,
) -> Result<reqwest::blocking::Response, String> {
    client
        .post(CHAT_URL)
        .bearer_auth(key)
        .json(&body)
        .send()
        .map_err(|err| format!("订正请求失败：{err}"))
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
    let value: serde_json::Value = serde_json::from_str(raw).map_err(|_| "转写结果不是 JSON".to_string())?;
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
    let value: serde_json::Value = serde_json::from_str(raw).map_err(|_| "订正结果不是 JSON".to_string())?;
    let content = value
        .pointer("/choices/0/message/content")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    let text = crate::polish::parse_polished(content);
    if text.is_empty() {
        Err("订正没有返回正文".to_string())
    } else {
        Ok(text)
    }
}

fn error_message(status: reqwest::StatusCode, body: &str) -> String {
    let body = body.chars().take(280).collect::<String>();
    format!("Gateway 返回 {status}：{body}")
}
