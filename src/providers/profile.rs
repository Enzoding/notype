use super::DEFAULT_BASE_URL;
use crate::store::Store;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderKind {
    #[default]
    Vercel,
    OpenRouter,
}
impl ProviderKind {
    pub const ALL: [Self; 2] = [Self::Vercel, Self::OpenRouter];
    pub fn index(self) -> usize {
        match self {
            Self::Vercel => 0,
            Self::OpenRouter => 1,
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Vercel => "vercel",
            Self::OpenRouter => "openrouter",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Vercel => "Vercel AI Gateway",
            Self::OpenRouter => "OpenRouter",
        }
    }
    pub fn parse(s: &str) -> Self {
        if s == "openrouter" {
            Self::OpenRouter
        } else {
            Self::Vercel
        }
    }
    pub fn base(self) -> &'static str {
        match self {
            Self::Vercel => DEFAULT_BASE_URL,
            Self::OpenRouter => "https://openrouter.ai/api/v1",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub kind: ProviderKind,
    pub base_url: String,
    pub transcribe_model: String,
    pub polish_model: String,
}
impl Profile {
    pub fn load(store: &Store, kind: ProviderKind) -> Self {
        let get = |name: &str, fallback: &str| {
            store
                .setting(&format!("provider.{}.{name}", kind.id()))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| fallback.into())
        };
        if kind == ProviderKind::Vercel {
            Self {
                kind,
                base_url: kind.base().to_string(),
                transcribe_model: get("transcribe", &store.transcribe_model()),
                polish_model: get("polish", &store.polish_model()),
            }
        } else {
            Self {
                kind,
                base_url: kind.base().to_string(),
                transcribe_model: get("transcribe", "openai/whisper-large-v3"),
                polish_model: get("polish", "openai/gpt-4o-mini"),
            }
        }
    }
    pub fn save(&self, store: &Store) -> Result<(), String> {
        for (name, value) in [
            ("transcribe", &self.transcribe_model),
            ("polish", &self.polish_model),
        ] {
            store
                .set_setting(&format!("provider.{}.{name}", self.kind.id()), value)
                .map_err(|_| "无法保存渠道配置".to_string())?;
        }
        Ok(())
    }
}

/// Preserve the previously selected transcription channel when upgrading split routing.
pub fn active_provider(store: &Store) -> ProviderKind {
    ProviderKind::parse(
        &store
            .setting("active_provider")
            .or_else(|| store.setting("transcribe_provider"))
            .unwrap_or_default(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_profile_never_changes_the_channel_endpoint() {
        let store = Store::open_in_memory().unwrap();
        let mut p = Profile::load(&store, ProviderKind::OpenRouter);
        p.base_url = "https://custom.invalid".into();
        p.save(&store).unwrap();
        assert_eq!(
            Profile::load(&store, ProviderKind::OpenRouter).base_url,
            ProviderKind::OpenRouter.base()
        );
    }
    #[test]
    fn channel_address_cannot_be_overridden_by_legacy_settings() {
        let store = Store::open_in_memory().unwrap();
        store.set_api_base_url("https://legacy.invalid").unwrap();
        store
            .set_setting("provider.vercel.base", "https://custom.invalid")
            .unwrap();
        store
            .set_setting("provider.openrouter.base", "https://custom.invalid")
            .unwrap();
        for kind in ProviderKind::ALL {
            assert_eq!(Profile::load(&store, kind).base_url, kind.base());
        }
    }
    #[test]
    fn unified_channel_migrates_from_old_transcription_choice() {
        let store = Store::open_in_memory().unwrap();
        store
            .set_setting("transcribe_provider", "openrouter")
            .unwrap();
        store.set_setting("polish_provider", "vercel").unwrap();
        assert_eq!(active_provider(&store), ProviderKind::OpenRouter);
        store.set_setting("active_provider", "vercel").unwrap();
        assert_eq!(active_provider(&store), ProviderKind::Vercel);
    }
}
