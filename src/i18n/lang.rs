//! Language identifiers and the pure rule that maps a user preference plus the
//! system's preferred languages to the language the interface will use.

/// A language the interface is translated into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    ZhHans,
    En,
}

impl Language {
    pub fn index(self) -> usize {
        match self {
            Self::ZhHans => 0,
            Self::En => 1,
        }
    }

    pub fn from_index(index: u8) -> Self {
        match index {
            0 => Self::ZhHans,
            _ => Self::En,
        }
    }
}

/// What the user chose in settings; `System` defers to the operating system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Preference {
    #[default]
    System,
    ZhHans,
    En,
}

impl Preference {
    /// Stable value stored in the `ui_language` setting.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::ZhHans => "zh-Hans",
            Self::En => "en",
        }
    }

    /// Unknown or missing values fall back to following the system.
    pub fn from_setting(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some("zh-Hans") => Self::ZhHans,
            Some("en") => Self::En,
            _ => Self::System,
        }
    }
}

/// Language for one BCP-47 / macOS locale identifier, if we ship it.
///
/// Only Simplified Chinese is supported: `zh-Hant*` and `zh-TW/HK/MO` are
/// Traditional and must not be shown simplified text, so they are not matched.
fn match_identifier(identifier: &str) -> Option<Language> {
    let lower = identifier.trim().to_ascii_lowercase().replace('_', "-");
    let mut parts = lower.split('-');
    match parts.next()? {
        "en" => Some(Language::En),
        "zh" => {
            let rest: Vec<&str> = parts.collect();
            let traditional = rest
                .iter()
                .any(|p| matches!(*p, "hant" | "tw" | "hk" | "mo"));
            (!traditional).then_some(Language::ZhHans)
        }
        _ => None,
    }
}

/// Resolve the language to display. With `System`, the first system language
/// we support wins; a first language we do not ship (e.g. French) falls back to
/// English rather than skipping ahead to a later entry the user ranked lower.
pub fn resolve(preference: Preference, system_languages: &[String]) -> Language {
    match preference {
        Preference::ZhHans => Language::ZhHans,
        Preference::En => Language::En,
        Preference::System => system_languages
            .first()
            .and_then(|first| match_identifier(first))
            .unwrap_or(Language::En),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sys(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn system_language_maps_to_supported_languages() {
        for (id, expected) in [
            ("zh-Hans-CN", Language::ZhHans),
            ("zh-Hans", Language::ZhHans),
            ("zh_CN", Language::ZhHans),
            ("zh-SG", Language::ZhHans),
            ("en-US", Language::En),
            ("en", Language::En),
            ("fr-FR", Language::En),
            ("ja-JP", Language::En),
        ] {
            assert_eq!(resolve(Preference::System, &sys(&[id])), expected, "{id}");
        }
    }

    #[test]
    fn traditional_chinese_and_empty_lists_fall_back_to_english() {
        for id in ["zh-Hant-TW", "zh-Hant", "zh-TW", "zh-HK", "zh-Hant-HK"] {
            assert_eq!(resolve(Preference::System, &sys(&[id])), Language::En);
        }
        assert_eq!(resolve(Preference::System, &[]), Language::En);
    }

    #[test]
    fn only_the_first_system_language_is_considered() {
        assert_eq!(
            resolve(Preference::System, &sys(&["fr-FR", "zh-Hans-CN"])),
            Language::En
        );
        assert_eq!(
            resolve(Preference::System, &sys(&["zh-Hans-CN", "en-US"])),
            Language::ZhHans
        );
    }

    #[test]
    fn explicit_preference_ignores_system_language() {
        assert_eq!(resolve(Preference::En, &sys(&["zh-Hans-CN"])), Language::En);
        assert_eq!(
            resolve(Preference::ZhHans, &sys(&["en-US"])),
            Language::ZhHans
        );
    }

    #[test]
    fn preference_setting_round_trips_and_defaults_to_system() {
        for p in [Preference::System, Preference::ZhHans, Preference::En] {
            assert_eq!(Preference::from_setting(Some(p.as_str())), p);
        }
        assert_eq!(Preference::from_setting(None), Preference::System);
        assert_eq!(
            Preference::from_setting(Some("klingon")),
            Preference::System
        );
    }
}
