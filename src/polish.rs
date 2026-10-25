#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolishMode {
    Prose,
    Literal,
    Email,
    Chat,
}

impl PolishMode {
    pub fn as_str(self) -> &'static str {
        match self {
            PolishMode::Prose => "prose",
            PolishMode::Literal => "literal",
            PolishMode::Email => "email",
            PolishMode::Chat => "chat",
        }
    }

    pub fn from_str(raw: &str) -> Option<Self> {
        match raw {
            "prose" => Some(PolishMode::Prose),
            "literal" => Some(PolishMode::Literal),
            "email" => Some(PolishMode::Email),
            "chat" => Some(PolishMode::Chat),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            PolishMode::Prose => "流畅正文",
            PolishMode::Literal => "代码与终端",
            PolishMode::Email => "商务邮件",
            PolishMode::Chat => "即时通讯",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            PolishMode::Prose => "正文",
            PolishMode::Literal => "代码",
            PolishMode::Email => "邮件",
            PolishMode::Chat => "聊天",
        }
    }

    #[allow(dead_code)]
    pub fn description(self) -> &'static str {
        match self {
            PolishMode::Prose => "去除口癖与重复，按语意分段或分点，适合文档、笔记与通用写作。",
            PolishMode::Literal => "精准保留代码标识符、命令与路径，只清理语气词并补全标点。",
            PolishMode::Email => "整理为可直接发送的得体邮件正文，行动项自动归纳为清晰列表。",
            PolishMode::Chat => "保持自然口吻与简短句式，一两句话直达重点，适合微信、Slack 与飞书。",
        }
    }
}

#[derive(Clone, Debug)]
pub struct PolishOptions {
    pub mode: PolishMode,
    pub custom_prompt: String,
    pub space_cjk: bool,
}

impl Default for PolishOptions {
    fn default() -> Self {
        Self {
            mode: PolishMode::Prose,
            custom_prompt: String::new(),
            space_cjk: true,
        }
    }
}

pub fn resolve_mode(strategy: &str, bundle_id: &str) -> Option<PolishMode> {
    match strategy {
        "raw" => None,
        "prose" => Some(PolishMode::Prose),
        "literal" => Some(PolishMode::Literal),
        "email" => Some(PolishMode::Email),
        "chat" => Some(PolishMode::Chat),
        _ => Some(mode_for_bundle(bundle_id)),
    }
}

pub fn friendly_app_name(bundle_id: &str) -> String {
    let b = bundle_id.trim();
    if b.is_empty() || b == "unknown" {
        return "当前应用".into();
    }
    if b == "notype.studio" || b == "opentypeless.studio" {
        return "演练台".into();
    }
    const KNOWN: &[(&str, &str)] = &[
        ("com.apple.dt.Xcode", "Xcode"),
        ("com.microsoft.VSCode", "VS Code"),
        ("com.todesktop.230313mzl4w4u92", "Cursor"),
        ("com.todesktop.", "Cursor"),
        ("dev.zed.Zed", "Zed"),
        ("com.apple.Terminal", "终端"),
        ("com.googlecode.iterm2", "iTerm2"),
        ("dev.warp.Warp", "Warp"),
        ("com.mitchellh.ghostty", "Ghostty"),
        ("com.apple.mail", "邮件"),
        ("com.microsoft.Outlook", "Outlook"),
        ("com.readdle.smartemail-Mac", "Spark"),
        ("com.mimestream.Mimestream", "Mimestream"),
        ("com.tinyspeck.slackmacgap", "Slack"),
        ("com.tencent.xinWeChat", "微信"),
        ("net.whatsapp.WhatsApp", "WhatsApp"),
        ("com.apple.MobileSMS", "信息"),
        ("ru.keepcoder.Telegram", "Telegram"),
        ("com.hnc.Discord", "Discord"),
        ("com.electron.lark", "飞书"),
        ("com.alibaba.DingTalkMac", "钉钉"),
        ("com.apple.Notes", "备忘录"),
        ("notion.id", "Notion"),
        ("md.obsidian", "Obsidian"),
        ("com.apple.Safari", "Safari"),
        ("com.google.Chrome", "Chrome"),
        ("company.thebrowser.Browser", "Arc"),
        ("com.brave.Browser", "Brave"),
        ("com.apple.finder", "Finder"),
    ];
    for (prefix, name) in KNOWN {
        if b == *prefix || b.starts_with(prefix) {
            return (*name).to_string();
        }
    }
    b.rsplit('.')
        .next()
        .filter(|part| !part.is_empty())
        .unwrap_or(b)
        .to_string()
}

pub fn mode_for_bundle(bundle_id: &str) -> PolishMode {
    const LITERAL_PREFIXES: &[&str] = &[
        "com.apple.dt.Xcode",
        "com.microsoft.VSCode",
        "com.todesktop.",
        "dev.zed.Zed",
        "com.jetbrains.",
        "com.apple.Terminal",
        "com.googlecode.iterm2",
        "dev.warp.Warp",
        "com.mitchellh.ghostty",
        "org.alacritty",
        "net.kovidgoyal.kitty",
    ];
    const EMAIL: &[&str] = &[
        "com.apple.mail",
        "com.microsoft.Outlook",
        "com.readdle.smartemail-Mac",
        "com.mimestream.Mimestream",
    ];
    const CHAT: &[&str] = &[
        "com.tinyspeck.slackmacgap",
        "com.tencent.xinWeChat",
        "net.whatsapp.WhatsApp",
        "com.apple.MobileSMS",
        "ru.keepcoder.Telegram",
        "com.hnc.Discord",
        "com.electron.lark",
        "com.alibaba.DingTalkMac",
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

#[allow(dead_code)]
pub fn messages(transcript: &str, bundle_id: &str, phrases: &[String]) -> (String, String) {
    let options = PolishOptions {
        mode: mode_for_bundle(bundle_id),
        custom_prompt: String::new(),
        space_cjk: false,
    };
    messages_with_options(transcript, phrases, &options)
}

pub fn messages_with_options(
    transcript: &str,
    phrases: &[String],
    options: &PolishOptions,
) -> (String, String) {
    let mode_rule = match options.mode {
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
        }
    };
    let glossary = if phrases.is_empty() {
        "No extra glossary.".to_string()
    } else {
        let lines = phrases
            .iter()
            .map(|phrase| format!("- {phrase}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "Keep these spellings exactly when the term is spoken. Never insert a glossary term that the speaker did not say:\n{lines}"
        )
    };
    let spacing_rule = if options.space_cjk {
        "\nSpacing: insert a single space between Chinese characters and Latin letters/numbers (e.g. '使用 Rust 开发 macOS 应用')."
    } else {
        ""
    };
    let custom_rule = if options.custom_prompt.trim().is_empty() {
        String::new()
    } else {
        format!("\nUser preference: {}", options.custom_prompt.trim())
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
{glossary}{spacing_rule}{custom_rule}"
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
        let (system, _) = messages("let mut x", "com.microsoft.VSCode", &["Notype".into()]);
        assert!(system.contains("code editor or terminal"));
        assert!(system.contains("Do not rewrite code"));
        assert!(system.contains("Notype"));
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
