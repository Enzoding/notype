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
            "The frontmost app is a code editor or terminal. Remove fillers and repeats, add punctuation, and apply the glossary. Do not rewrite code, commands, identifiers, or paths into prose."
        }
        PolishMode::Email => {
            "Turn this into an email body that can be sent as-is. Remove fillers. When there are requests or action items, use a short list. Do not add a subject line unless the speaker stated one."
        }
        PolishMode::Chat => {
            "Turn this into a short message. Remove fillers and repeats. Keep the speaker's tone. Stay in one or two sentences when that is enough."
        }
        PolishMode::Prose => {
            "Turn spoken words into text people can read. Do not leave the transcript verbatim. Remove fillers, repeats, and unfinished fragments. Split into paragraphs by meaning. When the speaker gives steps, a comparison, or several requests, use a list."
        },
    };
    let glossary = if phrases.is_empty() {
        "No extra glossary.".to_string()
    } else {
        let lines = phrases
            .iter()
            .map(|phrase| format!("- {phrase}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("Keep these spellings exactly:\n{lines}")
    };
    let system = format!(
        "You edit a speech transcript into text the speaker can send or keep. \
Reply with JSON only: {{\"text\":\"...\"}}. \
No preamble, no markdown, and no code fence. The JSON string is the finished text.\n\
Language: write in the language of the transcript. \
If the speaker mixed languages, keep that mix. \
Do not translate unless the speaker explicitly asked for a translation. \
Use the punctuation, capitalization, and list style of the transcript's language. \
The text value must stay in that language even though these instructions are in English.\n\
Cleanup: remove fillers and hesitation in whatever language was spoken, \
such as um, uh, like, you know, I mean, 嗯, 啊, 呃, 那个, 就是, 然后, euh, ähm, este, えーと. \
Remove repeats, false starts, and unfinished fragments. \
If the speaker corrects themselves, keep only the corrected meaning. \
You may reorder words so the result is complete and easy to scan. \
Do not add facts, dates, names, numbers, links, or opinions the speaker did not say. \
Keep names, code, URLs, and glossary spellings exactly. \
Do not convert currencies, units, calendars, or number formats.\n\
{mode_rule}\n\
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
        assert!(system.contains("fillers"));
        assert!(system.contains("use a list"));
        assert!(system.contains("language of the transcript"));
        assert!(system.contains("Do not translate"));
    }

    #[test]
    fn literal_mode_mentions_code() {
        let (system, _) = messages("let mut x", "com.microsoft.VSCode", &["OpenTypeless".into()]);
        assert!(system.contains("code editor or terminal"));
        assert!(system.contains("Do not rewrite code"));
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
