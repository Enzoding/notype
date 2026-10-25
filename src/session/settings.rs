use gpui_kit::Context;

use crate::audio::{self, SoundCue};
use crate::capture::ActiveRecording;
use crate::gateway;
use crate::hotkey;
use crate::mining::AutoLearnMode;
use crate::secrets::{self, KeySource};
use crate::store::TriggerMode;

use super::Session;

pub(super) fn key_status(key: &str, source: &KeySource) -> String {
    if key.trim().is_empty() {
        return "未配置 API Key".into();
    }
    if matches!(source, KeySource::MigrationPending) {
        "钥匙串迁移未完成，请检查授权并重新保存密钥".into()
    } else {
        "密钥已保存在钥匙串".into()
    }
}

impl Session {
    pub fn set_dock(&mut self, show: bool, cx: &mut Context<Self>) {
        if let Ok(store) = self.store.lock() {
            let _ = store.set_show_dock(show);
        }
        self.show_dock = show;
        cx.set_activation_policy(if show {
            gpui_kit::ActivationPolicy::Regular
        } else {
            gpui_kit::ActivationPolicy::Accessory
        });
        if show {
            cx.activate(true);
        }
        cx.notify();
    }

    pub fn set_hotkey_code(&mut self, code: i64, cx: &mut Context<Self>) {
        self.hotkey_code = code;
        hotkey::set_hotkey_code(code);
        if let Ok(store) = self.store.lock() {
            let _ = store.set_hotkey_code(code);
        }
        cx.notify();
    }

    pub fn set_trigger_mode(&mut self, mode: TriggerMode, cx: &mut Context<Self>) {
        self.trigger_mode = mode;
        hotkey::set_trigger_mode(mode);
        if let Ok(store) = self.store.lock() {
            let _ = store.set_trigger_mode(mode);
        }
        cx.notify();
    }

    pub fn set_polish_strategy(&mut self, strategy: &str, cx: &mut Context<Self>) {
        self.polish_strategy = strategy.to_string();
        if let Ok(store) = self.store.lock() {
            let _ = store.set_polish_strategy(strategy);
        }
        cx.notify();
    }

    pub fn set_custom_prompt(&mut self, prompt: &str, cx: &mut Context<Self>) {
        self.custom_prompt = prompt.trim().to_string();
        if let Ok(store) = self.store.lock() {
            let _ = store.set_custom_prompt(&self.custom_prompt);
        }
        cx.notify();
    }

    pub fn set_space_cjk(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.space_cjk = enabled;
        if let Ok(store) = self.store.lock() {
            let _ = store.set_space_cjk(enabled);
        }
        cx.notify();
    }

    pub fn set_sound_feedback(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.sound_feedback = enabled;
        if let Ok(store) = self.store.lock() {
            let _ = store.set_sound_feedback(enabled);
        }
        if enabled {
            audio::play_cue(SoundCue::Start);
        }
        cx.notify();
    }

    pub fn set_save_audio(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.save_audio = enabled;
        if let Ok(store) = self.store.lock() {
            let _ = store.set_save_audio(enabled);
        }
        cx.notify();
    }

    pub fn set_auto_learn_mode(&mut self, mode: AutoLearnMode, cx: &mut Context<Self>) {
        self.auto_learn_mode = mode;
        if let Ok(store) = self.store.lock() {
            let _ = store.set_auto_learn_mode(mode.as_str());
        }
        cx.notify();
    }

    pub fn set_learn_from_edits(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.learn_from_edits = enabled;
        if let Ok(store) = self.store.lock() {
            let _ = store.set_learn_from_edits(enabled);
        }
        cx.notify();
    }

    pub fn set_mic_device(&mut self, name: &str, cx: &mut Context<Self>) {
        self.mic_device = name.to_string();
        if let Ok(store) = self.store.lock() {
            let _ = store.set_mic_device(name);
        }
        if self.mic_testing {
            self.toggle_mic_test(cx);
            self.toggle_mic_test(cx);
        }
        cx.notify();
    }

    pub fn refresh_mics(&mut self, cx: &mut Context<Self>) {
        if self.mic_refreshing {
            return;
        }
        self.mic_refreshing = true;
        cx.spawn(async move |this, cx| {
            let devices = cx
                .background_executor()
                .spawn(async { crate::capture::list_input_devices() })
                .await;
            let _ = this.update(cx, |s, cx| {
                s.available_mics = devices;
                s.mic_refreshing = false;
                cx.notify();
            });
        })
        .detach();
    }

