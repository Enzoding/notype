//! Channel configuration and protocol boundary. Credentials never enter persisted profiles.
mod catalog;
mod chat;
mod endpoint;
mod profile;
mod readiness;
mod transcription;
mod transport;
pub use catalog::{fetch_models, ModelCatalog, ModelEntry};
pub use chat::{extract_glossary, polish_with_options, test_connection};
pub use endpoint::{api_base, chat_url, mask_key, transcribe_url, validate_base};
pub use profile::{active_provider, Profile, ProviderKind};
pub use readiness::{ChannelChecks, SetupReadiness};
pub use transcription::transcribe_with_model;
pub(crate) use transport::CancellationScope;
pub const DEFAULT_BASE_URL: &str = "https://ai-gateway.vercel.sh";
pub const TRANSCRIBE_MODEL: &str = "microsoft/mai-transcribe-2";
pub const POLISH_MODEL: &str = "deepseek/deepseek-v4.1-flash";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    #[test]
    fn versioned_urls_do_not_duplicate_v1() {
        assert_eq!(
            chat_url("https://openrouter.ai/api/v1/"),
            "https://openrouter.ai/api/v1/chat/completions"
        );
        assert_eq!(
            transcribe_url("https://example.com/v1"),
            "https://example.com/v4/ai/transcription-model"
        );
    }
    #[test]
    fn rejects_credential_bearing_endpoint() {
        assert!(validate_base("https://key@example.com").is_err());
        assert!(validate_base("https://example.com").is_ok());
    }
    #[test]
    fn profiles_preserve_legacy_settings_and_isolate_channels() {
        let store = Store::open_in_memory().unwrap();
        store.set_transcribe_model("old/model").unwrap();
        store
            .set_api_base_url("https://legacy.example.com/v1")
            .unwrap();
        store.set_polish_model("legacy/chat").unwrap();
        let mut p = Profile::load(&store, ProviderKind::OpenRouter);
        p.transcribe_model = "other/model".into();
        p.save(&store).unwrap();
        assert_eq!(
            Profile::load(&store, ProviderKind::Vercel).transcribe_model,
            "old/model"
        );
        assert_eq!(
            Profile::load(&store, ProviderKind::OpenRouter).transcribe_model,
            "other/model"
        );
    }
    #[test]
    fn masks_unicode_without_panicking() {
        assert!(!mask_key("密钥测试很长很长abcdef").is_empty());
    }
}
