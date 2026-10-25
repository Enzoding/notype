pub struct ConnectionState {
    pub revision: u64,
    pub kind: crate::providers::ProviderKind,
    pub checks: [crate::providers::ChannelChecks; 2],
    pub profiles: [crate::providers::Profile; 2],
    pub provider_keys: [String; 2],
    pub model_catalogs: [crate::providers::ModelCatalog; 2],
    pub models_loading: bool,
    pub model_status: String,
    pub transcribe_model: String,
    pub polish_model: String,
    pub key_status: String,
    pub gateway_test_status: Option<(bool, String)>,
    pub gateway_testing: bool,
    pub key: String,
}

use super::Session;
use crate::providers::{self, ModelEntry, Profile, ProviderKind};
use gpui_kit::Context;
impl Session {
    pub fn profile(&self, kind: ProviderKind) -> Profile {
        self.connections.profiles[if kind == ProviderKind::Vercel { 0 } else { 1 }].clone()
    }
    pub fn provider_key(&self, kind: ProviderKind) -> String {
        self.connections.provider_keys[if kind == ProviderKind::Vercel { 0 } else { 1 }].clone()
    }
    pub fn polish_route(&self) -> (String, String, String) {
        let p = self.profile(self.connections.kind);
        (
            p.base_url,
            self.provider_key(self.connections.kind),
            p.polish_model,
        )
    }
    pub fn select_provider(&mut self, kind: ProviderKind, cx: &mut Context<Self>) {
        let saved = self
            .store
            .lock()
            .map_err(|_| "无法读取渠道配置".to_string())
            .and_then(|store| {
                store
                    .set_setting("active_provider", kind.id())
                    .map_err(|_| "无法保存渠道选择".into())
            });
        if let Err(error) = saved {
            self.connections.key_status = error;
            cx.notify();
            return;
        }
        let p = self.profile(kind);
        self.connections.revision = self.connections.revision.wrapping_add(1);
        self.connections.kind = kind;
        self.connections.transcribe_model = p.transcribe_model;
        self.connections.polish_model = p.polish_model;
        self.connections.key = self.provider_key(kind);
        self.connections.key_status = if self.connections.key.is_empty() {
            crate::i18n::t("settings.key.channel_not_configured")
        } else {
            crate::i18n::t("settings.key.saved_in_keychain")
        }
        .to_string();
        self.connections.gateway_test_status = None;
        self.connections.model_status.clear();
        cx.notify();
    }
    pub(super) fn invalidate_connection_checks(&mut self) {
        self.connections.revision = self.connections.revision.wrapping_add(1);
        self.connections.checks[self.connections.kind.index()].invalidate();
        self.connections.gateway_test_status = None;
    }
    pub(super) fn save_profile(&mut self) {
        self.invalidate_connection_checks();
        let p = Profile {
            kind: self.connections.kind,
            base_url: self.connections.kind.base().to_string(),
            transcribe_model: self.connections.transcribe_model.clone(),
            polish_model: self.connections.polish_model.clone(),
        };
        if let Ok(store) = self.store.lock() {
            match p.save(&store) {
                Ok(()) => {
                    self.connections.profiles[if self.connections.kind == ProviderKind::Vercel {
                        0
                    } else {
                        1
                    }] = p
                }
                Err(e) => self.connections.key_status = e,
            }
        }
        self.connections.gateway_test_status = None;
    }
    pub fn refresh_models(&mut self, cx: &mut Context<Self>) {
        if self.connections.models_loading {
            return;
        }
        let profile = self.profile(self.connections.kind);
        let kind = self.connections.kind;
        let key = self.provider_key(kind);
        let tested_base = profile.base_url.clone();
        self.connections.models_loading = true;
        self.connections.model_status = crate::i18n::t("settings.catalog.fetching").to_string();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { providers::fetch_models(&profile, &key) })
                .await;
            let _ = this.update(cx, |s, cx| {
                s.connections.models_loading = false;
                match result {
                    Ok(models) => {
                        if s.profile(kind).base_url != tested_base {
                            s.connections.model_status =
                                crate::i18n::t("settings.catalog.endpoint_changed").to_string();
                            cx.notify();
                            return;
                        }
                        if let Ok(store) = s.store.lock() {
                            if let Ok(value) = serde_json::to_string(&models) {
                                let _ = store.set_setting(&format!("models.{}", kind.id()), &value);
                            }
                        }
                        s.connections.model_catalogs
                            [if kind == ProviderKind::Vercel { 0 } else { 1 }] =
                            crate::providers::ModelCatalog::from_entries(models);
                        s.connections.model_status =
                            crate::i18n::t("settings.catalog.updated").to_string();
                    }
                    Err(err) => {
                        let err_str = err.to_string();
                        s.connections.model_status = crate::i18n::t_args(
                            "settings.catalog.failed_keep_last",
                            &[("error", &err_str)],
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub fn catalog(&self, transcription: bool) -> Vec<ModelEntry> {
        self.connections.model_catalogs[self.connections.kind.index()]
            .visible(transcription)
            .to_vec()
    }
    pub fn test_transcription(&mut self, cx: &mut Context<Self>) {
        if self.connections.gateway_testing {
            return;
        }
        let p = self.profile(self.connections.kind);
        let key = self.provider_key(self.connections.kind);
        self.connections.gateway_testing = true;
        self.connections.gateway_test_status = None;
        let kind = p.kind;
        let model = p.transcribe_model.clone();
        let revision = self.connections.revision;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    // Generate a synthetic spoken phrase with macOS; no user audio is used.
                    let path = std::env::temp_dir().join(format!(
                        "notype-test-{}-{}.wav",
                        std::process::id(),
                        crate::tasks::now()
                    ));
                    let output = std::process::Command::new("/usr/bin/say")
                        .args(["-o"])
                        .arg(&path)
                        .args([
                            "--file-format=WAVE",
                            "--data-format=LEI16@16000",
                            "Notype connection test",
                        ])
                        .output()
                        .map_err(|_| "无法生成测试语音".to_string())?;
                    if !output.status.success() {
                        return Err("无法生成测试语音".into());
                    }
                    let audio = std::fs::read(&path).map_err(|_| "无法读取测试语音".to_string());
                    let _ = std::fs::remove_file(path);
                    crate::gateway::transcribe_with_model(
                        p.kind,
                        &p.base_url,
                        &key,
                        &audio?,
                        &[],
                        &p.transcribe_model,
                    )
                    .map(|_| ())
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                s.connections.gateway_testing = false;
                if s.connections.revision == revision
                    && s.connections.kind == kind
                    && s.connections.transcribe_model == model
                {
                    let result = match result {
                        Ok(()) => (
                            true,
                            crate::i18n::t("settings.transcription.test_success").to_string(),
                        ),
                        Err(e) => (false, e),
                    };
                    s.connections.checks[kind.index()].transcription = Some(result.clone());
                    s.connections.gateway_test_status = Some(result);
                }
                cx.notify();
            });
        })
        .detach();
    }
}