    pub fn toggle_mic_test(&mut self, cx: &mut Context<Self>) {
        if let Some(stream) = self.mic_test_stream.take() {
            drop(stream.finish());
            self.mic_testing = false;
            self.level = 0.0;
            self.wave = [0.0; crate::capture::WAVE_BARS];
        } else if self.recording.is_none() {
            let preferred = if self.mic_device.is_empty() {
                None
            } else {
                Some(self.mic_device.as_str())
            };
            if let Ok(stream) = ActiveRecording::start_with_device(preferred) {
                self.mic_test_stream = Some(stream);
                self.mic_testing = true;
            }
        }
        cx.notify();
    }

    pub fn set_transcribe_model(&mut self, model: &str, cx: &mut Context<Self>) {
        let model = model.trim();
        self.connections.transcribe_model = if model.is_empty() {
            if self.connections.kind == crate::providers::ProviderKind::OpenRouter {
                "openai/whisper-large-v3".into()
            } else {
                gateway::TRANSCRIBE_MODEL.to_string()
            }
        } else {
            model.to_string()
        };
        self.save_profile();
        cx.notify();
    }

    pub fn set_polish_model(&mut self, model: &str, cx: &mut Context<Self>) {
        let model = model.trim();
        self.connections.polish_model = if model.is_empty() {
            if self.connections.kind == crate::providers::ProviderKind::OpenRouter {
                "openai/gpt-4o-mini".into()
            } else {
                gateway::POLISH_MODEL.to_string()
            }
        } else {
            model.to_string()
        };
        self.save_profile();
        cx.notify();
    }

    pub fn test_gateway(&mut self, cx: &mut Context<Self>) {
        if self.connections.gateway_testing {
            return;
        }
        let base_url = self.connections.kind.base().to_string();
        let key = self.connections.key.clone();
        let model = self.connections.polish_model.clone();
        let kind = self.connections.kind;
        self.connections.gateway_testing = true;
        self.connections.gateway_test_status = None;
        cx.notify();
        let model_for_status = model.clone();
        let revision = self.connections.revision;
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_executor()
                .spawn(async move { gateway::test_connection(&base_url, &key, &model) })
                .await;
            let _ = this.update(cx, |session, cx| {
                session.connections.gateway_testing = false;
                if session.connections.revision != revision
                    || session.connections.kind != kind
                    || session.connections.polish_model != model_for_status
                {
                    cx.notify();
                    return;
                }
                let result = match res {
                    Ok(ms) => (true, format!("润色模型已连通，延迟 {ms} ms")),
                    Err(err) => (false, err),
                };
                session.connections.checks[kind.index()].polishing = Some(result.clone());
                session.connections.gateway_test_status = Some(result);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn save_key(&mut self, value: &str, cx: &mut Context<Self>) {
        let value = value.trim();
        if value.is_empty() {
            self.connections.key_status = "请先填写密钥".into();
            cx.notify();
            return;
        }
        match secrets::save_provider(self.connections.kind, value) {
            Ok(()) => {
                self.invalidate_connection_checks();
                self.connections.key = value.into();
                self.connections.provider_keys[if self.connections.kind
                    == crate::providers::ProviderKind::Vercel
                {
                    0
                } else {
                    1
                }] = value.into();
                self.connections.key_status = "密钥已保存在钥匙串".into();
                self.connections.gateway_test_status = None;
            }
            Err(err) => self.connections.key_status = err,
        }
        cx.notify();
    }
    pub fn clear_key(&mut self, cx: &mut Context<Self>) {
        match secrets::clear_provider(self.connections.kind) {
            Ok(()) => {
                self.invalidate_connection_checks();
                self.connections.key.clear();
                self.connections.provider_keys[if self.connections.kind
                    == crate::providers::ProviderKind::Vercel
                {
                    0
                } else {
                    1
                }]
                .clear();
                self.connections.key_status = "未配置当前渠道密钥".into();
                self.connections.gateway_test_status = None;
            }
            Err(err) => self.connections.key_status = err,
        }
        cx.notify();
    }
}
