#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolishMode {
    Prose,
    Literal,
    Email,
    Chat,
}

pub fn mode_for_bundle(bundle_id: &str) -> PolishMode {
    const LITERAL_PREFIXES: &[&str] = &[
        "com.apple.dt.Xcode",
        "com.microsoft.VSCode",
        "com.todesktop.",
        "com.apple.Terminal",
        "com.googlecode.iterm2",
        "dev.warp.Warp",
        "com.mitchellh.ghostty",
    ];
    const EMAIL: &[&str] = &["com.apple.mail", "com.microsoft.Outlook"];
    const CHAT: &[&str] = &[
        "com.tinyspeck.slackmacgap",
        "com.tencent.xinWeChat",
        "net.whatsapp.WhatsApp",
        "com.apple.MobileSMS",
    ];
    if LITERAL_PREFIXES.iter().any(|id| bundle_id.starts_with(id)) {
        PolishMode::Literal
    } else if EMAIL.iter().any(|id| bundle_id == *id) {
        PolishMode::Email
    } else if CHAT.iter().any(|id| bundle_id == *id) {
        PolishMode::Chat
    } else {
        PolishMode::Prose
    }
}

pub fn messages(transcript: &str, bundle_id: &str, phrases: &[String]) -> (String, String) {
    let mode = mode_for_bundle(bundle_id);
    let mode_rule = match mode {
        PolishMode::Literal => {
            "当前应用是代码或终端。去掉口水词和重复，补上标点，套用词库。不要把代码改写成说明。"
        }
        PolishMode::Email => {
            "整理成可以直接发送的邮件正文。删掉口水词，把要求或事项写成短列表。不要写主题行，除非用户把主题说出来了。"
        }
        PolishMode::Chat => "整理成短消息。删掉口水词和重复，保留原来的语气，一句话能说清就不要铺开。",
        PolishMode::Prose => {
            "把口语收成可以直接阅读的正文，不要原样保留口述。删掉口水词、重复和没说完的半句。按意思分段；出现步骤、对比或几个要求时写成列表。"
        },
    };
    let glossary = if phrases.is_empty() {
        "没有额外词库。".to_string()
    } else {
        format!("这些写法必须原样保留：{}", phrases.join("、"))
    };
    let system = format!(
        "你在把语音转写整理成正文。只输出 JSON：{{\"text\":\"整理后的正文\"}}。\
不要前言，不要代码块。\
删掉嗯、啊、呃、那个、就是、然后、you know 这类口水词，以及来回重复。\
用户中途改口时，只保留改口后的意思。\
可以调整语序，让句子完整、层次清楚，但不要添加用户没说的事实、日期、人名或链接。\
保持用户原来的语言，中英混说就保持中英混说，不要整段翻译。\
{mode_rule}\
{glossary}"
    );
    (system, transcript.to_string())
}

pub fn parse_polished(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(text) = json_text(trimmed) {
        return text;
    }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start < end {
            if let Some(text) = json_text(&trimmed[start..=end]) {
                return text;
            }
        }
    }
    trimmed.trim_matches('`').trim().to_string()
}

fn json_text(raw: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let text = value.get("text")?.as_str()?.trim();
    if text.is_empty() { None } else { Some(text.to_string()) }
}

pub fn max_output_tokens(transcript: &str) -> u32 {
    let chars = transcript.chars().count() as u32;
    (chars.saturating_mul(3)).clamp(320, 2000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prose_asks_for_structure() {
        let (system, _) = messages("嗯，我想做三件事", "com.apple.Notes", &[]);
        assert!(system.contains("口水词"));
        assert!(system.contains("列表"));
    }

    #[test]
    fn literal_mode_mentions_code() {
        let (system, _) = messages("let mut x", "com.microsoft.VSCode", &["OpenTypeless".into()]);
        assert!(system.contains("代码或终端"));
        assert!(system.contains("OpenTypeless"));
    }

    #[test]
    fn parses_wrapped_json() {
        let raw = "好的\n```json\n{\"text\":\"明天开会\"}\n```";
        assert_eq!(parse_polished(raw), "明天开会");
    }

    #[test]
    fn editor_bundle_is_literal() {
        assert_eq!(mode_for_bundle("com.apple.dt.Xcode"), PolishMode::Literal);
        assert_eq!(mode_for_bundle("com.apple.mail"), PolishMode::Email);
    }
}
